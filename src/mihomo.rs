use crate::config::AppConfig;
use crate::state::EventBus;
use crate::subscription::POOL_GROUP_NAME;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
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
            .timeout(Duration::from_secs(6))
            .build()
            .expect("reqwest client");
        Self {
            base_url: format!("http://127.0.0.1:{}", api_port),
            secret,
            client,
        }
    }

    pub async fn health_check(&self) -> bool {
        let url = format!("{}/version", self.base_url);
        let mut req = self.client.get(&url);
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
        let mut req = self.client.get(&url);
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
                    if line.contains("error") || line.contains("ERR") || line.contains("fatal") {
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

/// Downloads the official Windows amd64 mihomo binary into `%LOCALAPPDATA%\vertex-vpn\bin\mihomo.exe`.
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

/// Detects if another VPN's TUN virtual adapter (e.g. "FlClash Meta Tunnel") is currently Up,
/// which would cause double-hopped traffic and inflated latency.
#[cfg(windows)]
pub fn detect_other_vpn_tun() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { \"$($_.Name)`t$($_.InterfaceDescription)\" }",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let vpn_tun_keywords = ["flclash", "clash", "meta tunnel", "wintun", "sing-box"];

    for line in text.lines() {
        let clean = line.trim_start_matches('\u{feff}').trim();
        let mut parts = clean.splitn(2, '\t');
        let name = parts.next().unwrap_or("").trim();
        let desc = parts.next().unwrap_or("").trim();
        let combined = format!("{} {}", name, desc).to_ascii_lowercase();
        if vpn_tun_keywords.iter().any(|kw| combined.contains(kw)) {
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(not(windows))]
pub fn detect_other_vpn_tun() -> Option<String> {
    None
}
