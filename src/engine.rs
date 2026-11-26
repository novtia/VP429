use crate::config::{
    derive_profile_name, AppConfig, ChainExitSource, RotationMode, SubscriptionProfile,
};
use crate::mihomo::{
    detect_other_vpn_tun, download_mihomo_core, is_elevated, MihomoApiClient, MihomoManager,
};
use crate::mitm::ca::CertAuthority;
use crate::mitm::proxy::{run_proxy_listener, MitmProxyContext};
use crate::scheduler::NodeScheduler;
use crate::state::EventBus;
use crate::subscription::{
    fetch_subscription_to_path, generate_mihomo_config, load_cached_subscription,
    load_cached_subscription_from, parse_custom_exit_proxy, resolve_chain_exit_mapping,
    ParsedSubscription, CHAIN_EXIT_PROXY_NAME, OUTBOUND_GROUP_NAME, POOL_GROUP_NAME,
};
use crate::sysproxy;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::VecDeque;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex};

#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    pub up_bps: u64,
    pub down_bps: u64,
    pub total_up: u64,
    pub total_down: u64,
    pub speed_history: Vec<(u64, u64)>,
    pub uptime_secs: u64,
    pub local_ip: String,
    pub exit_ip: Option<String>,
    pub exit_country_code: Option<String>,
    pub checking_ip: bool,
    pub outbound_mode: String,
}

struct TelemetryInner {
    up_bps: u64,
    down_bps: u64,
    total_up: u64,
    total_down: u64,
    speed_history: VecDeque<(u64, u64)>,
    started_at: Option<Instant>,
    local_ip: String,
    exit_ip: Option<String>,
    exit_country_code: Option<String>,
    checking_ip: bool,
    outbound_mode: String,
}

#[derive(Deserialize)]
struct TrafficSample {
    up: u64,
    down: u64,
}

#[derive(Deserialize)]
struct IpApiResp {
    query: Option<String>,
    #[serde(rename = "countryCode")]
    country_code: Option<String>,
}

fn detect_local_ip() -> String {
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("223.5.5.5:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

pub struct ProxyEngine {
    pub config: Arc<RwLock<AppConfig>>,
    pub bus: Arc<EventBus>,
    pub scheduler: Arc<NodeScheduler>,
    pub ca: Arc<CertAuthority>,
    mihomo: Arc<MihomoManager>,
    api_client: RwLock<Option<MihomoApiClient>>,
    runtime_secret: RwLock<Option<String>>,
    runtime_has_chain_exit: AtomicBool,
    subscription: RwLock<Option<ParsedSubscription>>,
    telemetry: RwLock<TelemetryInner>,
    running: AtomicBool,
    testing_latency: AtomicBool,
    shutdown_tx: Mutex<Option<watch::Sender<bool>>>,
    ca_installed: Arc<AtomicBool>,
}

impl ProxyEngine {
    pub fn new(config: AppConfig) -> Result<Arc<Self>> {
        let bus = EventBus::new();
        let scheduler = NodeScheduler::new(
            config.rotation_mode.clone(),
            config.cooldown_seconds,
            bus.clone(),
        );
        let ca = CertAuthority::load_or_create()?;
        let ca_installed = ca.is_installed_windows();
        let mihomo = Arc::new(MihomoManager::new(bus.clone()));

        let cached_sub = config
            .active_profile_id
            .as_deref()
            .and_then(|id| load_cached_subscription_from(&AppConfig::profile_cache_path(id)))
            .or_else(load_cached_subscription);
        if let Some(ref sub) = cached_sub {
            scheduler.load_nodes(sub.nodes.clone());
            scheduler.restore_selected_node(config.selected_node.as_deref());
            if config.enable_chain_proxy {
                if let Some(ChainExitSource::SubscriptionNode { ref name }) =
                    config.chain_exit_source
                {
                    scheduler.set_excluded_node(Some(name.clone()));
                }
            }
            bus.info(format!("已从本地缓存加载 {} 个节点", sub.nodes.len()));
        } else {
            bus.info("欢迎使用 VP429，请前往「配置文件」点击右下角 + 添加订阅 URL");
        }

        let initial_outbound_mode = match config.outbound_mode.as_str() {
            "rule" | "global" | "direct" => config.outbound_mode.clone(),
            _ => "rule".to_string(),
        };

        Ok(Arc::new(Self {
            config: Arc::new(RwLock::new(config)),
            bus,
            scheduler,
            ca,
            mihomo,
            api_client: RwLock::new(None),
            runtime_secret: RwLock::new(None),
            runtime_has_chain_exit: AtomicBool::new(false),
            subscription: RwLock::new(cached_sub),
            telemetry: RwLock::new(TelemetryInner {
                up_bps: 0,
                down_bps: 0,
                total_up: 0,
                total_down: 0,
                speed_history: VecDeque::from(vec![(0, 0); 36]),
                started_at: None,
                local_ip: detect_local_ip(),
                exit_ip: None,
                exit_country_code: None,
                checking_ip: false,
                outbound_mode: initial_outbound_mode,
            }),
            running: AtomicBool::new(false),
            testing_latency: AtomicBool::new(false),
            shutdown_tx: Mutex::new(None),
            ca_installed: Arc::new(AtomicBool::new(ca_installed)),
        }))
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn is_testing_latency(&self) -> bool {
        self.testing_latency.load(Ordering::SeqCst)
    }

    pub fn telemetry_snapshot(&self) -> TelemetrySnapshot {
        let g = self.telemetry.read().unwrap();
        let uptime_secs = g
            .started_at
            .map(|s| s.elapsed().as_secs())
            .unwrap_or(0);
        TelemetrySnapshot {
            up_bps: g.up_bps,
            down_bps: g.down_bps,
            total_up: g.total_up,
            total_down: g.total_down,
            speed_history: g.speed_history.iter().copied().collect(),
            uptime_secs,
            local_ip: g.local_ip.clone(),
            exit_ip: g.exit_ip.clone(),
            exit_country_code: g.exit_country_code.clone(),
            checking_ip: g.checking_ip,
            outbound_mode: g.outbound_mode.clone(),
        }
    }

    pub async fn select_node_manual(&self, node_name: &str) -> Option<String> {
        let switched = self.scheduler.select_manual(node_name).await;
        if let Some(ref name) = switched {
            if let Ok(mut cfg) = self.config.write() {
                cfg.selected_node = Some(name.clone());
            }
            self.save_config();
        }
        switched
    }

    pub async fn set_outbound_mode(&self, mode: &str) {
        {
            let mut t = self.telemetry.write().unwrap();
            t.outbound_mode = mode.to_string();
        }
        if let Ok(mut cfg) = self.config.write() {
            cfg.outbound_mode = mode.to_string();
        }
        self.save_config();
        let api = self.api_client.read().unwrap().clone();
        if let Some(api) = api {
            // Note: our `rule` config already routes all non-LAN traffic to VERTEX-POOL while
            // keeping watch_domains routed through RUST-429-MITM. Switching Mihomo itself to
            // `global` would skip `rules:` and bypass TUN 429 MITM interception!
            let mihomo_mode = if mode == "direct" { "direct" } else { "rule" };
            if let Err(e) = api.set_mode(mihomo_mode).await {
                self.bus.error(format!("切换出站模式失败: {}", e));
            } else {
                if mode == "global" {
                    let _ = api.select_proxy_group("GLOBAL", OUTBOUND_GROUP_NAME).await;
                    self.bus
                        .info("出站模式已切换为: global（全局代理 + 保留 429 MITM 拦截）");
                } else {
                    self.bus.info(format!("出站模式已切换为: {}", mode));
                }
            }
        }
        self.bus.notify_state();
    }

    pub async fn check_exit_ip(&self) {
        {
            let mut t = self.telemetry.write().unwrap();
            if t.checking_ip {
                return;
            }
            t.checking_ip = true;
        }
        self.bus.notify_state();

        let (running, proxy_port) = (
            self.is_running(),
            self.config.read().unwrap().local_proxy_port,
        );
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(6));
        if running {
            if let Ok(proxy) = reqwest::Proxy::all(format!("http://127.0.0.1:{}", proxy_port)) {
                builder = builder.proxy(proxy);
            }
        } else {
            builder = builder.no_proxy();
        }

        let result = async {
            let client = builder.build()?;
            let resp: IpApiResp = client
                .get("http://ip-api.com/json/?fields=status,countryCode,query")
                .send()
                .await?
                .json()
                .await?;
            Ok::<_, anyhow::Error>(resp)
        }
        .await;

        {
            let mut t = self.telemetry.write().unwrap();
            t.checking_ip = false;
            match result {
                Ok(info) => {
                    t.exit_ip = info.query;
                    t.exit_country_code = info.country_code;
                }
                Err(_) => {
                    t.exit_ip = Some("检测失败".to_string());
                    t.exit_country_code = None;
                }
            }
        }
        self.bus.notify_state();
    }

    pub fn is_ca_installed(&self) -> bool {
        self.ca_installed.load(Ordering::SeqCst)
    }

    pub fn save_config(&self) {
        if let Ok(cfg) = self.config.read() {
            if let Err(e) = cfg.save() {
                self.bus.error(format!("保存配置失败: {}", e));
            }
        }
    }

    pub fn update_rotation_policy(&self, mode: RotationMode, cooldown_secs: u64, auto_retry: bool, max_retries: usize) {
        if let Ok(mut cfg) = self.config.write() {
            cfg.rotation_mode = mode.clone();
            cfg.cooldown_seconds = cooldown_secs.max(5);
            cfg.auto_retry_on_429 = auto_retry;
            cfg.max_retries = max_retries.min(10);
        }
        self.scheduler.update_policy(mode, cooldown_secs.max(5));
        self.save_config();
    }

    pub fn update_watch_domains(&self, domains: Vec<String>) {
        if let Ok(mut cfg) = self.config.write() {
            cfg.watch_domains = domains;
        }
        self.save_config();
        self.bus.info("已更新选择性 HTTPS MITM 监听域名（重启代理后同步至 TUN 规则）");
        self.bus.notify_state();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_system_settings(
        &self,
        local_port: u16,
        allow_lan: bool,
        mixed_port: u16,
        tun_mitm_port: u16,
        bypass_port: u16,
        api_port: u16,
        max_body_mb: usize,
        mihomo_path: Option<String>,
    ) -> Result<()> {
        let ports = [local_port, mixed_port, tun_mitm_port, bypass_port, api_port];
        for &p in &ports {
            if p < 1024 {
                return Err(anyhow!("端口号需在 1024 - 65535 之间 (当前包含 {})", p));
            }
        }
        let mut uniq = ports.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        if uniq.len() != ports.len() {
            return Err(anyhow!("5 个服务端口不能互相重复，请检查端口配置"));
        }

        let sys_proxy_enabled = {
            let mut cfg = self.config.write().unwrap();
            cfg.local_proxy_port = local_port;
            cfg.allow_lan = allow_lan;
            cfg.mihomo_mixed_port = mixed_port;
            cfg.tun_mitm_port = tun_mitm_port;
            cfg.mihomo_bypass_port = bypass_port;
            cfg.mihomo_api_port = api_port;
            cfg.max_retry_body_bytes = max_body_mb.clamp(1, 256) * 1024 * 1024;
            cfg.mihomo_path = mihomo_path.filter(|s| !s.trim().is_empty());
            cfg.enable_system_proxy
        };
        self.save_config();

        if self.is_running() {
            if sys_proxy_enabled {
                let _ = sysproxy::enable_system_proxy(local_port);
            }
            self.bus
                .ok("系统设置已保存（端口变更将在下次重启代理服务后生效）");
        } else {
            self.bus.ok("系统设置已保存");
        }
        self.bus.notify_state();
        Ok(())
    }

    pub fn install_ca(&self) {
        match self.ca.install_windows_user_root() {
            Ok(msg) => {
                self.ca_installed.store(true, Ordering::SeqCst);
                self.bus.ok(msg);
            }
            Err(err) => {
                self.bus.error(format!("{}", err));
            }
        }
        self.bus.notify_state();
    }

    #[allow(dead_code)]
    pub fn active_profile(&self) -> Option<SubscriptionProfile> {
        let cfg = self.config.read().unwrap();
        let active_id = cfg.active_profile_id.as_deref()?;
        cfg.profiles.iter().find(|p| p.id == active_id).cloned()
    }

    pub async fn add_and_sync_profile(&self, raw_name: String, url: String) -> Result<usize> {
        let trimmed_url = url.trim().to_string();
        if trimmed_url.is_empty() {
            return Err(anyhow!("订阅 URL 不能为空"));
        }
        let name = if raw_name.trim().is_empty() {
            derive_profile_name(&trimmed_url)
        } else {
            raw_name.trim().to_string()
        };
        let id = format!("prof_{:x}", rand::random::<u64>());
        let cache_path = AppConfig::profile_cache_path(&id);

        let fallback_port = if self.is_running() {
            Some(self.config.read().unwrap().local_proxy_port)
        } else {
            None
        };
        self.bus.info(format!("正在添加并拉取配置「{}」...", name));
        let (parsed, meta) =
            fetch_subscription_to_path(&trimmed_url, &cache_path, fallback_port).await?;
        let final_name = if raw_name.trim().is_empty() {
            meta.suggested_name.unwrap_or(name)
        } else {
            name
        };
        let count = parsed.nodes.len();
        let now_str = chrono::Local::now().format("%m-%d %H:%M").to_string();
        let now_unix = chrono::Utc::now().timestamp();

        {
            let mut cfg = self.config.write().unwrap();
            cfg.subscription_url = trimmed_url.clone();
            cfg.profiles.push(SubscriptionProfile {
                id: id.clone(),
                name: final_name.clone(),
                url: trimmed_url,
                node_count: count,
                updated_at: now_str,
                updated_unix: now_unix,
                used_bytes: meta.upload.saturating_add(meta.download),
                total_bytes: meta.total,
                expire_unix: meta.expire,
            });
            cfg.active_profile_id = Some(id);
        }
        self.save_config();
        self.scheduler.load_nodes(parsed.nodes.clone());
        *self.subscription.write().unwrap() = Some(parsed);
        self.bus
            .ok(format!("配置「{}」已激活，共 {} 个可用节点", final_name, count));
        self.bus.notify_state();
        Ok(count)
    }

    pub async fn activate_profile(&self, id: &str) -> Result<usize> {
        let (name, url) = {
            let cfg = self.config.read().unwrap();
            let p = cfg
                .profiles
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| anyhow!("未找到指定配置文件"))?;
            (p.name.clone(), p.url.clone())
        };

        let cache_path = AppConfig::profile_cache_path(id);
        let (parsed, fetched_meta) = match load_cached_subscription_from(&cache_path) {
            Some(p) => (p, None),
            None => {
                self.bus.info(format!("本地无缓存，正在拉取「{}」...", name));
                let fallback_port = if self.is_running() {
                    Some(self.config.read().unwrap().local_proxy_port)
                } else {
                    None
                };
                let (p, m) = fetch_subscription_to_path(&url, &cache_path, fallback_port).await?;
                (p, Some(m))
            }
        };
        let count = parsed.nodes.len();
        {
            let mut cfg = self.config.write().unwrap();
            cfg.active_profile_id = Some(id.to_string());
            cfg.subscription_url = url;
            if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == id) {
                p.node_count = count;
                if let Some(m) = fetched_meta {
                    p.used_bytes = m.upload.saturating_add(m.download);
                    p.total_bytes = m.total;
                    p.expire_unix = m.expire;
                    p.updated_unix = chrono::Utc::now().timestamp();
                    p.updated_at = chrono::Local::now().format("%m-%d %H:%M").to_string();
                }
            }
        }
        let saved_node = self.config.read().unwrap().selected_node.clone();
        self.save_config();
        self.scheduler.load_nodes(parsed.nodes.clone());
        self.scheduler.restore_selected_node(saved_node.as_deref());
        *self.subscription.write().unwrap() = Some(parsed);
        self.bus.ok(format!("已切换至配置「{}」({} 节点)", name, count));
        self.bus.notify_state();
        Ok(count)
    }

    pub async fn refresh_profile(&self, id: &str) -> Result<usize> {
        let (name, url, is_active) = {
            let cfg = self.config.read().unwrap();
            let p = cfg
                .profiles
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| anyhow!("未找到指定配置文件"))?;
            (
                p.name.clone(),
                p.url.clone(),
                cfg.active_profile_id.as_deref() == Some(id),
            )
        };

        self.bus.info(format!("正在更新配置「{}」...", name));
        let cache_path = AppConfig::profile_cache_path(id);
        let fallback_port = if self.is_running() {
            Some(self.config.read().unwrap().local_proxy_port)
        } else {
            None
        };
        let (parsed, meta) = fetch_subscription_to_path(&url, &cache_path, fallback_port).await?;
        let count = parsed.nodes.len();
        let now_str = chrono::Local::now().format("%m-%d %H:%M").to_string();
        let now_unix = chrono::Utc::now().timestamp();

        {
            let mut cfg = self.config.write().unwrap();
            if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == id) {
                if let Some(ref suggested) = meta.suggested_name {
                    if p.name.contains('.') && !suggested.is_empty() {
                        p.name = suggested.clone();
                    }
                }
                p.node_count = count;
                p.updated_at = now_str;
                p.updated_unix = now_unix;
                if meta.total > 0 {
                    p.used_bytes = meta.upload.saturating_add(meta.download);
                    p.total_bytes = meta.total;
                }
                if meta.expire > 0 {
                    p.expire_unix = meta.expire;
                }
            }
        }
        self.save_config();
        if is_active {
            let saved_node = self.config.read().unwrap().selected_node.clone();
            self.scheduler.load_nodes(parsed.nodes.clone());
            self.scheduler.restore_selected_node(saved_node.as_deref());
            *self.subscription.write().unwrap() = Some(parsed);
        }
        self.bus.ok(format!("配置「{}」更新完成，共 {} 个节点", name, count));
        self.bus.notify_state();
        Ok(count)
    }

    pub fn delete_profile(&self, id: &str) {
        let removed_name = {
            let mut cfg = self.config.write().unwrap();
            let name = cfg
                .profiles
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone());
            cfg.profiles.retain(|p| p.id != id);
            if cfg.active_profile_id.as_deref() == Some(id) {
                cfg.active_profile_id = cfg.profiles.first().map(|p| p.id.clone());
            }
            name
        };
        let _ = fs::remove_file(AppConfig::profile_cache_path(id));
        self.save_config();
        if let Some(name) = removed_name {
            self.bus.info(format!("已删除配置文件「{}」", name));
        }
        self.bus.notify_state();
    }

    pub async fn download_core(&self) -> Result<()> {
        let path = download_mihomo_core(&self.bus).await?;
        {
            let mut cfg = self.config.write().unwrap();
            cfg.mihomo_path = Some(path.to_string_lossy().to_string());
        }
        self.save_config();
        self.bus.notify_state();
        Ok(())
    }

    pub async fn start(self: &Arc<Self>) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        let cfg = self.config.read().unwrap().clone();
        let sub = self
            .subscription
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow!("请先同步有效的 Clash 订阅 URL"))?;

        let effective_tun = cfg.enable_tun && is_elevated();
        let enable_mitm = self.is_ca_installed();

        if cfg.enable_tun && !is_elevated() {
            self.bus.warn_429(
                "当前非管理员权限，本次已自动关闭 TUN（仍使用系统代理）；如需 TUN 请点击仪表盘「提权」以管理员身份重启",
            );
        }
        if !enable_mitm {
            self.bus.info(
                "本地根证书未安装，已自动使用纯代理直通模式（如需启用 429 自动重试拦截，请前往「429 策略」安装根证书后重启代理）",
            );
        }

        let mihomo_bin = MihomoManager::resolve_binary_path(&cfg)
            .ok_or_else(|| anyhow!("未找到 mihomo.exe，请前往「429 轮询与证书」点击下载内核"))?;

        let secret = format!("vp429_{:x}", rand::random::<u64>());
        if let Some(other_tun) = detect_other_vpn_tun() {
            self.bus.warn_429(format!(
                "检测到其他 VPN 网卡 ({}) 正在运行，建议先关闭以避免双层代理",
                other_tun
            ));
        }
        let runtime_yaml =
            generate_mihomo_config(&sub, &cfg, &secret, enable_mitm, effective_tun)?;
        let has_chain_exit = cfg
            .chain_exit_source
            .as_ref()
            .map(|s| resolve_chain_exit_mapping(&sub, s).is_ok())
            .unwrap_or(false);
        let runtime_path = AppConfig::runtime_yaml_path();
        fs::write(&runtime_path, runtime_yaml)?;

        self.bus.info("正在启动 Mihomo 内核...");
        self.mihomo.start(&mihomo_bin, &runtime_path).await?;

        let api = MihomoApiClient::new(cfg.mihomo_api_port, secret.clone());
        let mut ready = false;
        for _ in 0..25 {
            if api.health_check().await {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        if !ready {
            self.mihomo.stop().await;
            return Err(anyhow!("Mihomo 内核启动超时，请检查日志"));
        }

        *self.api_client.write().unwrap() = Some(api.clone());
        *self.runtime_secret.write().unwrap() = Some(secret);
        self.runtime_has_chain_exit
            .store(has_chain_exit, Ordering::SeqCst);
        self.scheduler.set_switcher(Some(Arc::new(api.clone())));
        // Avoid self-loop if chain proxy is enabled on startup with a subscription landing node
        if cfg.enable_chain_proxy && has_chain_exit {
            if let Some(ChainExitSource::SubscriptionNode { ref name }) = cfg.chain_exit_source {
                self.scheduler.set_excluded_node(Some(name.clone()));
                let _ = self.scheduler.ensure_not_excluded().await;
            }
        } else {
            self.scheduler.set_excluded_node(None);
        }
        // Point Mihomo's built-in GLOBAL group to VERTEX-OUTBOUND
        let _ = api.select_proxy_group("GLOBAL", OUTBOUND_GROUP_NAME).await;
        let target_outbound = if cfg.enable_chain_proxy && has_chain_exit {
            CHAIN_EXIT_PROXY_NAME
        } else {
            POOL_GROUP_NAME
        };
        let _ = api
            .select_proxy_group(OUTBOUND_GROUP_NAME, target_outbound)
            .await;
        if let Some(cur) = self.scheduler.current_node_name() {
            let _ = api.select_proxy_group(POOL_GROUP_NAME, &cur).await;
        }

        let (shut_tx, shut_rx) = watch::channel(false);
        *self.shutdown_tx.lock().await = Some(shut_tx);

        let ctx = MitmProxyContext {
            config: self.config.clone(),
            ca: self.ca.clone(),
            ca_installed: self.ca_installed.clone(),
            scheduler: self.scheduler.clone(),
            bus: self.bus.clone(),
            upstream_tls: MitmProxyContext::build_default_upstream_tls(Vec::new()),
        };

        // Start user-facing proxy listener (:7890 by default; 0.0.0.0 if allow_lan)
        let bind_ip = if cfg.allow_lan { "0.0.0.0" } else { "127.0.0.1" };
        let local_addr = format!("{}:{}", bind_ip, cfg.local_proxy_port);
        let ctx1 = ctx.clone();
        let rx1 = shut_rx.clone();
        tokio::spawn(async move {
            let _ = run_proxy_listener(local_addr, false, ctx1, rx1).await;
        });

        // Start internal TUN-routed MITM listener (:17891)
        let tun_mitm_addr = format!("127.0.0.1:{}", cfg.tun_mitm_port);
        let ctx2 = ctx.clone();
        let rx2 = shut_rx.clone();
        tokio::spawn(async move {
            let _ = run_proxy_listener(tun_mitm_addr, true, ctx2, rx2).await;
        });

        // Background interval rotation worker
        let engine_weak = Arc::downgrade(self);
        let mut rx3 = shut_rx.clone();
        tokio::spawn(async move {
            let mut elapsed_secs: u64 = 0;
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = rx3.changed() => {
                        if *rx3.borrow() { break; }
                    }
                    _ = ticker.tick() => {
                        let Some(eng) = engine_weak.upgrade() else { break; };
                        let (mode, chain_active) = {
                            let c = eng.config.read().unwrap();
                            (
                                c.rotation_mode.clone(),
                                c.enable_chain_proxy && c.chain_exit_source.is_some(),
                            )
                        };
                        if !chain_active {
                            if let RotationMode::RoundRobinInterval { seconds } = mode {
                                elapsed_secs += 1;
                                if elapsed_secs >= seconds.max(5) {
                                    elapsed_secs = 0;
                                    let _ = eng.scheduler.rotate_next("定时轮询切换").await;
                                }
                            } else {
                                elapsed_secs = 0;
                            }
                        } else {
                            elapsed_secs = 0;
                        }
                    }
                }
            }
        });

        if cfg.enable_system_proxy {
            if let Err(e) = sysproxy::enable_system_proxy(cfg.local_proxy_port) {
                self.bus.error(format!("设置 Windows 系统代理失败: {}", e));
            } else {
                self.bus.ok(format!(
                    "已启用 Windows 系统代理 -> 127.0.0.1:{}",
                    cfg.local_proxy_port
                ));
            }
        }

        self.running.store(true, Ordering::SeqCst);
        let init_mode = {
            let mut t = self.telemetry.write().unwrap();
            t.started_at = Some(Instant::now());
            t.up_bps = 0;
            t.down_bps = 0;
            t.local_ip = detect_local_ip();
            t.outbound_mode.clone()
        };
        if init_mode == "direct" {
            let _ = api.set_mode("direct").await;
        }

        // Stream live bandwidth from Mihomo `/traffic` endpoint
        let engine_traffic = Arc::downgrade(self);
        let traffic_url = api.traffic_url();
        let traffic_secret = api.secret().to_string();
        let mut rx_traffic = shut_rx.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let mut req = client.get(&traffic_url);
            if !traffic_secret.is_empty() {
                req = req.bearer_auth(&traffic_secret);
            }
            if let Ok(mut resp) = req.send().await {
                let mut buf = Vec::new();
                loop {
                    tokio::select! {
                        _ = rx_traffic.changed() => {
                            if *rx_traffic.borrow() { break; }
                        }
                        chunk = resp.chunk() => {
                            let Ok(Some(bytes)) = chunk else { break; };
                            buf.extend_from_slice(&bytes);
                            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                                let line: Vec<u8> = buf.drain(..=pos).collect();
                                if let Ok(sample) = serde_json::from_slice::<TrafficSample>(&line) {
                                    let Some(eng) = engine_traffic.upgrade() else { return; };
                                    if let Ok(mut t) = eng.telemetry.write() {
                                        t.up_bps = sample.up;
                                        t.down_bps = sample.down;
                                        t.total_up += sample.up;
                                        t.total_down += sample.down;
                                        if t.speed_history.len() >= 36 {
                                            t.speed_history.pop_front();
                                        }
                                        t.speed_history.push_back((sample.up, sample.down));
                                    };
                                }
                            }
                        }
                    }
                }
            }
        });

        // Auto-detect exit IP shortly after start
        let eng_ip = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(600)).await;
            eng_ip.check_exit_ip().await;
        });

        self.bus.ok(format!(
            "代理服务已就绪 (本地 :{}, Bypass :{}, TUN: {}, MITM: {})",
            cfg.local_proxy_port,
            cfg.mihomo_bypass_port,
            if effective_tun {
                "ON"
            } else if cfg.enable_tun {
                "OFF/需管理员"
            } else {
                "OFF"
            },
            if enable_mitm { "ON" } else { "直通" }
        ));
        self.bus.notify_state();
        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(true);
        }
        self.mihomo.stop().await;
        self.scheduler.set_switcher(None);
        *self.api_client.write().unwrap() = None;
        *self.runtime_secret.write().unwrap() = None;
        self.runtime_has_chain_exit.store(false, Ordering::SeqCst);
        let _ = sysproxy::disable_system_proxy();
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut t) = self.telemetry.write() {
            t.started_at = None;
            t.up_bps = 0;
            t.down_bps = 0;
        }
        self.bus.info("代理服务已停止，系统代理已还原");
        self.bus.notify_state();
    }

    pub async fn toggle_system_proxy(&self, enabled: bool) {
        let port = {
            let mut cfg = self.config.write().unwrap();
            cfg.enable_system_proxy = enabled;
            cfg.local_proxy_port
        };
        self.save_config();
        if self.is_running() {
            if enabled {
                if let Err(e) = sysproxy::enable_system_proxy(port) {
                    self.bus.error(format!("开启系统代理失败: {}", e));
                } else {
                    self.bus.ok(format!("系统代理已开启 -> 127.0.0.1:{}", port));
                }
            } else {
                let _ = sysproxy::disable_system_proxy();
                self.bus.info("系统代理已关闭");
            }
        }
        self.bus.notify_state();
    }

    /// Regenerates `mihomo-runtime.yaml` and hot-reloads the running Mihomo instance via REST API.
    async fn reload_mihomo_runtime(&self) -> Result<()> {
        let api = self.api_client.read().unwrap().clone();
        let secret = self.runtime_secret.read().unwrap().clone();
        let (Some(api), Some(secret)) = (api, secret) else {
            return Ok(());
        };
        let sub = self
            .subscription
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow!("当前无有效订阅配置"))?;
        let cfg = self.config.read().unwrap().clone();
        let effective_tun = cfg.enable_tun && is_elevated();
        let enable_mitm = self.is_ca_installed();

        let has_chain_exit = cfg
            .chain_exit_source
            .as_ref()
            .map(|s| resolve_chain_exit_mapping(&sub, s).is_ok())
            .unwrap_or(false);
        let runtime_yaml =
            generate_mihomo_config(&sub, &cfg, &secret, enable_mitm, effective_tun)?;
        let runtime_path = AppConfig::runtime_yaml_path();
        fs::write(&runtime_path, runtime_yaml)?;

        api.reload_config(&runtime_path).await?;
        self.runtime_has_chain_exit
            .store(has_chain_exit, Ordering::SeqCst);

        // Re-apply outbound mode & active proxy selections after hot-reload
        let _ = api.select_proxy_group("GLOBAL", OUTBOUND_GROUP_NAME).await;
        let target_outbound = if cfg.enable_chain_proxy && has_chain_exit {
            CHAIN_EXIT_PROXY_NAME
        } else {
            POOL_GROUP_NAME
        };
        let _ = api
            .select_proxy_group(OUTBOUND_GROUP_NAME, target_outbound)
            .await;
        if let Some(cur) = self.scheduler.current_node_name() {
            let _ = api.select_proxy_group(POOL_GROUP_NAME, &cur).await;
        }
        if cfg.outbound_mode == "direct" {
            let _ = api.set_mode("direct").await;
        }
        Ok(())
    }

    pub async fn toggle_chain_proxy(self: &Arc<Self>, enabled: bool) -> Result<()> {
        let exit_src = self.config.read().unwrap().chain_exit_source.clone();
        if enabled {
            let Some(ref src) = exit_src else {
                let msg = "尚未指定固定落地节点：请先在「节点列表」点击「设为落地」，或在「429 策略」配置自定义落地";
                self.bus.warn_429(msg);
                return Err(anyhow!(msg));
            };
            if let Some(ref sub) = *self.subscription.read().unwrap() {
                if let Err(e) = resolve_chain_exit_mapping(sub, src) {
                    self.bus.error(format!("落地节点不可用: {}", e));
                    return Err(e);
                }
            }
        }

        {
            let mut cfg = self.config.write().unwrap();
            cfg.enable_chain_proxy = enabled;
        }
        self.save_config();

        if enabled {
            if let Some(ChainExitSource::SubscriptionNode { ref name }) = exit_src {
                self.scheduler.set_excluded_node(Some(name.clone()));
                let _ = self.scheduler.ensure_not_excluded().await;
            } else {
                self.scheduler.set_excluded_node(None);
            }
        } else {
            self.scheduler.set_excluded_node(None);
        }

        if self.is_running() {
            if enabled && !self.runtime_has_chain_exit.load(Ordering::SeqCst) {
                if let Err(e) = self.reload_mihomo_runtime().await {
                    self.bus
                        .error(format!("热重载链式代理配置失败（请尝试重启代理）: {}", e));
                    return Err(e);
                }
            } else {
                let api = self.api_client.read().unwrap().clone();
                if let Some(api) = api {
                    let target = if enabled {
                        CHAIN_EXIT_PROXY_NAME
                    } else {
                        POOL_GROUP_NAME
                    };
                    if let Err(e) = api.select_proxy_group(OUTBOUND_GROUP_NAME, target).await {
                        self.bus.error(format!("热切换链式代理失败: {}", e));
                        return Err(e);
                    }
                }
            }

            let eng_ip = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(450)).await;
                eng_ip.check_exit_ip().await;
            });
        }

        if enabled {
            let label = exit_src
                .map(|s| s.short_label())
                .unwrap_or_else(|| "未知".to_string());
            self.bus.ok(format!(
                "链式代理已开启：前置池 -> 固定落地 [{}]（429 不切出口 IP）",
                label
            ));
        } else {
            self.bus
                .info("链式代理已关闭：恢复直出节点池（429 自动切换出口节点）");
        }
        self.bus.notify_state();
        Ok(())
    }

    pub async fn set_chain_exit_subscription_node(
        self: &Arc<Self>,
        node_name: &str,
    ) -> Result<()> {
        let was_enabled = {
            let mut cfg = self.config.write().unwrap();
            cfg.chain_exit_source = Some(ChainExitSource::SubscriptionNode {
                name: node_name.to_string(),
            });
            cfg.enable_chain_proxy
        };
        self.save_config();

        if was_enabled {
            self.scheduler.set_excluded_node(Some(node_name.to_string()));
            let _ = self.scheduler.ensure_not_excluded().await;
        }

        if self.is_running() {
            if let Err(e) = self.reload_mihomo_runtime().await {
                self.bus
                    .warn_429(format!("落地节点已保存，但热重载失败（重启代理后生效）: {}", e));
            } else if was_enabled {
                let eng_ip = self.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(450)).await;
                    eng_ip.check_exit_ip().await;
                });
            }
        }
        self.bus
            .ok(format!("已将订阅节点「{}」设为链式固定落地出口", node_name));
        self.bus.notify_state();
        Ok(())
    }

    pub async fn set_chain_exit_custom(
        self: &Arc<Self>,
        raw_spec: String,
        backoff_ms: u64,
    ) -> Result<()> {
        parse_custom_exit_proxy(&raw_spec)?;
        let was_enabled = {
            let mut cfg = self.config.write().unwrap();
            cfg.chain_exit_source = Some(ChainExitSource::CustomSpec {
                raw: raw_spec.trim().to_string(),
            });
            cfg.chain_429_backoff_ms = backoff_ms.clamp(100, 30_000);
            cfg.enable_chain_proxy
        };
        self.save_config();
        self.scheduler.set_excluded_node(None);

        if self.is_running() {
            if let Err(e) = self.reload_mihomo_runtime().await {
                self.bus
                    .warn_429(format!("自定义落地已保存，但热重载失败（重启代理后生效）: {}", e));
            } else if was_enabled {
                let eng_ip = self.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(450)).await;
                    eng_ip.check_exit_ip().await;
                });
            }
        }
        self.bus.ok("自定义链式落地节点已保存并生效");
        self.bus.notify_state();
        Ok(())
    }

    pub async fn clear_chain_exit(self: &Arc<Self>) -> Result<()> {
        let was_enabled = {
            let mut cfg = self.config.write().unwrap();
            let prev = cfg.enable_chain_proxy;
            cfg.enable_chain_proxy = false;
            cfg.chain_exit_source = None;
            prev
        };
        self.save_config();
        self.scheduler.set_excluded_node(None);

        if self.is_running() {
            let api = self.api_client.read().unwrap().clone();
            if let Some(api) = api {
                let _ = api
                    .select_proxy_group(OUTBOUND_GROUP_NAME, POOL_GROUP_NAME)
                    .await;
            }
            if was_enabled {
                let eng_ip = self.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(450)).await;
                    eng_ip.check_exit_ip().await;
                });
            }
        }
        self.bus.info("已清除固定落地节点配置并关闭链式代理");
        self.bus.notify_state();
        Ok(())
    }

    pub fn update_chain_backoff_ms(&self, backoff_ms: u64) {
        if let Ok(mut cfg) = self.config.write() {
            cfg.chain_429_backoff_ms = backoff_ms.clamp(100, 30_000);
        }
        self.save_config();
        self.bus.notify_state();
    }

    pub async fn ping_all_nodes(&self) {
        if self
            .testing_latency
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        self.bus.notify_state();

        let nodes = self.scheduler.snapshot().nodes;
        if nodes.is_empty() {
            self.testing_latency.store(false, Ordering::SeqCst);
            self.bus
                .warn_429("当前没有可测速的节点，请先在「配置文件」中添加并同步订阅");
            self.bus.notify_state();
            return;
        }

        // Reuse running Mihomo API if active; otherwise spawn a temporary headless Mihomo
        // (with TUN disabled) so URL-test measures real end-to-end tunnel latency just like FlClash.
        let mut temp_mihomo: Option<MihomoManager> = None;
        let existing_api = self.api_client.read().unwrap().clone();
        let api = if let Some(existing) = existing_api {
            existing
        } else {
            let cfg = self.config.read().unwrap().clone();
            let Some(mihomo_bin) = MihomoManager::resolve_binary_path(&cfg) else {
                self.testing_latency.store(false, Ordering::SeqCst);
                self.bus
                    .error("未找到 mihomo.exe，请先前往「429 策略」一键下载内核后再测速");
                self.bus.notify_state();
                return;
            };
            let Some(sub) = self.subscription.read().unwrap().clone() else {
                self.testing_latency.store(false, Ordering::SeqCst);
                self.bus.error("请先同步订阅配置后再测速");
                self.bus.notify_state();
                return;
            };

            let temp_api_port = cfg.mihomo_api_port.saturating_add(10);
            let mut temp_cfg = cfg.clone();
            temp_cfg.enable_tun = false;
            temp_cfg.mihomo_api_port = temp_api_port;
            temp_cfg.mihomo_mixed_port = cfg.mihomo_mixed_port.saturating_add(10);
            temp_cfg.mihomo_bypass_port = cfg.mihomo_bypass_port.saturating_add(10);

            let secret = format!("vp429_ping_{:x}", rand::random::<u64>());
            if let Some(other_tun) = detect_other_vpn_tun() {
                self.bus.warn_429(format!(
                    "检测到 {} 正在运行，当前测速会叠加双层代理延迟；关闭其他 VPN 后即为真实延迟",
                    other_tun
                ));
            }
            let temp_yaml_path = AppConfig::data_dir().join("mihomo-ping-temp.yaml");
            match generate_mihomo_config(&sub, &temp_cfg, &secret, false, false)
                .and_then(|y| fs::write(&temp_yaml_path, y).map_err(Into::into))
            {
                Ok(()) => {}
                Err(e) => {
                    self.testing_latency.store(false, Ordering::SeqCst);
                    self.bus.error(format!("生成临时测速配置失败: {}", e));
                    self.bus.notify_state();
                    return;
                }
            }

            self.bus.info("正在启动临时 Mihomo 实例进行真连接 URL 测速...");
            let mgr = MihomoManager::new(self.bus.clone());
            if let Err(e) = mgr.start(&mihomo_bin, &temp_yaml_path).await {
                self.testing_latency.store(false, Ordering::SeqCst);
                self.bus.error(format!("启动临时 Mihomo 失败: {}", e));
                self.bus.notify_state();
                return;
            }

            let temp_api = MihomoApiClient::new(temp_api_port, secret);
            let mut ready = false;
            for _ in 0..25 {
                if temp_api.health_check().await {
                    ready = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            if !ready {
                mgr.stop().await;
                self.testing_latency.store(false, Ordering::SeqCst);
                self.bus.error("临时 Mihomo 测速实例启动超时");
                self.bus.notify_state();
                return;
            }
            temp_mihomo = Some(mgr);
            temp_api
        };

        self.bus.info(format!(
            "正在真连接测速 {} 个节点 (gstatic.com/generate_204)...",
            nodes.len()
        ));

        let sem = Arc::new(tokio::sync::Semaphore::new(4));
        let mut handles = Vec::with_capacity(nodes.len());
        for node in nodes {
            let api = api.clone();
            let sched = self.scheduler.clone();
            let sem = sem.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await;
                let measured = match api.test_delay(&node.name, 5000).await {
                    Ok(ms) if ms > 0 => Some(ms),
                    _ => Some(0), // 0 = timeout/unreachable
                };
                sched.set_latency(&node.name, measured);
            }));
        }

        for h in handles {
            let _ = h.await;
        }

        if let Some(mgr) = temp_mihomo {
            mgr.stop().await;
        }

        self.testing_latency.store(false, Ordering::SeqCst);
        self.bus.ok("全部节点真连接测速完成");
        self.bus.notify_state();
    }

    #[allow(dead_code)]
    pub async fn simulate_429(&self) {
        let cur = self
            .scheduler
            .current_node_name()
            .unwrap_or_else(|| "US-Demo-01".to_string());
        if let Some(next) = self
            .scheduler
            .report_429(&cur, "aiplatform.googleapis.com (模拟)")
            .await
        {
            self.scheduler.record_success(&next, true);
            self.bus.ok(format!(
                "模拟透明重试 #1 成功 -> HTTP 200 OK (新节点: {})",
                next
            ));
        }
    }
}
