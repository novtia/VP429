use crate::config::AppConfig;
use anyhow::{anyhow, Context, Result};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

pub const POOL_GROUP_NAME: &str = "VERTEX-POOL";
pub const MITM_PROXY_NAME: &str = "RUST-429-MITM";

#[derive(Debug, Clone)]
pub struct ProxyNodeMeta {
    pub name: String,
    pub proxy_type: String,
    pub server: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct ParsedSubscription {
    pub raw_proxies: Vec<Value>,
    pub nodes: Vec<ProxyNodeMeta>,
    pub hosts: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionMeta {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: i64,
    pub suggested_name: Option<String>,
}

fn parse_subscription_userinfo(header: &str) -> (u64, u64, u64, i64) {
    let mut upload = 0u64;
    let mut download = 0u64;
    let mut total = 0u64;
    let mut expire = 0i64;
    for part in header.split(';') {
        if let Some((k, v)) = part.trim().split_once('=') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            match key.as_str() {
                "upload" => upload = val.parse().unwrap_or(0),
                "download" => download = val.parse().unwrap_or(0),
                "total" => total = val.parse().unwrap_or(0),
                "expire" => expire = val.parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    (upload, download, total, expire)
}

fn parse_content_disposition_name(header: &str) -> Option<String> {
    for part in header.split(';') {
        let p = part.trim();
        if let Some(rest) = p.strip_prefix("filename*=") {
            let encoded = rest
                .split("''")
                .nth(1)
                .unwrap_or(rest)
                .trim_matches('"');
            let decoded = urlencoding::decode(encoded)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| encoded.to_string());
            let clean = decoded
                .trim_end_matches(".yaml")
                .trim_end_matches(".yml")
                .trim_end_matches(".conf")
                .trim()
                .to_string();
            if !clean.is_empty() {
                return Some(clean);
            }
        } else if let Some(rest) = p.strip_prefix("filename=") {
            let raw = rest.trim_matches('"').trim();
            let decoded = urlencoding::decode(raw)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| raw.to_string());
            let clean = decoded
                .trim_end_matches(".yaml")
                .trim_end_matches(".yml")
                .trim_end_matches(".conf")
                .trim()
                .to_string();
            if !clean.is_empty() {
                return Some(clean);
            }
        }
    }
    None
}

pub async fn fetch_subscription_to_path(
    url: &str,
    cache_path: &std::path::Path,
    fallback_proxy_port: Option<u16>,
) -> Result<(ParsedSubscription, SubscriptionMeta)> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("订阅 URL 不能为空"));
    }

    let try_fetch = |proxy_port: Option<u16>| async move {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("FlClash/v0.8.80 clash-verge Platform/windows");
        if let Some(p) = proxy_port {
            if let Ok(proxy) = reqwest::Proxy::all(format!("http://127.0.0.1:{}", p)) {
                builder = builder.proxy(proxy);
            }
        }
        let client = builder.build()?;
        let resp = client
            .get(trimmed)
            .send()
            .await
            .with_context(|| format!("请求订阅链接失败: {}", trimmed))?;
        if !resp.status().is_success() {
            return Err(anyhow!("订阅服务器返回 HTTP {}", resp.status()));
        }

        let mut meta = SubscriptionMeta::default();
        if let Some(ui) = resp
            .headers()
            .get("subscription-userinfo")
            .and_then(|v| v.to_str().ok())
        {
            let (u, d, t, e) = parse_subscription_userinfo(ui);
            meta.upload = u;
            meta.download = d;
            meta.total = t;
            meta.expire = e;
        }
        if let Some(cd) = resp
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
        {
            meta.suggested_name = parse_content_disposition_name(cd).map(|s| s.replace('+', " "));
        }

        let body = resp.text().await?;
        let parsed = parse_clash_yaml(&body)?;
        Ok((parsed, meta, body))
    };

    let (parsed, meta, body) = match try_fetch(None).await {
        Ok(res) => res,
        Err(direct_err) => {
            if let Some(port) = fallback_proxy_port {
                try_fetch(Some(port)).await.map_err(|proxy_err| {
                    anyhow!("直连失败 ({})；代理回退也失败 ({})", direct_err, proxy_err)
                })?
            } else {
                return Err(direct_err);
            }
        }
    };

    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(cache_path, &body);
    let _ = fs::write(AppConfig::subscription_cache_path(), &body);
    Ok((parsed, meta))
}

pub fn load_cached_subscription_from(path: &std::path::Path) -> Option<ParsedSubscription> {
    let body = fs::read_to_string(path).ok()?;
    parse_clash_yaml(&body).ok()
}

pub fn load_cached_subscription() -> Option<ParsedSubscription> {
    load_cached_subscription_from(&AppConfig::subscription_cache_path())
}

pub fn parse_clash_yaml(yaml_text: &str) -> Result<ParsedSubscription> {
    let doc: Value = serde_yaml::from_str(yaml_text)
        .context("解析 Clash/Mihomo YAML 失败，请确认订阅链接返回的是标准 Clash YAML 格式")?;

    let proxies_seq = doc
        .get("proxies")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| anyhow!("YAML 中未找到有效的 proxies 列表"))?;

    // Parse optional top-level `hosts:` mapping (used by providers like BitzNet)
    let mut hosts = HashMap::new();
    if let Some(hosts_map) = doc.get("hosts").and_then(|v| v.as_mapping()) {
        for (k, v) in hosts_map {
            if let (Some(ks), Some(vs)) = (k.as_str(), v.as_str()) {
                hosts.insert(ks.trim().to_string(), vs.trim().to_string());
            }
        }
    }

    let mut raw_proxies = Vec::new();
    let mut nodes = Vec::new();

    for item in proxies_seq {
        let Some(map) = item.as_mapping() else {
            continue;
        };
        let Some(name) = map.get(Value::String("name".into())).and_then(|v| v.as_str()) else {
            continue;
        };
        let proxy_type = map
            .get(Value::String("type".into()))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let raw_server = map
            .get(Value::String("server".into()))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        // Resolve server through subscription `hosts:` if mapped
        let server = hosts.get(&raw_server).cloned().unwrap_or(raw_server);
        // Support both integer `port: 443` and quoted string `port: "32520"`
        let port = map
            .get(Value::String("port".into()))
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
            })
            .unwrap_or(0) as u16;

        // Filter out non-proxy info lines commonly injected by providers
        let lower = name.to_ascii_lowercase();
        if name.trim().is_empty()
            || name == MITM_PROXY_NAME
            || lower.contains("剩余流量")
            || lower.contains("套餐到期")
            || lower.contains("官网")
            || lower.contains("更新订阅")
            || lower.contains("禁止使用")
            || lower.contains("订阅托管")
            || server.is_empty()
            || port == 0
        {
            continue;
        }

        raw_proxies.push(item.clone());
        nodes.push(ProxyNodeMeta {
            name: name.to_string(),
            proxy_type,
            server,
            port,
        });
    }

    if nodes.is_empty() {
        return Err(anyhow!("订阅中未解析到可用代理节点"));
    }

    Ok(ParsedSubscription {
        raw_proxies,
        nodes,
        hosts,
    })
}

/// Generates a loop-free mihomo runtime YAML configuration:
/// - `mixed-port` (`17890`) handles normal rules
/// - Dedicated HTTP `listeners` (`17892`) binds directly to `proxy: VERTEX-POOL`, bypassing `rules`
///   so decrypted traffic from our Rust MITM server never loops back into `RUST-429-MITM`.
/// - Watch domains route to `RUST-429-MITM` (`127.0.0.1:17891`) when coming from TUN or `mixed-port`.
pub fn generate_mihomo_config(
    sub: &ParsedSubscription,
    cfg: &AppConfig,
    api_secret: &str,
) -> Result<String> {
    let mut root = Mapping::new();

    root.insert(
        Value::String("mixed-port".into()),
        Value::Number(cfg.mihomo_mixed_port.into()),
    );
    root.insert(Value::String("allow-lan".into()), Value::Bool(false));
    root.insert(Value::String("unified-delay".into()), Value::Bool(true));
    root.insert(Value::String("mode".into()), Value::String("rule".into()));
    root.insert(
        Value::String("log-level".into()),
        Value::String("info".into()),
    );
    root.insert(
        Value::String("external-controller".into()),
        Value::String(format!("127.0.0.1:{}", cfg.mihomo_api_port)),
    );
    root.insert(
        Value::String("secret".into()),
        Value::String(api_secret.to_string()),
    );

    // DNS block (required for TUN fake-ip and subscription `hosts:`)
    let mut dns = Mapping::new();
    dns.insert(Value::String("enable".into()), Value::Bool(true));
    dns.insert(Value::String("ipv6".into()), Value::Bool(false));
    dns.insert(Value::String("use-hosts".into()), Value::Bool(true));
    dns.insert(
        Value::String("enhanced-mode".into()),
        Value::String("fake-ip".into()),
    );
    dns.insert(
        Value::String("default-nameserver".into()),
        Value::Sequence(vec![
            Value::String("223.5.5.5".into()),
            Value::String("119.29.29.29".into()),
        ]),
    );
    dns.insert(
        Value::String("proxy-server-nameserver".into()),
        Value::Sequence(vec![
            Value::String("223.5.5.5".into()),
            Value::String("119.29.29.29".into()),
        ]),
    );
    dns.insert(
        Value::String("nameserver".into()),
        Value::Sequence(vec![
            Value::String("223.5.5.5".into()),
            Value::String("119.29.29.29".into()),
        ]),
    );
    root.insert(Value::String("dns".into()), Value::Mapping(dns));

    if !sub.hosts.is_empty() {
        let mut hosts_map = Mapping::new();
        for (k, v) in &sub.hosts {
            hosts_map.insert(Value::String(k.clone()), Value::String(v.clone()));
        }
        root.insert(Value::String("hosts".into()), Value::Mapping(hosts_map));
    }

    // TUN block
    let mut tun = Mapping::new();
    tun.insert(Value::String("enable".into()), Value::Bool(cfg.enable_tun));
    tun.insert(
        Value::String("stack".into()),
        Value::String("mixed".into()),
    );
    tun.insert(
        Value::String("auto-route".into()),
        Value::Bool(cfg.enable_tun),
    );
    tun.insert(
        Value::String("auto-detect-interface".into()),
        Value::Bool(cfg.enable_tun),
    );
    tun.insert(
        Value::String("dns-hijack".into()),
        Value::Sequence(vec![Value::String("any:53".into())]),
    );
    root.insert(Value::String("tun".into()), Value::Mapping(tun));

    // Loop-free bypass listener directly targeting VERTEX-POOL
    let mut bypass_listener = Mapping::new();
    bypass_listener.insert(
        Value::String("name".into()),
        Value::String("vertex-bypass".into()),
    );
    bypass_listener.insert(Value::String("type".into()), Value::String("http".into()));
    bypass_listener.insert(
        Value::String("port".into()),
        Value::Number(cfg.mihomo_bypass_port.into()),
    );
    bypass_listener.insert(
        Value::String("listen".into()),
        Value::String("127.0.0.1".into()),
    );
    bypass_listener.insert(
        Value::String("proxy".into()),
        Value::String(POOL_GROUP_NAME.into()),
    );
    root.insert(
        Value::String("listeners".into()),
        Value::Sequence(vec![Value::Mapping(bypass_listener)]),
    );

    // Proxies = subscription proxies + internal RUST-429-MITM loopback proxy
    let mut proxies = sub.raw_proxies.clone();
    let mut mitm_proxy = Mapping::new();
    mitm_proxy.insert(
        Value::String("name".into()),
        Value::String(MITM_PROXY_NAME.into()),
    );
    mitm_proxy.insert(Value::String("type".into()), Value::String("http".into()));
    mitm_proxy.insert(
        Value::String("server".into()),
        Value::String("127.0.0.1".into()),
    );
    mitm_proxy.insert(
        Value::String("port".into()),
        Value::Number(cfg.tun_mitm_port.into()),
    );
    proxies.push(Value::Mapping(mitm_proxy));
    root.insert(Value::String("proxies".into()), Value::Sequence(proxies));

    // Proxy group: VERTEX-POOL
    let mut pool_group = Mapping::new();
    pool_group.insert(
        Value::String("name".into()),
        Value::String(POOL_GROUP_NAME.into()),
    );
    pool_group.insert(
        Value::String("type".into()),
        Value::String("select".into()),
    );
    pool_group.insert(
        Value::String("proxies".into()),
        Value::Sequence(
            sub.nodes
                .iter()
                .map(|n| Value::String(n.name.clone()))
                .collect(),
        ),
    );
    root.insert(
        Value::String("proxy-groups".into()),
        Value::Sequence(vec![Value::Mapping(pool_group)]),
    );

    // Rules: watch domains -> RUST-429-MITM, local -> DIRECT, rest -> VERTEX-POOL
    let mut rules = vec![
        Value::String("IP-CIDR,127.0.0.0/8,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,192.168.0.0/16,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,10.0.0.0/8,DIRECT,no-resolve".into()),
    ];

    for dom in &cfg.watch_domains {
        let clean = dom.trim().trim_start_matches("*.");
        if !clean.is_empty() {
            rules.push(Value::String(format!(
                "DOMAIN-SUFFIX,{},{}",
                clean, MITM_PROXY_NAME
            )));
        }
    }
    rules.push(Value::String(format!("MATCH,{}", POOL_GROUP_NAME)));
    root.insert(Value::String("rules".into()), Value::Sequence(rules));

    serde_yaml::to_string(&Value::Mapping(root)).context("序列化 mihomo 运行配置失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CLASH_YAML: &str = r#"
hosts:
  alias.example.net: real.example.net
proxies:
  - name: "剩余流量: 100GB"
    type: ss
    server: 1.1.1.1
    port: 443
  - name: "US-Oregon-01"
    type: vless
    server: alias.example.net
    port: "32520"
  - name: "JP-Tokyo-02"
    type: trojan
    server: 203.0.113.20
    port: 443
"#;

    #[test]
    fn test_parse_and_generate_loop_free_config() {
        let parsed = parse_clash_yaml(SAMPLE_CLASH_YAML).unwrap();
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.nodes[0].name, "US-Oregon-01");
        assert_eq!(parsed.nodes[0].server, "real.example.net");
        assert_eq!(parsed.nodes[0].port, 32520);
        assert_eq!(parsed.nodes[1].name, "JP-Tokyo-02");

        let mut cfg = AppConfig::default();
        cfg.enable_tun = true;
        let runtime_yaml = generate_mihomo_config(&parsed, &cfg, "secret123").unwrap();

        assert!(runtime_yaml.contains("vertex-bypass"));
        assert!(runtime_yaml.contains("port: 17892"));
        assert!(runtime_yaml.contains("proxy: VERTEX-POOL"));
        assert!(runtime_yaml.contains("RUST-429-MITM"));
        assert!(runtime_yaml.contains("alias.example.net: real.example.net"));
        assert!(runtime_yaml.contains("DOMAIN-SUFFIX,aiplatform.googleapis.com,RUST-429-MITM"));
        assert!(runtime_yaml.contains("MATCH,VERTEX-POOL"));
    }
}
