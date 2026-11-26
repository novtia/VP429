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
    excluded_node: Option<String>,
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
                excluded_node: None,
                mode,
                cooldown: Duration::from_secs(cooldown_secs.max(1)),
                total_429: 0,
                total_retries_ok: 0,
                switcher: None,
            }),
            bus,
        })
    }

    pub fn set_excluded_node(&self, node_name: Option<String>) {
        if let Ok(mut g) = self.inner.write() {
            g.excluded_node = node_name;
        }
        self.bus.notify_state();
    }

    /// Ensures the active front-end node is not the excluded landing node (avoids chain self-loop).
    pub async fn ensure_not_excluded(&self) -> Option<String> {
        let (target_name, switcher) = {
            let mut g = self.inner.write().ok()?;
            if g.nodes.len() <= 1 {
                return g.nodes.get(g.active_index).map(|n| n.meta.name.clone());
            }
            let Some(ref excl) = g.excluded_node.clone() else {
                return g.nodes.get(g.active_index).map(|n| n.meta.name.clone());
            };
            if g.nodes.get(g.active_index).map(|n| n.meta.name.as_str()) != Some(excl.as_str()) {
                return g.nodes.get(g.active_index).map(|n| n.meta.name.clone());
            }
            let now = Instant::now();
            let next_idx = Self::find_next_ready(&g.nodes, g.active_index, now, Some(excl.as_str()))
                .or_else(|| {
                    (1..=g.nodes.len())
                        .map(|s| (g.active_index + s) % g.nodes.len())
                        .find(|&i| g.nodes[i].meta.name != *excl)
                })
                .unwrap_or(g.active_index);
            g.active_index = next_idx;
            (g.nodes[next_idx].meta.name.clone(), g.switcher.clone())
        };
        self.bus.info(format!(
            "为避免链式代理自环，前置节点已自动切换至 {}",
            target_name
        ));
        let switched = self.apply_switch(target_name, switcher).await;
        self.bus.notify_state();
        Some(switched)
    }

    /// Records a 429 hit when chain proxy is active (fixed exit IP: no cooldown or exit rotation).
    pub fn record_429_fixed_exit(&self, target_host: &str) {
        if let Ok(mut g) = self.inner.write() {
            g.total_429 += 1;
        }
        self.bus.warn_429(format!(
            "{} 命中 429（链式代理已锁定固定出口，不切换出口 IP）",
            target_host
        ));
        self.bus.notify_state();
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

    /// Restores the previously persisted active node if it exists in the current node list.
    pub fn restore_selected_node(&self, node_name: Option<&str>) {
        let Some(target) = node_name else {
            return;
        };
        if let Ok(mut g) = self.inner.write() {
            if let Some(idx) = g.nodes.iter().position(|n| n.meta.name == target) {
                g.active_index = idx;
            }
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
            let excl = g.excluded_node.clone();
            if g.mode == RotationMode::RoundRobinPerRequest {
                if let Some(next_idx) =
                    Self::find_next_ready(&g.nodes, g.active_index, now, excl.as_deref())
                {
                    g.active_index = next_idx;
                }
                (
                    g.nodes[g.active_index].meta.name.clone(),
                    g.switcher.clone(),
                    true,
                )
            } else if !g.nodes[g.active_index].is_ready(now) {
                if let Some(next_idx) =
                    Self::find_next_ready(&g.nodes, g.active_index, now, excl.as_deref())
                {
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

            let excl = g.excluded_node.clone();
            let (next_idx, all_cooled) =
                match Self::find_next_ready(&g.nodes, off_idx, now, excl.as_deref()) {
                    Some(idx) => (idx, false),
                    None => {
                        // Fallback: pick node with earliest cooldown_until (excluding current/excluded if possible)
                        let best = g
                            .nodes
                            .iter()
                            .enumerate()
                            .filter(|(i, n)| {
                                (g.nodes.len() == 1 || *i != off_idx)
                                    && (g.nodes.len() <= 1
                                        || excl.as_deref() != Some(n.meta.name.as_str()))
                            })
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
            let excl = g.excluded_node.clone();
            let next_idx =
                Self::find_next_ready(&g.nodes, g.active_index, now, excl.as_deref()).unwrap_or_else(
                    || {
                        if g.nodes.len() <= 1 {
                            0
                        } else {
                            (1..=g.nodes.len())
                                .map(|s| (g.active_index + s) % g.nodes.len())
                                .find(|&i| excl.as_deref() != Some(g.nodes[i].meta.name.as_str()))
                                .unwrap_or((g.active_index + 1) % g.nodes.len())
                        }
                    },
                );
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

    fn find_next_ready(
        nodes: &[NodeEntry],
        from: usize,
        now: Instant,
        excluded: Option<&str>,
    ) -> Option<usize> {
        let len = nodes.len();
        if len == 0 {
            return None;
        }
        for step in 1..=len {
            let idx = (from + step) % len;
            if len > 1 && excluded == Some(nodes[idx].meta.name.as_str()) {
                continue;
            }
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

    #[tokio::test]
    async fn test_chain_exclusion_and_fixed_exit_429() {
        let bus = EventBus::new();
        let sched = NodeScheduler::new(RotationMode::On429Only, 60, bus);
        sched.load_nodes(sample_nodes());
        assert_eq!(sched.current_node_name().as_deref(), Some("Node-A"));

        // Exclude Node-A (set as landing node) -> front-end automatically avoids self-loop
        sched.set_excluded_node(Some("Node-A".into()));
        let front = sched.ensure_not_excluded().await;
        assert_eq!(front.as_deref(), Some("Node-B"));

        // Rotating front-end skips Node-A
        let r1 = sched.rotate_next("test").await;
        assert_eq!(r1.as_deref(), Some("Node-C"));
        let r2 = sched.rotate_next("test").await;
        assert_eq!(r2.as_deref(), Some("Node-B"));

        // Fixed exit 429 records counter without cooling down or switching
        sched.record_429_fixed_exit("aiplatform.googleapis.com");
        let snap = sched.snapshot();
        assert_eq!(snap.total_429, 1);
        assert_eq!(snap.cooldown_count, 0);
        assert_eq!(snap.active_node.as_deref(), Some("Node-B"));
    }
}
