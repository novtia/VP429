use crate::config::AppConfig;
use crate::state::EventBus;
use crate::subscription::POOL_GROUP_NAME;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Abstraction over switching the active outbound proxy node (implemented by MihomoApiClient and mock tests).
pub trait ProxySwitcher: Send + Sync {
    fn select_node<'a>(
        &'a self,
        node_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    fn select_group_node<'a>(
        &'a self,
        _group: &'a str,
        node_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        self.select_node(node_name)
    }
}

#[derive(Clone)]
pub struct MihomoApiClient {
    base_url: String,
    secret: String,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct DelayResp {
    delay: u64,
}

impl MihomoApiClient {
    pub fn new(api_port: u16, secret: String) -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .pool_max_idle_per_host(128)
            .tcp_nodelay(true)
            .timeout(Duration::from_secs(6))
            .build()
            .expect("reqwest client");
        Self {
            base_url: format!("http://127.0.0.1:{}", api_port),
            secret,
            client,
        }
    }

    pub fn api_port(&self) -> Option<u16> {
        self.base_url
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
    }

    pub async fn health_check(&self) -> bool {
        let url = format!("{}/version", self.base_url);
        let mut req = self.client.get(&url).timeout(Duration::from_millis(800));
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        matches!(req.send().await, Ok(r) if r.status().is_success())
    }

    pub async fn select_proxy_group(&self, group: &str, node_name: &str) -> Result<()> {
        let encoded_group = urlencoding::encode(group);
        let url = format!("{}/proxies/{}", self.base_url, encoded_group);
        let body = serde_json::json!({ "name": node_name });
        let mut req = self.client.put(&url).json(&body);
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        let resp = req
            .send()
            .await
            .with_context(|| format!("调用 Mihomo API 切换节点 {} 失败", node_name))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Mihomo 切换节点失败 ({}): {}", status, text));
        }
        Ok(())
    }

    pub async fn test_delay(&self, node_name: &str, timeout_ms: u64) -> Result<u64> {
        let encoded = urlencoding::encode(node_name);
        let url = format!(
            "{}/proxies/{}/delay?timeout={}&url=http://www.gstatic.com/generate_204",
            self.base_url, encoded, timeout_ms
        );
        let mut req = self
            .client
            .get(&url)
            .timeout(Duration::from_millis(timeout_ms + 800));
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("测速超时或不可用"));
        }
        let parsed: DelayResp = resp.json().await?;
        Ok(parsed.delay)
    }

    /// Native Mihomo batch group delay probe (`GET /group/{group}/delay`).
    #[allow(dead_code)]
    pub async fn test_group_delay(
        &self,
        group: &str,
        timeout_ms: u64,
    ) -> Result<HashMap<String, u64>> {
        let encoded = urlencoding::encode(group);
        let url = format!(
            "{}/group/{}/delay?timeout={}&url=http://www.gstatic.com/generate_204",
            self.base_url, encoded, timeout_ms
        );
        let mut req = self
            .client
            .get(&url)
            .timeout(Duration::from_millis(timeout_ms + 1200));
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("代理组批量测速超时或不可用"));
        }
        let map: HashMap<String, u64> = resp.json().await?;
        Ok(map)
    }

    pub async fn set_mode(&self, mode: &str) -> Result<()> {
        let url = format!("{}/configs", self.base_url);
        let body = serde_json::json!({ "mode": mode });
        let mut req = self.client.patch(&url).json(&body);
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("切换出站模式失败: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn reload_config(&self, config_path: &Path) -> Result<()> {
        let url = format!("{}/configs?force=true", self.base_url);
        let body = serde_json::json!({ "path": config_path.to_string_lossy() });
        let mut req = self
            .client
            .put(&url)
            .json(&body)
            .timeout(Duration::from_secs(12));
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        let resp = req
            .send()
            .await
            .context("调用 Mihomo API 热重载配置失败")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Mihomo 热重载配置失败 ({}): {}", status, text));
        }
        Ok(())
    }

    pub async fn close_all_connections(&self) {
        let url = format!("{}/connections", self.base_url);
        let mut req = self.client.delete(&url);
        if !self.secret.is_empty() {
            req = req.bearer_auth(&self.secret);
        }
        let _ = req.send().await;
    }

    pub fn traffic_url(&self) -> String {
        format!("{}/traffic", self.base_url)
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }
}

impl ProxySwitcher for MihomoApiClient {
    fn select_node<'a>(
        &'a self,
        node_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { self.select_proxy_group(POOL_GROUP_NAME, node_name).await })
    }

    fn select_group_node<'a>(
        &'a self,
        group: &'a str,
        node_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { self.select_proxy_group(group, node_name).await })
    }
}

pub struct MihomoManager {
    child: Mutex<Option<Child>>,
    bus: Arc<EventBus>,
}

impl MihomoManager {
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self {
            child: Mutex::new(None),
            bus,
        }
    }

    pub fn resolve_binary_path(cfg: &AppConfig) -> Option<PathBuf> {
        if let Some(custom) = &cfg.mihomo_path {
            let p = PathBuf::from(custom);
            if p.exists() {
                return Some(p);
            }
        }
        let default_bin = AppConfig::bin_dir().join("mihomo.exe");
        if default_bin.exists() {
            return Some(default_bin);
        }
        let local_bin = PathBuf::from("./bin/mihomo.exe");
        if local_bin.exists() {
            return Some(local_bin);
        }
        None
    }

    pub async fn is_alive(&self) -> bool {
        let mut guard = self.child.lock().await;
        if let Some(ref mut c) = *guard {
            if let Ok(None) = c.try_wait() {
                return true;
            }
        }
        false
    }

    pub async fn start(&self, binary: &Path, runtime_yaml: &Path) -> Result<()> {
        self.stop().await;

        let mut cmd = Command::new(binary);
        cmd.arg("-d")
            .arg(AppConfig::data_dir())
            .arg("-f")
            .arg(runtime_yaml)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("启动 mihomo 进程失败: {:?}", binary))?;

        if let Some(stdout) = child.stdout.take() {
            let bus = self.bus.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.contains("level=fatal") || line.contains("FTL") {
                        bus.error(format!("[mihomo] {}", line));
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let bus = self.bus.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if !line.trim().is_empty() {
                        bus.error(format!("[mihomo] {}", line));
                    }
                }
            });
        }

        let mut guard = self.child.lock().await;
        *guard = Some(child);
        Ok(())
    }

    pub async fn stop(&self) {
        let mut guard = self.child.lock().await;
        if let Some(mut child) = guard.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Ensures `geoip.metadb` (or `country.mmdb`) and `geosite.dat` exist in `AppConfig::data_dir()`
/// by downloading from fast domestic-friendly CDN mirrors in the background.
pub async fn ensure_geodata_files(proxy_port: Option<u16>) -> bool {
    let data_dir = AppConfig::data_dir();
    let geoip_target = data_dir.join("geoip.metadb");
    let geosite_target = data_dir.join("geosite.dat");
    let has_geoip = geoip_target.exists()
        || data_dir.join("country.mmdb").exists()
        || data_dir.join("Country.mmdb").exists();
    let has_geosite = geosite_target.exists() || data_dir.join("GeoSite.dat").exists();

    if has_geoip && has_geosite {
        return false;
    }

    let build_client = |use_proxy: bool| -> Option<reqwest::Client> {
        let mut b = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .user_agent("VP429/0.1");
        if use_proxy {
            if let Some(p) = proxy_port {
                if let Ok(px) = reqwest::Proxy::all(format!("http://127.0.0.1:{}", p)) {
                    b = b.proxy(px);
                }
            }
        } else {
            b = b.no_proxy();
        }
        b.build().ok()
    };

    let fetch_first = |urls: &[&str], dst: &Path| {
        let urls: Vec<String> = urls.iter().map(|s| (*s).to_string()).collect();
        let dst = dst.to_path_buf();
        async move {
            for use_proxy in [false, true] {
                if use_proxy && proxy_port.is_none() {
                    continue;
                }
                let Some(client) = build_client(use_proxy) else {
                    continue;
                };
                for url in &urls {
                    if let Ok(resp) = client.get(url).send().await {
                        if resp.status().is_success() {
                            if let Ok(bytes) = resp.bytes().await {
                                if bytes.len() > 64 * 1024 {
                                    let tmp = dst.with_extension("tmp");
                                    if fs::write(&tmp, &bytes).is_ok() {
                                        let _ = fs::rename(&tmp, &dst);
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            false
        }
    };

    let mut downloaded_any = false;
    if !has_geoip {
        let ok = fetch_first(
            &[
                "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.metadb",
                "https://fastly.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.metadb",
                "https://gcore.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.metadb",
            ],
            &geoip_target,
        )
        .await;
        if ok {
            downloaded_any = true;
        }
    }
    if !has_geosite {
        let ok = fetch_first(
            &[
                "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat",
                "https://fastly.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat",
                "https://gcore.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat",
            ],
            &geosite_target,
        )
        .await;
        if ok {
            downloaded_any = true;
        }
    }
    downloaded_any
}

pub async fn download_mihomo_core(bus: &EventBus) -> Result<PathBuf> {
    bus.info("正在从 GitHub 获取最新 Mihomo Release 信息...");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent("vertex-vpn/0.1")
        .build()?;

    let release: GithubRelease = client
        .get("https://api.github.com/repos/MetaCubeX/mihomo/releases/latest")
        .send()
        .await
        .context("请求 GitHub Releases API 失败")?
        .error_for_status()?
        .json()
        .await?;

    let asset = release
        .assets
        .iter()
        .find(|a| {
            a.name.starts_with("mihomo-windows-amd64-v")
                && a.name.ends_with(".zip")
                && !a.name.contains("compatible")
                && !a.name.contains("go1")
        })
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|a| a.name.starts_with("mihomo-windows-amd64") && a.name.ends_with(".zip"))
        })
        .ok_or_else(|| anyhow!("未在 Release {} 中找到 Windows amd64 压缩包", release.tag_name))?;

    bus.info(format!("正在下载 {} ({})...", asset.name, release.tag_name));
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let bin_dir = AppConfig::bin_dir();
    let target_exe = bin_dir.join("mihomo.exe");

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut extracted = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.name().ends_with(".exe") {
            let mut out = fs::File::create(&target_exe)?;
            std::io::copy(&mut file, &mut out)?;
            extracted = true;
            break;
        }
    }
    if !extracted {
        return Err(anyhow!("压缩包中未找到 mihomo.exe"));
    }

    bus.ok(format!("Mihomo 内核已安装至 {:?}", target_exe));
    Ok(target_exe)
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

#[cfg(windows)]
pub fn relaunch_as_admin() -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
    let verb: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let res = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (res as isize) <= 32 {
        return Err(anyhow!("请求管理员权限失败或用户取消了 UAC 授权"));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn relaunch_as_admin() -> Result<()> {
    Err(anyhow!("当前平台不支持自动提权"))
}

/// Non-blocking background check for other active VPN TUN virtual adapters (never blocks start or ping).
#[cfg(windows)]
pub async fn detect_other_vpn_tun_async() -> Option<String> {
    tokio::task::spawn_blocking(|| {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let out = std::process::Command::new("ipconfig")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let vpn_tun_keywords = ["flclash", "meta tunnel", "clash", "sing-box"];
        for line in text.lines() {
            let lower = line.to_ascii_lowercase();
            if let Some(kw) = vpn_tun_keywords.iter().find(|&&kw| lower.contains(kw)) {
                return Some((*kw).to_string());
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

#[cfg(not(windows))]
pub async fn detect_other_vpn_tun_async() -> Option<String> {
    None
}
