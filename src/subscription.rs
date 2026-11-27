use crate::config::{AppConfig, ChainExitSource};
use anyhow::{anyhow, Context, Result};
use serde_yaml::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Duration;

pub const POOL_GROUP_NAME: &str = "VERTEX-POOL";
pub const AUTO_GROUP_NAME: &str = "VERTEX-AUTO";
pub const OUTBOUND_GROUP_NAME: &str = "VERTEX-OUTBOUND";
pub const CHAIN_EXIT_PROXY_NAME: &str = "VERTEX-CHAIN-EXIT";
pub const POOL_RELAY_PROXY_NAME: &str = "VERTEX-POOL-RELAY";
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
    pub raw_hosts: Option<Mapping>,
    pub fake_ip_filter: Vec<String>,
    pub raw_geox_url: Option<Mapping>,
    pub raw_rule_providers: Option<Mapping>,
    pub raw_rules: Vec<String>,
    pub adblock_groups: HashSet<String>,
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
            .user_agent("FlClash/v0.8.88 clash-verge mihomo/1.19 Platform/windows");
        if let Some(p) = proxy_port {
            if let Ok(proxy) = reqwest::Proxy::all(format!("http://127.0.0.1:{}", p)) {
                builder = builder.proxy(proxy);
            }
        } else {
            builder = builder.no_proxy();
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

/// Checks whether a subscription node entry is a provider-injected notice/banner pseudo-node.
fn is_notice_pseudo_node(name: &str, proxy_type: &str, server: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == MITM_PROXY_NAME {
        return true;
    }
    let pt = proxy_type.trim().to_ascii_lowercase();
    if pt == "reject" || pt == "direct" || pt == "pass" {
        return true;
    }
    let srv = server.trim();
    if srv.is_empty() || srv == "127.0.0.1" || srv == "0.0.0.0" || srv == "localhost" {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    const BANNER_KEYWORDS: &[&str] = &[
        "剩余流量",
        "套餐到期",
        "到期时间",
        "距离下次重置",
        "流量重置",
        "官网",
        "更新订阅",
        "激活订阅",
        "禁止使用",
        "订阅托管",
        "订阅转换",
        "防失联",
        "发布页",
        "全红更新",
        "超时请",
        "故障请",
        "官方群",
        "TG群",
        "频道",
    ];
    BANNER_KEYWORDS
        .iter()
        .any(|kw| trimmed.contains(kw) || lower.contains(&kw.to_ascii_lowercase()))
}

pub fn parse_clash_yaml(yaml_text: &str) -> Result<ParsedSubscription> {
    let doc: Value = serde_yaml::from_str(yaml_text)
        .context("解析 Clash/Mihomo YAML 失败，请确认订阅链接返回的是标准 Clash YAML 格式")?;

    let proxies_seq = doc
        .get("proxies")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| anyhow!("YAML 中未找到有效的 proxies 列表"))?;

    // Preserve full `hosts:` mapping (both string aliases and IP list arrays like `[223.5.5.5, 223.6.6.6]`)
    let mut hosts = HashMap::new();
    let raw_hosts = doc.get("hosts").and_then(|v| v.as_mapping()).cloned();
    if let Some(ref hosts_map) = raw_hosts {
        for (k, v) in hosts_map {
            if let Some(ks) = k.as_str() {
                if let Some(vs) = v.as_str() {
                    hosts.insert(ks.trim().to_string(), vs.trim().to_string());
                } else if let Some(seq) = v.as_sequence() {
                    if let Some(first_ip) = seq.iter().find_map(|x| x.as_str()) {
                        hosts.insert(ks.trim().to_string(), first_ip.trim().to_string());
                    }
                }
            }
        }
    }

    // Preserve provider-specific `dns.fake-ip-filter` entries
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

    let raw_geox_url = doc.get("geox-url").and_then(|v| v.as_mapping()).cloned();
    let raw_rule_providers = doc
        .get("rule-providers")
        .and_then(|v| v.as_mapping())
        .cloned();

    // Detect adblock / reject proxy-groups in the original subscription so rules targeting them stay REJECT
    let mut adblock_groups = HashSet::new();
    if let Some(groups_seq) = doc.get("proxy-groups").and_then(|v| v.as_sequence()) {
        for g in groups_seq {
            let Some(gmap) = g.as_mapping() else { continue };
            let Some(gname) = gmap.get(Value::String("name".into())).and_then(|v| v.as_str()) else {
                continue;
            };
            let gl = gname.to_ascii_lowercase();
            let first_proxy = gmap
                .get(Value::String("proxies".into()))
                .and_then(|v| v.as_sequence())
                .and_then(|s| s.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if gname.contains("拦截")
                || gname.contains("广告")
                || gl.contains("adblock")
                || gl.contains("reject")
                || gl.contains("hijack")
                || first_proxy.eq_ignore_ascii_case("REJECT")
                || first_proxy.eq_ignore_ascii_case("REJECT-DROP")
            {
                adblock_groups.insert(gname.trim().to_string());
            }
        }
    }

    let mut raw_rules = Vec::new();
    if let Some(rules_seq) = doc.get("rules").and_then(|v| v.as_sequence()) {
        for r in rules_seq {
            if let Some(s) = r.as_str() {
                let clean = s.trim();
                if !clean.is_empty() {
                    raw_rules.push(clean.to_string());
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
        let server = hosts.get(&raw_server).cloned().unwrap_or(raw_server);
        let port = map
            .get(Value::String("port".into()))
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
            })
            .unwrap_or(0) as u16;

        if name == POOL_RELAY_PROXY_NAME {
            raw_proxies.push(item.clone());
            continue;
        }

        if port == 0 || is_notice_pseudo_node(name, &proxy_type, &server) {
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
        raw_hosts,
        fake_ip_filter,
        raw_geox_url,
        raw_rule_providers,
        raw_rules,
        adblock_groups,
    })
}

/// Parses a user-supplied custom landing/exit proxy specification.
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

    if !trimmed.contains('\n') && !trimmed.contains(' ') {
        let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
        if parts.len() == 4 {
            if let Ok(port) = parts[1].trim().parse::<u16>() {
                if !parts[0].trim().is_empty() && port > 0 {
                    let mut map = Mapping::new();
                    map.insert(
                        Value::String("name".into()),
                        Value::String(CHAIN_EXIT_PROXY_NAME.into()),
                    );
                    map.insert(Value::String("type".into()), Value::String("http".into()));
                    map.insert(
                        Value::String("server".into()),
                        Value::String(parts[0].trim().into()),
                    );
                    map.insert(Value::String("port".into()), Value::Number(port.into()));
                    if !parts[2].is_empty() {
                        map.insert(
                            Value::String("username".into()),
                            Value::String(parts[2].to_string()),
                        );
                    }
                    if !parts[3].is_empty() {
                        map.insert(
                            Value::String("password".into()),
                            Value::String(parts[3].to_string()),
                        );
                    }
                    return Ok(map);
                }
            }
        }
    }

    let val: Value = serde_yaml::from_str(trimmed).context(
        "解析自定义落地节点失败，请输入 socks5://... / http://... / host:port:user:pass 或单条 Clash 节点 YAML",
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

fn derive_pool_node_name(
    index: usize,
    server: &str,
    password: Option<&str>,
    custom_label: Option<&str>,
) -> String {
    if let Some(lbl) = custom_label {
        let clean = lbl.trim();
        if !clean.is_empty() {
            return clean.to_string();
        }
    }
    if let Some(pass) = password {
        let segs: Vec<&str> = pass.split('-').collect();
        if segs.len() >= 3 {
            for (i, seg) in segs.iter().enumerate() {
                if seg.len() == 2 && seg.chars().all(|c| c.is_ascii_uppercase()) && i + 1 < segs.len() {
                    let tail = segs[i..].join("-");
                    return format!("{} #{:02}", tail, index + 1);
                }
            }
        } else if segs.len() == 2
            && segs[1].len() == 2
            && segs[1].chars().all(|c| c.is_ascii_uppercase())
        {
            return format!("{}-Pool #{:02}", segs[1], index + 1);
        }
    }
    let host_short = server.split('.').next().unwrap_or(server);
    format!("{} #{:02}", host_short, index + 1)
}

pub fn parse_proxy_pool_to_clash_yaml(
    raw_text: &str,
    default_protocol: &str,
    relay: Option<(&ParsedSubscription, &str)>,
) -> Result<(String, ParsedSubscription)> {
    let proto = match default_protocol.trim().to_ascii_lowercase().as_str() {
        "socks5" | "socks5h" => "socks5",
        _ => "http",
    };

    let mut proxies_seq: Vec<Value> = Vec::new();
    let mut has_relay = false;
    if let Some((relay_sub, relay_node_name)) = relay {
        let mut relay_map = relay_sub
            .raw_proxies
            .iter()
            .filter_map(|v| v.as_mapping())
            .find(|m| {
                m.get(Value::String("name".into()))
                    .and_then(|v| v.as_str())
                    == Some(relay_node_name)
            })
            .cloned()
            .ok_or_else(|| anyhow!("未找到前置中转节点「{}」", relay_node_name))?;
        relay_map.insert(
            Value::String("name".into()),
            Value::String(POOL_RELAY_PROXY_NAME.into()),
        );
        proxies_seq.push(Value::Mapping(relay_map));
        has_relay = true;
    }

    let mut used_names = HashSet::new();
    let mut exit_count = 0usize;

    for (line_idx, raw_line) in raw_text.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        let (entry, custom_label) = match trimmed.split_once(" #") {
            Some((left, right)) => (left.trim(), Some(right.trim())),
            None => (trimmed, None),
        };

        let lower = entry.to_ascii_lowercase();
        let mut map = if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("socks5://")
            || lower.starts_with("socks5h://")
        {
            parse_custom_exit_proxy(entry)
                .with_context(|| format!("第 {} 行代理 URL 解析失败: {}", line_idx + 1, entry))?
        } else {
            let parts: Vec<&str> = entry.splitn(4, ':').collect();
            if parts.len() == 4 {
                let host = parts[0].trim();
                let port: u16 = parts[1]
                    .trim()
                    .parse()
                    .with_context(|| format!("第 {} 行端口无效: {}", line_idx + 1, parts[1]))?;
                if host.is_empty() || port == 0 {
                    return Err(anyhow!("第 {} 行主机或端口无效", line_idx + 1));
                }
                let mut m = Mapping::new();
                m.insert(Value::String("type".into()), Value::String(proto.into()));
                m.insert(Value::String("server".into()), Value::String(host.into()));
                m.insert(Value::String("port".into()), Value::Number(port.into()));
                if !parts[2].is_empty() {
                    m.insert(
                        Value::String("username".into()),
                        Value::String(parts[2].to_string()),
                    );
                }
                if !parts[3].is_empty() {
                    m.insert(
                        Value::String("password".into()),
                        Value::String(parts[3].to_string()),
                    );
                }
                m
            } else if parts.len() == 2 {
                let host = parts[0].trim();
                let port: u16 = parts[1]
                    .trim()
                    .parse()
                    .with_context(|| format!("第 {} 行端口无效: {}", line_idx + 1, parts[1]))?;
                if host.is_empty() || port == 0 {
                    return Err(anyhow!("第 {} 行主机或端口无效", line_idx + 1));
                }
                let mut m = Mapping::new();
                m.insert(Value::String("type".into()), Value::String(proto.into()));
                m.insert(Value::String("server".into()), Value::String(host.into()));
                m.insert(Value::String("port".into()), Value::Number(port.into()));
                m
            } else {
                return Err(anyhow!(
                    "第 {} 行格式不支持（支持 host:port:user:pass 或 http://user:pass@host:port）",
                    line_idx + 1
                ));
            }
        };

        let server_str = map
            .get(Value::String("server".into()))
            .and_then(|v| v.as_str())
            .unwrap_or("proxy")
            .to_string();
        let pass_str = map
            .get(Value::String("password".into()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let base_name = derive_pool_node_name(
            exit_count,
            &server_str,
            pass_str.as_deref(),
            custom_label,
        );
        let mut final_name = base_name.clone();
        let mut dup = 2usize;
        while !used_names.insert(final_name.clone()) {
            final_name = format!("{} ({})", base_name, dup);
            dup += 1;
        }
        map.insert(Value::String("name".into()), Value::String(final_name));
        if has_relay {
            map.insert(
                Value::String("dialer-proxy".into()),
                Value::String(POOL_RELAY_PROXY_NAME.into()),
            );
        }
        proxies_seq.push(Value::Mapping(map));
        exit_count += 1;
    }

    if exit_count == 0 {
        return Err(anyhow!("未解析到任何有效代理节点，请检查输入格式"));
    }

    let mut root = Mapping::new();
    if let Some((relay_sub, _)) = relay {
        if let Some(ref rh) = relay_sub.raw_hosts {
            root.insert(Value::String("hosts".into()), Value::Mapping(rh.clone()));
        } else if !relay_sub.hosts.is_empty() {
            let mut hosts_map = Mapping::new();
            for (k, v) in &relay_sub.hosts {
                hosts_map.insert(Value::String(k.clone()), Value::String(v.clone()));
            }
            root.insert(Value::String("hosts".into()), Value::Mapping(hosts_map));
        }
        if !relay_sub.fake_ip_filter.is_empty() {
            let mut dns_map = Mapping::new();
            dns_map.insert(
                Value::String("fake-ip-filter".into()),
                Value::Sequence(
                    relay_sub
                        .fake_ip_filter
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
            root.insert(Value::String("dns".into()), Value::Mapping(dns_map));
        }
    }
    root.insert(Value::String("proxies".into()), Value::Sequence(proxies_seq));
    let yaml_text = serde_yaml::to_string(&Value::Mapping(root))
        .context("序列化流量池 YAML 失败")?;
    let parsed = parse_clash_yaml(&yaml_text)?;
    Ok((yaml_text, parsed))
}

pub fn resolve_chain_exit_mappings(
    sub: &ParsedSubscription,
    src: &ChainExitSource,
) -> Result<Vec<Mapping>> {
    match src {
        ChainExitSource::SubscriptionNode { name, profile_id } => {
            let find_in = |s: &ParsedSubscription| {
                s.raw_proxies
                    .iter()
                    .filter_map(|v| v.as_mapping())
                    .find(|m| {
                        m.get(Value::String("name".into()))
                            .and_then(|v| v.as_str())
                            == Some(name.as_str())
                    })
                    .cloned()
            };

            let mut found = find_in(sub);
            if found.is_none() {
                if let Some(pid) = profile_id {
                    if let Some(other_sub) =
                        load_cached_subscription_from(&AppConfig::profile_cache_path(pid))
                    {
                        found = find_in(&other_sub);
                    }
                }
            }
            if found.is_none() {
                if let Ok(entries) = fs::read_dir(AppConfig::profiles_dir()) {
                    for entry in entries.flatten() {
                        if let Some(other_sub) = load_cached_subscription_from(&entry.path()) {
                            if let Some(m) = find_in(&other_sub) {
                                found = Some(m);
                                break;
                            }
                        }
                    }
                }
            }

            let mut map = found.ok_or_else(|| anyhow!("未找到落地节点「{}」", name))?;
            map.remove(Value::String("dialer-proxy".into()));
            Ok(vec![map])
        }
        ChainExitSource::ProfilePool {
            profile_id,
            profile_name,
        } => {
            let pool_sub = load_cached_subscription_from(&AppConfig::profile_cache_path(profile_id))
                .ok_or_else(|| anyhow!("未找到落地流量池配置「{}」", profile_name))?;
            let mut list = Vec::new();
            for v in &pool_sub.raw_proxies {
                let Some(mut map) = v.as_mapping().cloned() else {
                    continue;
                };
                let n = map
                    .get(Value::String("name".into()))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if n.is_empty() || n == POOL_RELAY_PROXY_NAME {
                    continue;
                }
                map.remove(Value::String("dialer-proxy".into()));
                list.push(map);
            }
            if list.is_empty() {
                return Err(anyhow!("落地流量池「{}」中没有可用节点", profile_name));
            }
            Ok(list)
        }
        ChainExitSource::CustomSpec { raw } => Ok(vec![parse_custom_exit_proxy(raw)?]),
    }
}

pub fn resolve_chain_exit_pool_names(
    sub: &ParsedSubscription,
    src: &ChainExitSource,
) -> Vec<String> {
    let Ok(mappings) = resolve_chain_exit_mappings(sub, src) else {
        return Vec::new();
    };
    if mappings.len() <= 1 {
        return Vec::new();
    }
    mappings
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let orig = m
                .get(Value::String("name".into()))
                .and_then(|v| v.as_str())
                .unwrap_or("exit");
            format!("CHAIN-EXIT-{:02} {}", i + 1, orig)
        })
        .collect()
}

pub fn resolve_chain_exit_mapping(
    sub: &ParsedSubscription,
    src: &ChainExitSource,
) -> Result<Mapping> {
    let mut list = resolve_chain_exit_mappings(sub, src)?;
    Ok(list.remove(0))
}

fn map_rule_target<'a>(target: &'a str, adblock_groups: &HashSet<String>) -> &'a str {
    let t = target.trim();
    if t.eq_ignore_ascii_case("DIRECT") {
        "DIRECT"
    } else if t.eq_ignore_ascii_case("REJECT")
        || t.eq_ignore_ascii_case("REJECT-DROP")
        || t.eq_ignore_ascii_case("PASS")
    {
        t
    } else if adblock_groups.contains(t) {
        "REJECT"
    } else {
        OUTBOUND_GROUP_NAME
    }
}

/// Rewrites an original Clash/Mihomo routing rule so `DIRECT` and `REJECT` rules are preserved
/// intact while any provider-specific proxy group target routes cleanly into `VERTEX-OUTBOUND`.
fn rewrite_subscription_rule(
    raw_rule: &str,
    adblock_groups: &HashSet<String>,
    has_geoip_db: bool,
    has_geosite_db: bool,
) -> Option<String> {
    let trimmed = raw_rule.trim();
    if trimmed.is_empty() {
        return None;
    }

    let upper_prefix = trimmed
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();

    if !has_geoip_db && upper_prefix == "GEOIP" {
        // Only `GEOIP,LAN` works built-in without an external MMDB/MetaDB file
        let second = trimmed.split(',').nth(1).unwrap_or("").trim();
        if !second.eq_ignore_ascii_case("private") && !second.eq_ignore_ascii_case("lan") {
            return None;
        }
    }
    if !has_geosite_db && upper_prefix == "GEOSITE" {
        return None;
    }
    // Skip SUB-RULE blocks since sub-rules definitions are not carried over
    if upper_prefix == "SUB-RULE" {
        return None;
    }

    // Handle compound logic rules: `AND,((...),(...)),TARGET` / `OR` / `NOT`
    if matches!(upper_prefix.as_str(), "AND" | "OR" | "NOT") {
        if let Some(first_comma) = trimmed.find(',') {
            let after_type = &trimmed[first_comma + 1..];
            let mut depth = 0i32;
            let mut cond_end = None;
            for (idx, ch) in after_type.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            cond_end = Some(idx + ch.len_utf8());
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end_idx) = cond_end {
                let cond_part = &after_type[..end_idx];
                let rest = after_type[end_idx..].trim_start_matches(',').trim();
                if !rest.is_empty() {
                    let mut rest_parts = rest.splitn(2, ',');
                    let target = rest_parts.next().unwrap_or("");
                    let mapped = map_rule_target(target, adblock_groups);
                    return Some(match rest_parts.next() {
                        Some(opts) if !opts.trim().is_empty() => {
                            format!("{},{},{},{}", upper_prefix, cond_part, mapped, opts.trim())
                        }
                        _ => format!("{},{},{}", upper_prefix, cond_part, mapped),
                    });
                }
            }
        }
        return None;
    }

    let parts: Vec<&str> = trimmed.split(',').map(|s| s.trim()).collect();
    if upper_prefix == "MATCH" {
        let target = parts.get(1).copied().unwrap_or(OUTBOUND_GROUP_NAME);
        let mapped = map_rule_target(target, adblock_groups);
        return Some(format!("MATCH,{}", mapped));
    }

    if parts.len() < 3 {
        return None;
    }

    let mapped = map_rule_target(parts[2], adblock_groups);
    if parts.len() == 3 {
        Some(format!("{},{},{}", parts[0], parts[1], mapped))
    } else {
        let tail = parts[3..].join(",");
        Some(format!("{},{},{},{}", parts[0], parts[1], mapped, tail))
    }
}

pub fn has_local_geoip_db() -> bool {
    let dir = AppConfig::data_dir();
    dir.join("geoip.metadb").exists()
        || dir.join("GeoIP.dat").exists()
        || dir.join("geoip.dat").exists()
        || dir.join("country.mmdb").exists()
        || dir.join("Country.mmdb").exists()
}

pub fn has_local_geosite_db() -> bool {
    let dir = AppConfig::data_dir();
    dir.join("geosite.dat").exists() || dir.join("GeoSite.dat").exists()
}

/// Generates a high-performance, loop-free Mihomo runtime YAML configuration:
/// - `mixed-port` (`cfg.mihomo_mixed_port` in standby, or `cfg.local_proxy_port` when active) serves
///   native HTTP + HTTPS + SOCKS5 + UDP with zero intermediate Rust bottleneck.
/// - `tcp-concurrent: true` + `unified-delay: true` + `keep-alive-interval: 30` match FlClash's core tuning.
/// - Preserves the subscription's native `rule-providers`, `rules` (domestic `DIRECT` & adblock `REJECT`),
///   and full `hosts` mappings, while routing proxy traffic through `VERTEX-OUTBOUND` and `watch_domains`
///   through `RUST-429-MITM`.
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
    root.insert(Value::String("allow-lan".into()), Value::Bool(cfg.allow_lan));
    root.insert(
        Value::String("bind-address".into()),
        Value::String(if cfg.allow_lan { "*".into() } else { "127.0.0.1".into() }),
    );
    root.insert(Value::String("unified-delay".into()), Value::Bool(true));
    root.insert(Value::String("tcp-concurrent".into()), Value::Bool(true));
    root.insert(
        Value::String("keep-alive-interval".into()),
        Value::Number(30.into()),
    );
    root.insert(
        Value::String("find-process-mode".into()),
        Value::String("strict".into()),
    );
    root.insert(Value::String("ipv6".into()), Value::Bool(false));
    root.insert(Value::String("mode".into()), Value::String("rule".into()));
    root.insert(
        Value::String("log-level".into()),
        Value::String("warning".into()),
    );
    root.insert(
        Value::String("external-controller".into()),
        Value::String(format!("127.0.0.1:{}", cfg.mihomo_api_port)),
    );
    root.insert(
        Value::String("secret".into()),
        Value::String(api_secret.to_string()),
    );

    let mut profile_block = Mapping::new();
    profile_block.insert(Value::String("store-selected".into()), Value::Bool(false));
    profile_block.insert(
        Value::String("store-fake-ip".into()),
        Value::Bool(enable_tun),
    );
    root.insert(Value::String("profile".into()), Value::Mapping(profile_block));

    // Fast domestic-accessible CDN mirrors for GeoIP / GeoSite so Mihomo never hangs on github.com
    let mut geox = sub.raw_geox_url.clone().unwrap_or_default();
    let insert_default_geox = |m: &mut Mapping, key: &str, url: &str| {
        if !m.contains_key(Value::String(key.into())) {
            m.insert(Value::String(key.into()), Value::String(url.into()));
        }
    };
    insert_default_geox(
        &mut geox,
        "geoip",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.metadb",
    );
    insert_default_geox(
        &mut geox,
        "geosite",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat",
    );
    insert_default_geox(
        &mut geox,
        "mmdb",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/country.mmdb",
    );
    root.insert(Value::String("geox-url".into()), Value::Mapping(geox));

    // Merge DoH bootstrap hosts with subscription `hosts:` (preserving array & string mappings)
    let mut hosts_map = sub.raw_hosts.clone().unwrap_or_default();
    for (k, v) in &sub.hosts {
        if !hosts_map.contains_key(Value::String(k.clone())) {
            hosts_map.insert(Value::String(k.clone()), Value::String(v.clone()));
        }
    }
    let insert_bootstrap_host = |m: &mut Mapping, domain: &str, ips: &[&str]| {
        if !m.contains_key(Value::String(domain.into())) {
            m.insert(
                Value::String(domain.into()),
                Value::Sequence(ips.iter().map(|ip| Value::String((*ip).into())).collect()),
            );
        }
    };
    insert_bootstrap_host(&mut hosts_map, "dns.alidns.com", &["223.5.5.5", "223.6.6.6"]);
    insert_bootstrap_host(&mut hosts_map, "doh.pub", &["1.12.12.12", "120.53.53.53"]);
    insert_bootstrap_host(&mut hosts_map, "cloudflare-dns.com", &["1.1.1.1", "1.0.0.1"]);
    insert_bootstrap_host(&mut hosts_map, "dns.google", &["8.8.8.8", "8.8.4.4"]);
    root.insert(Value::String("hosts".into()), Value::Mapping(hosts_map));

    // Split-horizon anti-pollution DNS:
    // - Use `fake-ip` ONLY when TUN is enabled (system proxy sends domain names via HTTP CONNECT / SOCKS5 directly).
    // - `proxy-server-nameserver` & `direct-nameserver` use encrypted domestic DoH + UDP fast fallback.
    // - `nameserver` uses encrypted DoH with `respect-rules: true` so overseas domains resolve cleanly without GFW poison.
    let mut dns = Mapping::new();
    dns.insert(Value::String("enable".into()), Value::Bool(true));
    dns.insert(
        Value::String("cache-algorithm".into()),
        Value::String("arc".into()),
    );
    dns.insert(Value::String("prefer-h3".into()), Value::Bool(false));
    dns.insert(Value::String("ipv6".into()), Value::Bool(false));
    dns.insert(Value::String("use-hosts".into()), Value::Bool(true));
    dns.insert(Value::String("use-system-hosts".into()), Value::Bool(true));
    dns.insert(Value::String("respect-rules".into()), Value::Bool(true));
    dns.insert(
        Value::String("default-nameserver".into()),
        Value::Sequence(vec![
            Value::String("223.5.5.5".into()),
            Value::String("119.29.29.29".into()),
        ]),
    );
    let domestic_doh = vec![
        Value::String("https://dns.alidns.com/dns-query".into()),
        Value::String("https://doh.pub/dns-query".into()),
        Value::String("223.5.5.5".into()),
    ];
    dns.insert(
        Value::String("proxy-server-nameserver".into()),
        Value::Sequence(domestic_doh.clone()),
    );
    dns.insert(
        Value::String("direct-nameserver".into()),
        Value::Sequence(domestic_doh.clone()),
    );
    dns.insert(
        Value::String("nameserver".into()),
        Value::Sequence(vec![
            Value::String("https://cloudflare-dns.com/dns-query".into()),
            Value::String("https://dns.google/dns-query".into()),
        ]),
    );
    dns.insert(
        Value::String("fallback".into()),
        Value::Sequence(domestic_doh),
    );

    let has_geosite_db = has_local_geosite_db();
    if enable_tun {
        dns.insert(
            Value::String("enhanced-mode".into()),
            Value::String("fake-ip".into()),
        );
        dns.insert(
            Value::String("fake-ip-range".into()),
            Value::String("198.18.0.1/16".into()),
        );
        let mut filters: Vec<String> = vec![
            "*.lan".into(),
            "*.local".into(),
            "*.localhost".into(),
            "+.msftconnecttest.com".into(),
            "+.msftncsi.com".into(),
            "localhost.ptlogin2.qq.com".into(),
            "+.stun.*.*".into(),
            "+.stun.*.*.*".into(),
            "+.push.apple.com".into(),
        ];
        for f in &sub.fake_ip_filter {
            if !has_geosite_db && f.trim().to_ascii_lowercase().starts_with("geosite:") {
                continue;
            }
            if !filters.iter().any(|existing| existing.eq_ignore_ascii_case(f)) {
                filters.push(f.clone());
            }
        }
        dns.insert(
            Value::String("fake-ip-filter".into()),
            Value::Sequence(filters.into_iter().map(Value::String).collect()),
        );
    } else {
        dns.insert(
            Value::String("enhanced-mode".into()),
            Value::String("redir-host".into()),
        );
    }
    root.insert(Value::String("dns".into()), Value::Mapping(dns));

    // Sniffer: only needed when TUN or MITM is active; avoid `override-destination: true` on TLS so IP-based certs never break
    if enable_tun || enable_mitm {
        let mut sniffer = Mapping::new();
        sniffer.insert(Value::String("enable".into()), Value::Bool(true));
        sniffer.insert(
            Value::String("force-dns-mapping".into()),
            Value::Bool(enable_tun),
        );
        sniffer.insert(Value::String("parse-pure-ip".into()), Value::Bool(true));
        sniffer.insert(
            Value::String("override-destination".into()),
            Value::Bool(false),
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
        http_sniff.insert(
            Value::String("override-destination".into()),
            Value::Bool(true),
        );
        sniff_protos.insert(Value::String("HTTP".into()), Value::Mapping(http_sniff));
        sniffer.insert(Value::String("sniff".into()), Value::Mapping(sniff_protos));
        sniffer.insert(
            Value::String("skip-domain".into()),
            Value::Sequence(vec![
                Value::String("Mijia Cloud".into()),
                Value::String("+.push.apple.com".into()),
            ]),
        );
        root.insert(Value::String("sniffer".into()), Value::Mapping(sniffer));
    }

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

    // Loop-free bypass listener directly targeting VERTEX-OUTBOUND (used by Rust 429 MITM upstream)
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

    // Proxies = subscription proxies + optional VERTEX-CHAIN-EXIT + RUST-429-MITM
    let mut proxies = sub.raw_proxies.clone();
    let mut has_chain_exit = false;
    let mut chain_exit_pool_names: Vec<String> = Vec::new();
    if let Some(ref src) = cfg.chain_exit_source {
        if let Ok(exit_maps) = resolve_chain_exit_mappings(sub, src) {
            if exit_maps.len() == 1 {
                let mut exit_map = exit_maps.into_iter().next().unwrap();
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
            } else if exit_maps.len() > 1 {
                for (i, mut exit_map) in exit_maps.into_iter().enumerate() {
                    let orig = exit_map
                        .get(Value::String("name".into()))
                        .and_then(|v| v.as_str())
                        .unwrap_or("exit")
                        .to_string();
                    let proxy_name = format!("CHAIN-EXIT-{:02} {}", i + 1, orig);
                    exit_map.insert(
                        Value::String("name".into()),
                        Value::String(proxy_name.clone()),
                    );
                    exit_map.insert(
                        Value::String("dialer-proxy".into()),
                        Value::String(POOL_GROUP_NAME.into()),
                    );
                    proxies.push(Value::Mapping(exit_map));
                    chain_exit_pool_names.push(proxy_name);
                }
                has_chain_exit = true;
            }
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

    // Proxy groups:
    // 1. VERTEX-OUTBOUND (select: VERTEX-POOL or VERTEX-CHAIN-EXIT)
    // 2. VERTEX-AUTO (url-test: automatic lowest-latency node selection)
    // 3. VERTEX-POOL (select: defaults to VERTEX-AUTO until pinned/rotated onto a specific node)
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

    let all_node_values: Vec<Value> = sub
        .nodes
        .iter()
        .map(|n| Value::String(n.name.clone()))
        .collect();

    let mut auto_group = Mapping::new();
    auto_group.insert(
        Value::String("name".into()),
        Value::String(AUTO_GROUP_NAME.into()),
    );
    auto_group.insert(
        Value::String("type".into()),
        Value::String("url-test".into()),
    );
    auto_group.insert(
        Value::String("url".into()),
        Value::String("http://www.gstatic.com/generate_204".into()),
    );
    auto_group.insert(Value::String("interval".into()), Value::Number(180.into()));
    auto_group.insert(Value::String("tolerance".into()), Value::Number(30.into()));
    auto_group.insert(Value::String("lazy".into()), Value::Bool(true));
    auto_group.insert(
        Value::String("proxies".into()),
        Value::Sequence(all_node_values.clone()),
    );

    let mut pool_choices = Vec::with_capacity(all_node_values.len() + 1);
    pool_choices.push(Value::String(AUTO_GROUP_NAME.into()));
    pool_choices.extend(all_node_values);

    let mut pool_group = Mapping::new();
    pool_group.insert(
        Value::String("name".into()),
        Value::String(POOL_GROUP_NAME.into()),
    );
    pool_group.insert(
        Value::String("type".into()),
        Value::String("select".into()),
    );
    pool_group.insert(Value::String("proxies".into()), Value::Sequence(pool_choices));

    let mut proxy_groups = vec![
        Value::Mapping(outbound_group),
        Value::Mapping(pool_group),
        Value::Mapping(auto_group),
    ];
    if !chain_exit_pool_names.is_empty() {
        let mut chain_group = Mapping::new();
        chain_group.insert(
            Value::String("name".into()),
            Value::String(CHAIN_EXIT_PROXY_NAME.into()),
        );
        chain_group.insert(
            Value::String("type".into()),
            Value::String("select".into()),
        );
        chain_group.insert(
            Value::String("proxies".into()),
            Value::Sequence(chain_exit_pool_names.into_iter().map(Value::String).collect()),
        );
        proxy_groups.push(Value::Mapping(chain_group));
    }
    root.insert(
        Value::String("proxy-groups".into()),
        Value::Sequence(proxy_groups),
    );

    // Preserve subscription `rule-providers:` (sanitizing any custom `proxy:` reference)
    if let Some(ref rp_map) = sub.raw_rule_providers {
        let mut cleaned_rp = Mapping::new();
        for (k, v) in rp_map {
            if let Some(mut m) = v.as_mapping().cloned() {
                if let Some(p_str) = m
                    .get(Value::String("proxy".into()))
                    .and_then(|x| x.as_str())
                    .map(|s| s.trim().to_string())
                {
                    if !p_str.eq_ignore_ascii_case("DIRECT") {
                        m.insert(
                            Value::String("proxy".into()),
                            Value::String(OUTBOUND_GROUP_NAME.into()),
                        );
                    }
                }
                cleaned_rp.insert(k.clone(), Value::Mapping(m));
            } else {
                cleaned_rp.insert(k.clone(), v.clone());
            }
        }
        if !cleaned_rp.is_empty() {
            root.insert(
                Value::String("rule-providers".into()),
                Value::Mapping(cleaned_rp),
            );
        }
    }

    // Build routing rules:
    // 1. Local LAN CIDRs -> DIRECT
    // 2. Selective watch_domains -> RUST-429-MITM (only when MITM is enabled)
    // 3. Subscription's native split-tunneling rules (or GeoSite/GeoIP CN fallback for raw proxy pools)
    // 4. Final fallback `MATCH,VERTEX-OUTBOUND`
    let mut rules: Vec<Value> = vec![
        Value::String("IP-CIDR,127.0.0.0/8,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,192.168.0.0/16,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,10.0.0.0/8,DIRECT,no-resolve".into()),
        Value::String("IP-CIDR,172.16.0.0/12,DIRECT,no-resolve".into()),
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

    let is_global_mode = cfg.outbound_mode == "global";
    let has_geoip_db = has_local_geoip_db();

    if !is_global_mode {
        if !sub.raw_rules.is_empty() {
            let mut has_match = false;
            for raw_rule in &sub.raw_rules {
                if let Some(rewritten) = rewrite_subscription_rule(
                    raw_rule,
                    &sub.adblock_groups,
                    has_geoip_db,
                    has_geosite_db,
                ) {
                    if rewritten.starts_with("MATCH,") {
                        has_match = true;
                    }
                    rules.push(Value::String(rewritten));
                }
            }
            if !has_match {
                rules.push(Value::String(format!("MATCH,{}", OUTBOUND_GROUP_NAME)));
            }
        } else {
            // Raw proxy pool without embedded rules: provide domestic direct split-tunneling automatically
            rules.push(Value::String("DOMAIN-SUFFIX,cn,DIRECT".into()));
            if has_geosite_db {
                rules.push(Value::String("GEOSITE,private,DIRECT".into()));
                rules.push(Value::String("GEOSITE,cn,DIRECT".into()));
            }
            if has_geoip_db {
                rules.push(Value::String("GEOIP,CN,DIRECT,no-resolve".into()));
            }
            rules.push(Value::String(format!("MATCH,{}", OUTBOUND_GROUP_NAME)));
        }
    } else {
        rules.push(Value::String(format!("MATCH,{}", OUTBOUND_GROUP_NAME)));
    }

    root.insert(Value::String("rules".into()), Value::Sequence(rules));

    serde_yaml::to_string(&Value::Mapping(root)).context("序列化 mihomo 运行配置失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CLASH_YAML: &str = r#"
hosts:
  alias.example.net: real.example.net
  dns.alidns.com: [223.5.5.5, 223.6.6.6]
proxies:
  - name: "剩余流量: 100GB"
    type: ss
    server: 1.1.1.1
    port: 443
  - name: "大量节点超时请「更新订阅」"
    type: vmess
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
proxy-groups:
  - name: "🛑 广告拦截"
    type: select
    proxies: ["REJECT", "DIRECT"]
rules:
  - DOMAIN-SUFFIX,bilibili.com,DIRECT
  - DOMAIN-SUFFIX,doubleclick.net,🛑 广告拦截
  - DOMAIN-SUFFIX,google.com,Bitz Net
  - MATCH,Bitz Net
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

        assert!(runtime_yaml.contains("tcp-concurrent: true"));
        assert!(runtime_yaml.contains("vertex-bypass"));
        assert!(runtime_yaml.contains("port: 17892"));
        assert!(runtime_yaml.contains("proxy: VERTEX-OUTBOUND"));
        assert!(runtime_yaml.contains("RUST-429-MITM"));
        assert!(runtime_yaml.contains("VERTEX-AUTO"));
        assert!(runtime_yaml.contains("alias.example.net: real.example.net"));
        assert!(runtime_yaml.contains("DOMAIN-SUFFIX,aiplatform.googleapis.com,RUST-429-MITM"));
        assert!(runtime_yaml.contains("DOMAIN-SUFFIX,bilibili.com,DIRECT"));
        assert!(runtime_yaml.contains("DOMAIN-SUFFIX,doubleclick.net,REJECT"));
        assert!(runtime_yaml.contains("DOMAIN-SUFFIX,google.com,VERTEX-OUTBOUND"));
        assert!(runtime_yaml.contains("MATCH,VERTEX-OUTBOUND"));
    }

    #[test]
    fn test_chain_proxy_subscription_and_custom_exit() {
        let parsed = parse_clash_yaml(SAMPLE_CLASH_YAML).unwrap();
        let mut cfg = AppConfig::default();
        cfg.enable_chain_proxy = true;
        cfg.chain_exit_source = Some(ChainExitSource::SubscriptionNode {
            name: "JP-Tokyo-02".into(),
            profile_id: None,
        });

        let yaml = generate_mihomo_config(&parsed, &cfg, "sec", false, false).unwrap();
        assert!(yaml.contains("name: VERTEX-CHAIN-EXIT"));
        assert!(yaml.contains("dialer-proxy: VERTEX-POOL"));
        assert!(yaml.contains("server: 203.0.113.20"));

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

    #[test]
    fn test_parse_proxy_pool_lines() {
        let raw = r#"
            # test pool
            gate-hk.example.net:1000:user1:pass1-US-TLvodAAA
            gate-hk.example.net:1000:user1:pass1-US-TLvodAAB
            socks5://alice:secret@198.51.100.2:1080 # Custom Socks
        "#;
        let (yaml, parsed) = parse_proxy_pool_to_clash_yaml(raw, "http", None).unwrap();
        assert_eq!(parsed.nodes.len(), 3);
        assert_eq!(parsed.nodes[0].name, "US-TLvodAAA #01");
        assert_eq!(parsed.nodes[0].proxy_type, "http");
        assert_eq!(parsed.nodes[0].server, "gate-hk.example.net");
        assert_eq!(parsed.nodes[0].port, 1000);
        assert_eq!(parsed.nodes[1].name, "US-TLvodAAB #02");
        assert_eq!(parsed.nodes[2].name, "Custom Socks");
        assert_eq!(parsed.nodes[2].proxy_type, "socks5");
        assert!(yaml.contains("pass1-US-TLvodAAA"));

        let relay_sub = parse_clash_yaml(SAMPLE_CLASH_YAML).unwrap();
        let (relayed_yaml, relayed_parsed) =
            parse_proxy_pool_to_clash_yaml(raw, "http", Some((&relay_sub, "US-Oregon-01"))).unwrap();
        assert_eq!(relayed_parsed.nodes.len(), 3);
        assert_eq!(relayed_parsed.raw_proxies.len(), 4);
        assert!(relayed_yaml.contains("dialer-proxy: VERTEX-POOL-RELAY"));
        assert!(relayed_yaml.contains("alias.example.net: real.example.net"));
    }

    #[test]
    fn test_validate_cached_profiles_with_mihomo_binary() {
        let profiles_dir = AppConfig::profiles_dir();
        let Ok(entries) = std::fs::read_dir(&profiles_dir) else {
            return;
        };
        let cfg = AppConfig::load();
        let mihomo_bin = crate::mihomo::MihomoManager::resolve_binary_path(&cfg);

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
                continue;
            }
            let Some(parsed) = load_cached_subscription_from(&path) else {
                continue;
            };
            assert!(!parsed.nodes.is_empty());

            for (mitm, tun) in [(false, false), (true, false), (true, true)] {
                let mut test_cfg = cfg.clone();
                test_cfg.enable_chain_proxy = false;
                let yaml =
                    generate_mihomo_config(&parsed, &test_cfg, "test_secret", mitm, tun).unwrap();
                if let Some(ref bin) = mihomo_bin {
                    let tmp_yaml = std::env::temp_dir().join(format!(
                        "vp429_validate_{}_{}_{}.yaml",
                        entry.file_name().to_string_lossy(),
                        mitm as u8,
                        tun as u8
                    ));
                    std::fs::write(&tmp_yaml, &yaml).unwrap();
                    let out = std::process::Command::new(bin)
                        .arg("-t")
                        .arg("-d")
                        .arg(AppConfig::data_dir())
                        .arg("-f")
                        .arg(&tmp_yaml)
                        .output()
                        .unwrap();
                    let _ = std::fs::remove_file(&tmp_yaml);
                    assert!(
                        out.status.success(),
                        "mihomo -t failed for {:?} (mitm={}, tun={}): {}\n{}",
                        path,
                        mitm,
                        tun,
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
            }
        }
    }
}
