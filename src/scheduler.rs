use crate::config::RotationMode;
use crate::mihomo::ProxySwitcher;
use crate::state::EventBus;
use crate::subscription::ProxyNodeMeta;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeSnapshot {
    pub name: String,
    pub proxy_type: String,
    pub server: String,
    pub port: u16,
    pub latency_ms: Option<u64>,
    pub cooldown_remaining_secs: u64,
    pub hit_429_count: u64,
    pub success_count: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct SchedulerSnapshot {
    pub active_node: Option<String>,
    pub nodes: Vec<NodeSnapshot>,
    pub total_429: u64,
    pub total_retries_ok: u64,
    pub ready_count: usize,
    pub cooldown_count: usize,
}

struct NodeEntry {
    meta: ProxyNodeMeta,
    latency_ms: Option<u64>,
    cooldown_until: Option<Instant>,
    hit_429_count: u64,
    success_count: u64,
}

impl NodeEntry {
    fn is_ready(&self, now: Instant) -> bool {
        match self.cooldown_until {
            Some(until) => now >= until,
            None => true,
        }
    }

    fn cooldown_remaining(&self, now: Instant) -> u64 {
        match self.cooldown_until {
            Some(until) if until > now => (until - now).as_secs().max(1),
            _ => 0,
        }
    }
}

struct SchedulerInner {
    nodes: Vec<NodeEntry>,
    active_index: usize,
    mode: RotationMode,
    cooldown: Duration,
    total_429: u64,
    total_retries_ok: u64,
    switcher: Option<Arc<dyn ProxySwitcher>>,
}

pub struct NodeScheduler {
    inner: RwLock<SchedulerInner>,
    bus: Arc<EventBus>,
}

impl NodeScheduler {
    pub fn new(mode: RotationMode, cooldown_secs: u64, bus: Arc<EventBus>) -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(SchedulerInner {
                nodes: Vec::new(),
                active_index: 0,
                mode,
                cooldown: Duration::from_secs(cooldown_secs.max(1)),
                total_429: 0,
                total_retries_ok: 0,
                switcher: None,
            }),
            bus,
        })
    }

    pub fn set_switcher(&self, switcher: Option<Arc<dyn ProxySwitcher>>) {
        if let Ok(mut g) = self.inner.write() {
            g.switcher = switcher;
        }
    }

    pub fn update_policy(&self, mode: RotationMode, cooldown_secs: u64) {
        if let Ok(mut g) = self.inner.write() {
            g.mode = mode;
            g.cooldown = Duration::from_secs(cooldown_secs.max(1));
        }
        self.bus.notify_state();
    }

    pub fn load_nodes(&self, metas: Vec<ProxyNodeMeta>) {
        if let Ok(mut g) = self.inner.write() {
            g.nodes = metas
                .into_iter()
                .map(|meta| NodeEntry {
                    meta,
                    latency_ms: None,
                    cooldown_until: None,
                    hit_429_count: 0,
                    success_count: 0,
                })
                .collect();
            g.active_index = 0;
        }
        self.bus.notify_state();
    }

    pub fn set_latency(&self, node_name: &str, ms: Option<u64>) {
        if let Ok(mut g) = self.inner.write() {
            if let Some(node) = g.nodes.iter_mut().find(|n| n.meta.name == node_name) {
                node.latency_ms = ms;
            }
        }
        self.bus.notify_state();
    }

    pub fn current_node_name(&self) -> Option<String> {
        let g = self.inner.read().ok()?;
        g.nodes.get(g.active_index).map(|n| n.meta.name.clone())
    }

    pub fn record_success(&self, node_name: &str, was_retry: bool) {
        if let Ok(mut g) = self.inner.write() {
            if let Some(node) = g.nodes.iter_mut().find(|n| n.meta.name == node_name) {
                node.success_count += 1;
            }
            if was_retry {
                g.total_retries_ok += 1;
            }
        }
        self.bus.notify_state();
    }

    /// Called before forwarding an intercepted request.
    /// In `RoundRobinPerRequest` mode, advances to the next ready node and updates the backend switcher.
    pub async fn prepare_for_request(&self) -> Option<String> {
        let (target_name, switcher, needs_switch) = {
            let mut g = self.inner.write().ok()?;
            if g.nodes.is_empty() {
                return None;
            }
            let now = Instant::now();
            if g.mode == RotationMode::RoundRobinPerRequest {
                if let Some(next_idx) = Self::find_next_ready(&g.nodes, g.active_index, now) {
                    g.active_index = next_idx;
                }
                (
                    g.nodes[g.active_index].meta.name.clone(),
                    g.switcher.clone(),
                    true,
                )
            } else if !g.nodes[g.active_index].is_ready(now) {
                if let Some(next_idx) = Self::find_next_ready(&g.nodes, g.active_index, now) {
                    g.active_index = next_idx;
                    (
                        g.nodes[next_idx].meta.name.clone(),
                        g.switcher.clone(),
                        true,
                    )
                } else {
                    (g.nodes[g.active_index].meta.name.clone(), None, false)
                }
            } else {
                (g.nodes[g.active_index].meta.name.clone(), None, false)
            }
        };

        if needs_switch {
            Some(self.apply_switch(target_name, switcher).await)
        } else {
            Some(target_name)
        }
    }

    /// Called when an upstream response returns HTTP 429 (or configured rate-limit status).
    /// Puts `offending_node` on cooldown, selects the next healthy node, and switches mihomo.
    pub async fn report_429(&self, offending_node: &str, target_host: &str) -> Option<String> {
        let (next_name, cooldown_secs, switcher, all_cooled) = {
            let mut g = self.inner.write().ok()?;
            if g.nodes.is_empty() {
                return None;
            }
            let now = Instant::now();
            let cd = g.cooldown;
            g.total_429 += 1;

            let mut off_idx = g.active_index;
            if let Some((idx, node)) = g
                .nodes
                .iter_mut()
                .enumerate()
                .find(|(_, n)| n.meta.name == offending_node)
            {
                node.cooldown_until = Some(now + cd);
                node.hit_429_count += 1;
                off_idx = idx;
            }

            let (next_idx, all_cooled) = match Self::find_next_ready(&g.nodes, off_idx, now) {
                Some(idx) => (idx, false),
                None => {
                    // Fallback: pick node with earliest cooldown_until (excluding current if possible)
                    let best = g
                        .nodes
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| g.nodes.len() == 1 || *i != off_idx)
                        .min_by_key(|(_, n)| n.cooldown_until.unwrap_or(now))
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    (best, true)
                }
            };

            g.active_index = next_idx;
            (
                g.nodes[next_idx].meta.name.clone(),
                cd.as_secs(),
                g.switcher.clone(),
                all_cooled,
            )
        };

        self.bus.warn_429(format!(
            "{} 命中 429 -> {} 进入冷却 {}s，切换至 {}",
            target_host, offending_node, cooldown_secs, next_name
        ));
        if all_cooled {
            self.bus
                .warn_429("所有节点均处于 429 冷却中，已回退至最快解冻节点");
        }

        let switched = self.apply_switch(next_name, switcher).await;
        self.bus.notify_state();
        Some(switched)
    }

    pub async fn rotate_next(&self, reason: &str) -> Option<String> {
        let (next_name, switcher) = {
            let mut g = self.inner.write().ok()?;
            if g.nodes.is_empty() {
                return None;
            }
            let now = Instant::now();
            let next_idx = Self::find_next_ready(&g.nodes, g.active_index, now)
                .unwrap_or((g.active_index + 1) % g.nodes.len());
            g.active_index = next_idx;
            (
                g.nodes[next_idx].meta.name.clone(),
                g.switcher.clone(),
            )
        };

        self.bus.info(format!("{} -> {}", reason, next_name));
        let switched = self.apply_switch(next_name, switcher).await;
        self.bus.notify_state();
        Some(switched)
    }

    pub async fn select_manual(&self, node_name: &str) -> Option<String> {
        let (target_name, switcher) = {
            let mut g = self.inner.write().ok()?;
            let idx = g.nodes.iter().position(|n| n.meta.name == node_name)?;
            g.active_index = idx;
            (g.nodes[idx].meta.name.clone(), g.switcher.clone())
        };
        self.bus.info(format!("手动切换出口节点 -> {}", target_name));
        let switched = self.apply_switch(target_name, switcher).await;
        self.bus.notify_state();
        Some(switched)
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        let now = Instant::now();
        let Ok(g) = self.inner.read() else {
            return SchedulerSnapshot {
                active_node: None,
                nodes: Vec::new(),
                total_429: 0,
                total_retries_ok: 0,
                ready_count: 0,
                cooldown_count: 0,
            };
        };

        let mut ready_count = 0;
        let mut cooldown_count = 0;
        let nodes = g
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let cd = n.cooldown_remaining(now);
                if cd == 0 {
                    ready_count += 1;
                } else {
                    cooldown_count += 1;
                }
                NodeSnapshot {
                    name: n.meta.name.clone(),
                    proxy_type: n.meta.proxy_type.clone(),
                    server: n.meta.server.clone(),
                    port: n.meta.port,
                    latency_ms: n.latency_ms,
                    cooldown_remaining_secs: cd,
                    hit_429_count: n.hit_429_count,
                    success_count: n.success_count,
                    is_active: i == g.active_index,
                }
            })
            .collect();

        SchedulerSnapshot {
            active_node: g.nodes.get(g.active_index).map(|n| n.meta.name.clone()),
            nodes,
            total_429: g.total_429,
            total_retries_ok: g.total_retries_ok,
            ready_count,
            cooldown_count,
        }
    }

    fn find_next_ready(nodes: &[NodeEntry], from: usize, now: Instant) -> Option<usize> {
        let len = nodes.len();
        if len == 0 {
            return None;
        }
        for step in 1..=len {
            let idx = (from + step) % len;
            if nodes[idx].is_ready(now) {
                return Some(idx);
            }
        }
        None
    }

    async fn apply_switch(&self, name: String, switcher: Option<Arc<dyn ProxySwitcher>>) -> String {
        if let Some(sw) = switcher {
            if let Err(err) = sw.select_node(&name).await {
                self.bus
                    .error(format!("同步节点 {} 到 Mihomo 失败: {}", name, err));
            }
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    struct MockSwitcher {
        selected: Mutex<Vec<String>>,
    }

    impl ProxySwitcher for MockSwitcher {
        fn select_node<'a>(
            &'a self,
            node_name: &'a str,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
            let name = node_name.to_string();
            Box::pin(async move {
                self.selected.lock().unwrap().push(name);
                Ok(())
            })
        }
    }

    fn sample_nodes() -> Vec<ProxyNodeMeta> {
        vec![
            ProxyNodeMeta {
                name: "Node-A".into(),
                proxy_type: "vless".into(),
                server: "1.1.1.1".into(),
                port: 443,
            },
            ProxyNodeMeta {
                name: "Node-B".into(),
                proxy_type: "trojan".into(),
                server: "2.2.2.2".into(),
                port: 443,
            },
            ProxyNodeMeta {
                name: "Node-C".into(),
                proxy_type: "ss".into(),
                server: "3.3.3.3".into(),
                port: 443,
            },
        ]
    }

    #[tokio::test]
    async fn test_429_failover_and_cooldown() {
        let bus = EventBus::new();
        let sched = NodeScheduler::new(RotationMode::On429Only, 60, bus);
        let mock = Arc::new(MockSwitcher {
            selected: Mutex::new(Vec::new()),
        });
        sched.set_switcher(Some(mock.clone()));
        sched.load_nodes(sample_nodes());

        assert_eq!(sched.current_node_name().as_deref(), Some("Node-A"));

        // Node-A hits 429 -> should rotate to Node-B
        let next = sched
            .report_429("Node-A", "aiplatform.googleapis.com")
            .await;
        assert_eq!(next.as_deref(), Some("Node-B"));

        // Node-B hits 429 -> should rotate to Node-C (skipping cooled Node-A)
        let next2 = sched
            .report_429("Node-B", "aiplatform.googleapis.com")
            .await;
        assert_eq!(next2.as_deref(), Some("Node-C"));

        let snap = sched.snapshot();
        assert_eq!(snap.total_429, 2);
        assert_eq!(snap.ready_count, 1);
        assert_eq!(snap.cooldown_count, 2);
        assert_eq!(*mock.selected.lock().unwrap(), vec!["Node-B", "Node-C"]);
    }

    #[tokio::test]
    async fn test_round_robin_per_request() {
        let bus = EventBus::new();
        let sched = NodeScheduler::new(RotationMode::RoundRobinPerRequest, 60, bus);
        sched.load_nodes(sample_nodes());

        let n1 = sched.prepare_for_request().await;
        let n2 = sched.prepare_for_request().await;
        let n3 = sched.prepare_for_request().await;
        assert_eq!(n1.as_deref(), Some("Node-B"));
        assert_eq!(n2.as_deref(), Some("Node-C"));
        assert_eq!(n3.as_deref(), Some("Node-A"));
    }
}
