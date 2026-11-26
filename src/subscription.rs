use crate::config::{AppConfig, ChainExitSource};
use anyhow::{anyhow, Context, Result};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

pub const POOL_GROUP_NAME: &str = "VERTEX-POOL";
pub const OUTBOUND_GROUP_NAME: &str = "VERTEX-OUTBOUND";
pub const CHAIN_EXIT_PROXY_NAME: &str = "VERTEX-CHAIN-EXIT";
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
    pub fake_ip_filter: Vec<String>,
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

    // Preserve provider-specific `dns.fake-ip-filter` entries (e.g. BitzNet node domains)
    let mut fake_ip_filter = Vec::new();
    if let Some(seq) = doc
        .get("dns")
        .and_then(|d| d.get("fake-ip-filter"))
        .and_then(|v| v.as_sequence())
    {
        for item in seq {
            if let Some(s) = item.as_str() {
                let clean = s.trim();
                if !clean.is_empty() {
                    fake_ip_filter.push(clean.to_string());
                }
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
        fake_ip_filter,
    })
}

/// Parses a user-supplied custom landing/exit proxy specification.
/// Supports:
/// 1. Proxy URLs: `socks5://[user:pass@]host:port`, `socks5h://...`, `http://...`, `https://...`
/// 2. A single Clash/Mihomo YAML proxy mapping (or 1-element YAML sequence).
pub fn parse_custom_exit_proxy(raw: &str) -> Result<Mapping> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("自定义落地节点不能为空"));
    }

    let lower = trimmed.to_ascii_lowercase();
    let url_scheme = if lower.starts_with("socks5://") {
        Some(("socks5", false, 9usize))
    } else if lower.starts_with("socks5h://") {
        Some(("socks5", false, 10usize))
    } else if lower.starts_with("http://") {
        Some(("http", false, 7usize))
    } else if lower.starts_with("https://") {
        Some(("http", true, 8usize))
    } else {
        None
    };

    if let Some((proxy_type, use_tls, prefix_len)) = url_scheme {
        let rest = trimmed[prefix_len..].trim_end_matches('/');
        let (auth_part, host_port) = match rest.rsplit_once('@') {
            Some((a, hp)) => (Some(a), hp),
            None => (None, rest),
        };
        let (host, port_str) = if let Some(after_bracket) = host_port.strip_prefix('[') {
            let (h, p) = after_bracket
                .split_once("]:")
                .ok_or_else(|| anyhow!("无效的 IPv6 代理地址，格式应为 [::1]:port"))?;
            (h, p)
        } else {
            host_port
                .rsplit_once(':')
                .ok_or_else(|| anyhow!("代理地址缺少端口，格式应为 host:port"))?
        };
        let port: u16 = port_str
            .trim()
            .parse()
            .map_err(|_| anyhow!("无效的端口号: {}", port_str))?;
        if host.trim().is_empty() || port == 0 {
            return Err(anyhow!("代理主机或端口无效"));
        }

        let mut map = Mapping::new();
        map.insert(
            Value::String("name".into()),
            Value::String(CHAIN_EXIT_PROXY_NAME.into()),
        );
        map.insert(
            Value::String("type".into()),
            Value::String(proxy_type.into()),
        );
        map.insert(
            Value::String("server".into()),
            Value::String(host.trim().into()),
        );
        map.insert(Value::String("port".into()), Value::Number(port.into()));
        if use_tls {
            map.insert(Value::String("tls".into()), Value::Bool(true));
        }
        if let Some(auth) = auth_part {
            let (user, pass) = match auth.split_once(':') {
                Some((u, p)) => (u, Some(p)),
                None => (auth, None),
            };
            let dec_user = urlencoding::decode(user)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| user.to_string());
            if !dec_user.is_empty() {
                map.insert(
                    Value::String("username".into()),
                    Value::String(dec_user),
                );
            }
            if let Some(p) = pass {
                let dec_pass = urlencoding::decode(p)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| p.to_string());
                map.insert(
                    Value::String("password".into()),
                    Value::String(dec_pass),
                );
            }
        }
        return Ok(map);
    }

    let val: Value = serde_yaml::from_str(trimmed).context(
        "解析自定义落地节点失败，请输入 socks5://... / http://... 或单条 Clash 节点 YAML",
    )?;
    let map = if let Some(seq) = val.as_sequence() {
        if seq.len() != 1 {
            return Err(anyhow!("YAML 列表仅支持填写单个落地节点"));
        }
        seq[0]
            .as_mapping()
            .cloned()
            .ok_or_else(|| anyhow!("YAML 列表项不是有效的节点对象"))?
    } else {
        val.as_mapping()
            .cloned()
            .ok_or_else(|| anyhow!("自定义落地节点必须是代理 URL 或 Clash YAML 节点映射"))?
    };

    let has_type = map
        .get(Value::String("type".into()))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_server = map
        .get(Value::String("server".into()))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_port = map
        .get(Value::String("port".into()))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        })
        .map(|p| p > 0)
        .unwrap_or(false);
    if !has_type || !has_server || !has_port {
        return Err(anyhow!("Clash 节点 YAML 必须包含有效的 type、server 与 port 字段"));
    }
    Ok(map)
}

pub fn resolve_chain_exit_mapping(
    sub: &ParsedSubscription,
    src: &ChainExitSource,
) -> Result<Mapping> {
    match src {
        ChainExitSource::SubscriptionNode { name } => sub
            .raw_proxies
            .iter()
            .filter_map(|v| v.as_mapping())
            .find(|m| {
                m.get(Value::String("name".into()))
                    .and_then(|v| v.as_str())
                    == Some(name.as_str())
            })
            .cloned()
            .ok_or_else(|| anyhow!("当前订阅中未找到落地节点「{}」", name)),
        ChainExitSource::CustomSpec { raw } => parse_custom_exit_proxy(raw),
    }
}

/// Generates a loop-free mihomo runtime YAML configuration:
/// - `mixed-port` (`17890`) handles normal rules
/// - Dedicated HTTP `listeners` (`17892`) binds directly to `proxy: VERTEX-OUTBOUND`, bypassing `rules`
///   so decrypted traffic from our Rust MITM server never loops back into `RUST-429-MITM`.
/// - `VERTEX-OUTBOUND` hot-switches between `VERTEX-POOL` (direct rotation) and `VERTEX-CHAIN-EXIT`
///   (fixed exit node dialing through `VERTEX-POOL`).
/// - Watch domains route to `RUST-429-MITM` (`127.0.0.1:17891`) when coming from TUN or `mixed-port`.
pub fn generate_mihomo_config(
    sub: &ParsedSubscription,
    cfg: &AppConfig,
    api_secret: &str,
    enable_mitm: bool,
    enable_tun: bool,
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
    let mut filters: Vec<String> = vec![
        "+.lan".into(),
        "+.local".into(),
        "+.msftconnecttest.com".into(),
        "+.msftncsi.com".into(),
        "localhost.ptlogin2.qq.com".into(),
    ];
    for f in &sub.fake_ip_filter {
        if !filters.iter().any(|existing| existing.eq_ignore_ascii_case(f)) {
            filters.push(f.clone());
        }
    }
    dns.insert(
        Value::String("fake-ip-filter".into()),
        Value::Sequence(filters.into_iter().map(Value::String).collect()),
    );
    root.insert(Value::String("dns".into()), Value::Mapping(dns));

    if !sub.hosts.is_empty() {
        let mut hosts_map = Mapping::new();
        for (k, v) in &sub.hosts {
            hosts_map.insert(Value::String(k.clone()), Value::String(v.clone()));
        }
        root.insert(Value::String("hosts".into()), Value::Mapping(hosts_map));
    }

    // Sniffer block: recover TLS SNI hostname on pure-IP connections (e.g. cached DNS / DoH)
    // so watch domains never bypass RUST-429-MITM via raw IP MATCH.
    let mut sniffer = Mapping::new();
    sniffer.insert(Value::String("enable".into()), Value::Bool(true));
    sniffer.insert(Value::String("force-dns-mapping".into()), Value::Bool(true));
    sniffer.insert(Value::String("parse-pure-ip".into()), Value::Bool(true));
    sniffer.insert(
        Value::String("override-destination".into()),
        Value::Bool(true),
    );
    let mut sniff_protos = Mapping::new();
    let mut tls_sniff = Mapping::new();
    tls_sniff.insert(
        Value::String("ports".into()),
        Value::Sequence(vec![Value::Number(443.into()), Value::Number(8443.into())]),
    );
    sniff_protos.insert(Value::String("TLS".into()), Value::Mapping(tls_sniff));
    let mut http_sniff = Mapping::new();
    http_sniff.insert(
        Value::String("ports".into()),
        Value::Sequence(vec![
            Value::Number(80.into()),
            Value::String("8080-8880".into()),
        ]),
    );
    sniff_protos.insert(Value::String("HTTP".into()), Value::Mapping(http_sniff));
    sniffer.insert(Value::String("sniff".into()), Value::Mapping(sniff_protos));
    root.insert(Value::String("sniffer".into()), Value::Mapping(sniffer));

    // TUN block
    let mut tun = Mapping::new();
    tun.insert(Value::String("enable".into()), Value::Bool(enable_tun));
    tun.insert(
        Value::String("stack".into()),
        Value::String("mixed".into()),
    );
    tun.insert(Value::String("auto-route".into()), Value::Bool(enable_tun));
    tun.insert(
        Value::String("auto-detect-interface".into()),
        Value::Bool(enable_tun),
    );
    tun.insert(
        Value::String("dns-hijack".into()),
        Value::Sequence(vec![Value::String("any:53".into())]),
    );
    root.insert(Value::String("tun".into()), Value::Mapping(tun));

    // Loop-free bypass listener directly targeting VERTEX-OUTBOUND
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
        Value::String(OUTBOUND_GROUP_NAME.into()),
    );
    root.insert(
        Value::String("listeners".into()),
        Value::Sequence(vec![Value::Mapping(bypass_listener)]),
    );

    // Proxies = subscription proxies + optional VERTEX-CHAIN-EXIT (dialer-proxy: VERTEX-POOL) + RUST-429-MITM
    let mut proxies = sub.raw_proxies.clone();
    let mut has_chain_exit = false;
    if let Some(ref src) = cfg.chain_exit_source {
        if let Ok(mut exit_map) = resolve_chain_exit_mapping(sub, src) {
            exit_map.insert(
                Value::String("name".into()),
                Value::String(CHAIN_EXIT_PROXY_NAME.into()),
            );
            exit_map.insert(
                Value::String("dialer-proxy".into()),
                Value::String(POOL_GROUP_NAME.into()),
            );
            proxies.push(Value::Mapping(exit_map));
            has_chain_exit = true;
        }
    }
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

    // Top-level Outbound group: VERTEX-OUTBOUND (hot-switches between VERTEX-POOL and VERTEX-CHAIN-EXIT)
    let mut outbound_group = Mapping::new();
    outbound_group.insert(
        Value::String("name".into()),
        Value::String(OUTBOUND_GROUP_NAME.into()),
    );
    outbound_group.insert(
        Value::String("type".into()),
        Value::String("select".into()),
    );
    let outbound_choices = if has_chain_exit {
        if cfg.enable_chain_proxy {
            vec![
                Value::String(CHAIN_EXIT_PROXY_NAME.into()),
                Value::String(POOL_GROUP_NAME.into()),
            ]
        } else {
            vec![
                Value::String(POOL_GROUP_NAME.into()),
                Value::String(CHAIN_EXIT_PROXY_NAME.into()),
            ]
        }
    } else {
        vec![Value::String(POOL_GROUP_NAME.into())]
    };
    outbound_group.insert(
        Value::String("proxies".into()),
        Value::Sequence(outbound_choices),
    );

    // Proxy group: VERTEX-POOL (front-end relay pool / direct rotation pool)
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
        Value::Sequence(vec![
            Value::Mapping(outbound_group),
            Value::Mapping(pool_group),
        ]),
    );

    // Rules: watch domains -> RUST-429-MITM, local -> DIRECT, rest -> VERTEX-OUTBOUND
    let mut rules = vec![
        Value::String("IP-CIDR,127.0.0.0/8,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,192.168.0.0/16,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,10.0.0.0/8,DIRECT,no-resolve".into()),
    ];

    if enable_mitm {
        for dom in &cfg.watch_domains {
            let trimmed = dom.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("*.") {
                let clean = trimmed.trim_start_matches("*.");
                if !clean.is_empty() {
                    // Reject QUIC (UDP/443) on watch domains so clients fall back to TCP HTTPS for MITM
                    rules.push(Value::String(format!(
                        "AND,((NETWORK,UDP),(DST-PORT,443),(DOMAIN-SUFFIX,{})),REJECT",
                        clean
                    )));
                    rules.push(Value::String(format!(
                        "DOMAIN-SUFFIX,{},{}",
                        clean, MITM_PROXY_NAME
                    )));
                }
            } else if trimmed.contains('*') {
                rules.push(Value::String(format!(
                    "AND,((NETWORK,UDP),(DST-PORT,443),(DOMAIN-WILDCARD,{})),REJECT",
                    trimmed
                )));
                rules.push(Value::String(format!(
                    "DOMAIN-WILDCARD,{},{}",
                    trimmed, MITM_PROXY_NAME
                )));
            } else {
                rules.push(Value::String(format!(
                    "AND,((NETWORK,UDP),(DST-PORT,443),(DOMAIN-SUFFIX,{})),REJECT",
                    trimmed
                )));
                rules.push(Value::String(format!(
                    "DOMAIN-SUFFIX,{},{}",
                    trimmed, MITM_PROXY_NAME
                )));
            }
        }
    }
    rules.push(Value::String(format!("MATCH,{}", OUTBOUND_GROUP_NAME)));
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
        let runtime_yaml = generate_mihomo_config(&parsed, &cfg, "secret123", true, true).unwrap();

        assert!(runtime_yaml.contains("vertex-bypass"));
        assert!(runtime_yaml.contains("port: 17892"));
        assert!(runtime_yaml.contains("proxy: VERTEX-OUTBOUND"));
        assert!(runtime_yaml.contains("RUST-429-MITM"));
        assert!(runtime_yaml.contains("alias.example.net: real.example.net"));
        assert!(runtime_yaml.contains("DOMAIN-SUFFIX,aiplatform.googleapis.com,RUST-429-MITM"));
        assert!(runtime_yaml.contains("MATCH,VERTEX-OUTBOUND"));
    }

    #[test]
    fn test_chain_proxy_subscription_and_custom_exit() {
        let parsed = parse_clash_yaml(SAMPLE_CLASH_YAML).unwrap();
        let mut cfg = AppConfig::default();
        cfg.enable_chain_proxy = true;
        cfg.chain_exit_source = Some(ChainExitSource::SubscriptionNode {
            name: "JP-Tokyo-02".into(),
        });

        let yaml = generate_mihomo_config(&parsed, &cfg, "sec", false, false).unwrap();
        assert!(yaml.contains("name: VERTEX-CHAIN-EXIT"));
        assert!(yaml.contains("dialer-proxy: VERTEX-POOL"));
        assert!(yaml.contains("server: 203.0.113.20"));

        // Custom socks5 URL with credentials
        let socks_map = parse_custom_exit_proxy("socks5://alice:s3cr3t@198.51.100.7:1080").unwrap();
        assert_eq!(
            socks_map.get(Value::String("type".into())).and_then(|v| v.as_str()),
            Some("socks5")
        );
        assert_eq!(
            socks_map.get(Value::String("server".into())).and_then(|v| v.as_str()),
            Some("198.51.100.7")
        );
        assert_eq!(
            socks_map.get(Value::String("port".into())).and_then(|v| v.as_u64()),
            Some(1080)
        );
        assert_eq!(
            socks_map.get(Value::String("username".into())).and_then(|v| v.as_str()),
            Some("alice")
        );

        // Custom single Clash YAML node
        let yaml_map = parse_custom_exit_proxy(
            "{name: MyHome, type: vless, server: home.example.com, port: 443, uuid: abc}",
        )
        .unwrap();
        assert_eq!(
            yaml_map.get(Value::String("type".into())).and_then(|v| v.as_str()),
            Some("vless")
        );
        assert_eq!(
            yaml_map.get(Value::String("server".into())).and_then(|v| v.as_str()),
            Some("home.example.com")
        );
    }
}
