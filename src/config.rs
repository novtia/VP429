use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RotationMode {
    On429Only,
    RoundRobinPerRequest,
    RoundRobinInterval { seconds: u64 },
}

impl RotationMode {
    #[allow(dead_code)]
    pub fn label(&self) -> String {
        match self {
            RotationMode::On429Only => "429-FAILOVER".to_string(),
            RotationMode::RoundRobinPerRequest => "PER-REQUEST".to_string(),
            RotationMode::RoundRobinInterval { seconds } => format!("INTERVAL-{}S", seconds),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionProfile {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub node_count: usize,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub updated_unix: i64,
    #[serde(default)]
    pub used_bytes: u64,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub expire_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub subscription_url: String,
    pub profiles: Vec<SubscriptionProfile>,
    pub active_profile_id: Option<String>,
    pub mihomo_path: Option<String>,
    pub local_proxy_port: u16,
    pub allow_lan: bool,
    pub mihomo_mixed_port: u16,
    pub tun_mitm_port: u16,
    pub mihomo_bypass_port: u16,
    pub mihomo_api_port: u16,
    pub enable_system_proxy: bool,
    pub enable_tun: bool,
    pub rotation_mode: RotationMode,
    pub cooldown_seconds: u64,
    pub auto_retry_on_429: bool,
    pub max_retries: usize,
    pub max_retry_body_bytes: usize,
    pub watch_domains: Vec<String>,
    pub intercept_status_codes: Vec<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            subscription_url: String::new(),
            profiles: Vec::new(),
            active_profile_id: None,
            mihomo_path: None,
            local_proxy_port: 7890,
            allow_lan: false,
            mihomo_mixed_port: 17890,
            tun_mitm_port: 17891,
            mihomo_bypass_port: 17892,
            mihomo_api_port: 19090,
            enable_system_proxy: true,
            enable_tun: false,
            rotation_mode: RotationMode::On429Only,
            cooldown_seconds: 60,
            auto_retry_on_429: true,
            max_retries: 2,
            max_retry_body_bytes: 16 * 1024 * 1024,
            watch_domains: vec![
                "aiplatform.googleapis.com".to_string(),
                "generativelanguage.googleapis.com".to_string(),
                "*.googleapis.com".to_string(),
            ],
            intercept_status_codes: vec![429],
        }
    }
}

impl AppConfig {
    pub fn data_dir() -> PathBuf {
        if let Some(proj) = ProjectDirs::from("io", "vp429", "VP429") {
            let dir = proj.data_local_dir().to_path_buf();
            let _ = fs::create_dir_all(&dir);
            // Seamlessly migrate legacy vertex-vpn data directory on first launch
            if !dir.join("config.json").exists() {
                if let Some(old_proj) = ProjectDirs::from("io", "vertexvpn", "vertex-vpn") {
                    let old_dir = old_proj.data_local_dir();
                    if old_dir.join("config.json").exists() {
                        fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
                            let _ = fs::create_dir_all(dst);
                            if let Ok(entries) = fs::read_dir(src) {
                                for entry in entries.flatten() {
                                    let p = entry.path();
                                    let target = dst.join(entry.file_name());
                                    if p.is_dir() {
                                        copy_dir_recursive(&p, &target);
                                    } else if !target.exists() {
                                        let _ = fs::copy(&p, &target);
                                    }
                                }
                            }
                        }
                        copy_dir_recursive(old_dir, &dir);
                    }
                }
            }
            return dir;
        }
        let fallback = PathBuf::from("./data");
        let _ = fs::create_dir_all(&fallback);
        fallback
    }

    pub fn config_path() -> PathBuf {
        Self::data_dir().join("config.json")
    }

    pub fn bin_dir() -> PathBuf {
        let dir = Self::data_dir().join("bin");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn ca_dir() -> PathBuf {
        let dir = Self::data_dir().join("ca");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn runtime_yaml_path() -> PathBuf {
        Self::data_dir().join("mihomo-runtime.yaml")
    }

    pub fn subscription_cache_path() -> PathBuf {
        Self::data_dir().join("subscription-cache.yaml")
    }

    pub fn profiles_dir() -> PathBuf {
        let dir = Self::data_dir().join("profiles");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn profile_cache_path(id: &str) -> PathBuf {
        Self::profiles_dir().join(format!("{}.yaml", id))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&content) {
                // Auto-migrate legacy single subscription_url into multi-profile list
                if cfg.profiles.is_empty() && !cfg.subscription_url.trim().is_empty() {
                    let url = cfg.subscription_url.trim().to_string();
                    let id = "default-profile".to_string();
                    let name = derive_profile_name(&url);
                    if Self::subscription_cache_path().exists() && !Self::profile_cache_path(&id).exists() {
                        let _ = fs::copy(
                            Self::subscription_cache_path(),
                            Self::profile_cache_path(&id),
                        );
                    }
                    cfg.profiles.push(SubscriptionProfile {
                        id: id.clone(),
                        name,
                        url,
                        node_count: 0,
                        updated_at: String::new(),
                        updated_unix: 0,
                        used_bytes: 0,
                        total_bytes: 0,
                        expire_unix: 0,
                    });
                    cfg.active_profile_id = Some(id);
                }
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json).with_context(|| format!("Failed to save config to {:?}", path))?;
        Ok(())
    }

    pub fn matches_watch_domain(&self, host: &str) -> bool {
        let host_lower = host.trim().to_ascii_lowercase();
        for pattern in &self.watch_domains {
            let pat = pattern.trim().to_ascii_lowercase();
            if pat.is_empty() {
                continue;
            }
            if let Some(suffix) = pat.strip_prefix("*.") {
                if host_lower == suffix || host_lower.ends_with(&format!(".{}", suffix)) {
                    return true;
                }
            } else if host_lower == pat || host_lower.ends_with(&format!(".{}", pat)) {
                return true;
            }
        }
        false
    }
}

pub fn derive_profile_name(url: &str) -> String {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim();
    if host.is_empty() {
        "未命名订阅".to_string()
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_domain_matching() {
        let cfg = AppConfig::default();
        assert!(cfg.matches_watch_domain("us-central1-aiplatform.googleapis.com"));
        assert!(cfg.matches_watch_domain("aiplatform.googleapis.com"));
        assert!(cfg.matches_watch_domain("generativelanguage.googleapis.com"));
        assert!(cfg.matches_watch_domain("storage.googleapis.com"));
        assert!(!cfg.matches_watch_domain("github.com"));
        assert!(!cfg.matches_watch_domain("evilgoogleapis.com"));
    }
}
