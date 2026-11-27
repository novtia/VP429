use chrono::Local;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    RateLimit429,
    Success,
    Error,
}

impl LogLevel {
    pub fn tag(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::RateLimit429 => "429",
            LogLevel::Success => "OK",
            LogLevel::Error => "ERR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppEvent {
    StateChanged,
    LogAdded(LogEntry),
}

pub struct EventBus {
    logs: RwLock<VecDeque<LogEntry>>,
    tx: broadcast::Sender<AppEvent>,
    version: AtomicU64,
}

impl EventBus {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(256);
        Arc::new(Self {
            logs: RwLock::new(VecDeque::with_capacity(200)),
            tx,
            version: AtomicU64::new(1),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    pub fn notify_state(&self) {
        self.version.fetch_add(1, Ordering::Release);
        let _ = self.tx.send(AppEvent::StateChanged);
    }

    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        let msg = message.into();
        let ts = Local::now().format("%H:%M:%S").to_string();
        #[cfg(debug_assertions)]
        eprintln!("[{}] [{:<4}] {}", ts, level.tag(), msg);
        let entry = LogEntry {
            timestamp: ts,
            level,
            message: msg,
        };
        if let Ok(mut guard) = self.logs.write() {
            if guard.len() >= 200 {
                guard.pop_front();
            }
            guard.push_back(entry.clone());
        }
        self.version.fetch_add(1, Ordering::Release);
        let _ = self.tx.send(AppEvent::LogAdded(entry));
    }

    pub fn info(&self, msg: impl Into<String>) {
        self.log(LogLevel::Info, msg);
    }

    pub fn warn_429(&self, msg: impl Into<String>) {
        self.log(LogLevel::RateLimit429, msg);
    }

    pub fn ok(&self, msg: impl Into<String>) {
        self.log(LogLevel::Success, msg);
    }

    pub fn error(&self, msg: impl Into<String>) {
        self.log(LogLevel::Error, msg);
    }

    pub fn snapshot_logs(&self) -> Vec<LogEntry> {
        self.logs
            .read()
            .map(|g| g.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_logs(&self) {
        if let Ok(mut g) = self.logs.write() {
            g.clear();
        }
        self.notify_state();
    }
}
