use crate::config::AppConfig;
use crate::mitm::ca::CertAuthority;
use crate::scheduler::NodeScheduler;
use crate::state::EventBus;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use dashmap::DashMap;
use http::{Request, Response, StatusCode, Version};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::{http1 as client_h1, http2 as client_h2};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_rustls::{TlsAcceptor, TlsConnector};

type ProxyBody = BoxBody<Bytes, hyper::Error>;
type PoolKey = (String, u16, u16, u64);

#[derive(Clone)]
pub struct MitmProxyContext {
    pub config: Arc<RwLock<AppConfig>>,
    pub ca: Arc<CertAuthority>,
    pub ca_installed: Arc<AtomicBool>,
    pub scheduler: Arc<NodeScheduler>,
    pub bus: Arc<EventBus>,
    pub upstream_tls: Arc<ClientConfig>,
    h2_pool: Arc<DashMap<PoolKey, client_h2::SendRequest<ProxyBody>>>,
    h1_pool: Arc<DashMap<PoolKey, Vec<client_h1::SendRequest<ProxyBody>>>>,
}

impl MitmProxyContext {
    pub fn new(
        config: Arc<RwLock<AppConfig>>,
        ca: Arc<CertAuthority>,
        ca_installed: Arc<AtomicBool>,
        scheduler: Arc<NodeScheduler>,
        bus: Arc<EventBus>,
        upstream_tls: Arc<ClientConfig>,
    ) -> Self {
        Self {
            config,
            ca,
            ca_installed,
            scheduler,
            bus,
            upstream_tls,
            h2_pool: Arc::new(DashMap::new()),
            h1_pool: Arc::new(DashMap::new()),
        }
    }

    pub fn build_default_upstream_tls(extra_roots: Vec<CertificateDer<'static>>) -> Arc<ClientConfig> {
        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        for cert in extra_roots {
            let _ = root_store.add(cert);
        }
        let mut cfg = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Arc::new(cfg)
    }

    fn evict_stale_epochs(&self, current_epoch: u64) {
        if self.h2_pool.len() > 32 {
            self.h2_pool.retain(|k, s| k.3 == current_epoch && s.is_ready());
        }
        if self.h1_pool.len() > 32 {
            self.h1_pool.retain(|k, _| k.3 == current_epoch);
        }
    }
}

pub async fn run_proxy_listener(
    bind_addr: String,
    force_mitm: bool,
    ctx: MitmProxyContext,
    shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let listener = TcpListener::bind(&bind_addr).await?;
    run_proxy_listener_on(listener, force_mitm, ctx, shutdown).await
}

pub async fn run_proxy_listener_on(
    listener: TcpListener,
    force_mitm: bool,
    ctx: MitmProxyContext,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let Ok((stream, _peer)) = accepted else {
                    continue;
                };
                let _ = stream.set_nodelay(true);
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_client_connection(stream, force_mitm, ctx.clone()).await {
                        let msg = err.to_string();
                        if !msg.contains("unexpected eof")
                            && !msg.contains("connection reset")
                            && !msg.contains("early eof")
                            && !msg.contains("broken pipe")
                        {
                            ctx.bus.error(format!("MITM 连接处理异常: {}", msg));
                        }
                    }
                });
            }
        }
    }
    Ok(())
}

async fn handle_client_connection(
    mut client: TcpStream,
    force_mitm: bool,
    ctx: MitmProxyContext,
) -> Result<()> {
    let mut header_buf = Vec::with_capacity(2048);
    let mut tmp = [0u8; 2048];
    let header_end;

    loop {
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        header_buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_header_end(&header_buf) {
            header_end = pos;
            break;
        }
        if header_buf.len() > 64 * 1024 {
            return Err(anyhow!("HTTP 请求头过大"));
        }
    }

    let head_str = String::from_utf8_lossy(&header_buf[..header_end]);
    let first_line = head_str.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_host_port(target, 443);
        let ca_ready = ctx.ca_installed.load(Ordering::Relaxed);
        let should_mitm = ca_ready
            && (force_mitm
                || ctx
                    .config
                    .read()
                    .map(|c| c.matches_watch_domain(&host))
                    .unwrap_or(false));

        if !should_mitm {
            let bypass_port = ctx
                .config
                .read()
                .map(|c| c.mihomo_bypass_port)
                .unwrap_or(17892);
            let mut upstream = TcpStream::connect(("127.0.0.1", bypass_port)).await?;
            let _ = upstream.set_nodelay(true);
            upstream.write_all(&header_buf).await?;
            let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            return Ok(());
        }

        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        let server_cfg = ctx.ca.server_config_for_host(&host)?;
        let acceptor = TlsAcceptor::from(server_cfg);
        let tls_stream = acceptor.accept(client).await.map_err(|e| {
            anyhow!(
                "{} 客户端拒绝 MITM 证书 ({})，请确认已安装根证书或将该域名移出监听列表",
                host,
                e
            )
        })?;

        let negotiated_h2 = tls_stream.get_ref().1.alpn_protocol() == Some(b"h2");
        let io = TokioIo::new(tls_stream);
        let host_arc = Arc::new(host);
        let host_for_err = host_arc.clone();
        let ctx_for_err = ctx.clone();
        let service = service_fn(move |req: Request<Incoming>| {
            let ctx = ctx.clone();
            let host = host_arc.clone();
            async move { handle_mitm_http_request(req, host, port, ctx).await }
        });

        let serve_res = if negotiated_h2 {
            hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
        } else {
            hyper::server::conn::http1::Builder::new()
                .preserve_header_case(true)
                .keep_alive(true)
                .serve_connection(io, service)
                .await
        };

        if let Err(err) = serve_res {
            let msg = err.to_string();
            if !msg.contains("connection closed")
                && !msg.contains("unexpected eof")
                && !msg.contains("early eof")
                && !msg.contains("error shutting down connection")
                && !msg.contains("stream no longer needed")
                && !msg.contains("not a result of an error")
            {
                ctx_for_err
                    .bus
                    .error(format!("[MITM] {} HTTP 会话异常: {}", host_for_err, msg));
            }
        }
        Ok(())
    } else {
        let bypass_port = ctx
            .config
            .read()
            .map(|c| c.mihomo_bypass_port)
            .unwrap_or(17892);
        let mut upstream = TcpStream::connect(("127.0.0.1", bypass_port)).await?;
        let _ = upstream.set_nodelay(true);
        upstream.write_all(&header_buf).await?;
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
        Ok(())
    }
}

fn build_upstream_request(
    parts: &http::request::Parts,
    body: ProxyBody,
    host: &str,
    port: u16,
    use_h2: bool,
) -> std::result::Result<Request<ProxyBody>, http::Error> {
    let pq = parts
        .uri
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or("/");
    let authority = if port == 443 {
        host.to_string()
    } else {
        format!("{}:{}", host, port)
    };

    let mut builder = if use_h2 {
        Request::builder()
            .method(parts.method.clone())
            .uri(format!("https://{}{}", authority, pq))
            .version(Version::HTTP_2)
    } else {
        Request::builder()
            .method(parts.method.clone())
            .uri(pq)
            .version(Version::HTTP_11)
    };

    if let Some(headers) = builder.headers_mut() {
        let mut saw_host = false;
        for (k, v) in &parts.headers {
            let kn = k.as_str();
            if kn.eq_ignore_ascii_case("proxy-connection")
                || (use_h2
                    && (kn.eq_ignore_ascii_case("connection")
                        || kn.eq_ignore_ascii_case("keep-alive")
                        || kn.eq_ignore_ascii_case("transfer-encoding")
                        || kn.eq_ignore_ascii_case("upgrade")
                        || kn.eq_ignore_ascii_case("host")))
            {
                continue;
            }
            if kn.eq_ignore_ascii_case("host") {
                saw_host = true;
            }
            headers.append(k.clone(), v.clone());
        }
        if !use_h2 && !saw_host {
            if let Ok(hv) = http::HeaderValue::from_str(&authority) {
                headers.insert(http::header::HOST, hv);
            }
        }
    }

    builder.body(body)
}

async fn handle_mitm_http_request(
    req: Request<Incoming>,
    host: Arc<String>,
    port: u16,
    ctx: MitmProxyContext,
) -> std::result::Result<Response<ProxyBody>, hyper::Error> {
    let (
        bypass_port,
        auto_retry,
        max_retries,
        max_body_bytes,
        intercept_codes,
        chain_proxy,
        chain_backoff_ms,
    ) = {
        let cfg = ctx.config.read().unwrap();
        (
            cfg.mihomo_bypass_port,
            cfg.auto_retry_on_429,
            cfg.max_retries,
            cfg.max_retry_body_bytes,
            cfg.intercept_status_codes.clone(),
            cfg.enable_chain_proxy && cfg.chain_exit_source.is_some(),
            cfg.chain_429_backoff_ms.clamp(100, 30_000),
        )
    };

    let (parts, incoming_body) = req.into_parts();
    let content_len = parts
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());

    let is_grpc_stream = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/grpc"))
        .unwrap_or(false);

    let can_buffer =
        auto_retry && !is_grpc_stream && content_len.map(|l| l <= max_body_bytes).unwrap_or(true);

    if !can_buffer {
        let used_node = if chain_proxy {
            ctx.scheduler
                .current_node_name()
                .unwrap_or_else(|| "DIRECT".to_string())
        } else {
            ctx.scheduler
                .prepare_for_request()
                .await
                .unwrap_or_else(|| "DIRECT".to_string())
        };
        let method_str = parts.method.to_string();
        return match send_upstream_streaming(
            &parts,
            incoming_body.boxed(),
            &host,
            port,
            bypass_port,
            &ctx,
        )
        .await
        {
            Ok(resp) => {
                let code = resp.status().as_u16();
                let is_grpc_429 = resp
                    .headers()
                    .get("grpc-status")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim() == "8")
                    .unwrap_or(false);
                if intercept_codes.contains(&code) || is_grpc_429 {
                    if chain_proxy {
                        if ctx.scheduler.has_chain_exit_pool() {
                            let _ = ctx.scheduler.report_429_chain_exit(&host).await;
                        } else {
                            ctx.scheduler.record_429_fixed_exit(&host);
                        }
                    } else {
                        let _ = ctx.scheduler.report_429(&used_node, &host).await;
                    }
                } else {
                    ctx.scheduler.record_success(&used_node, false);
                    ctx.bus.info(format!(
                        "[MITM] {} {} -> HTTP {} ({}: {})",
                        method_str,
                        host,
                        code,
                        if chain_proxy { "链式前置" } else { "节点" },
                        used_node
                    ));
                }
                Ok(resp.map(|b| b.boxed()))
            }
            Err(err) => Ok(error_response(
                StatusCode::BAD_GATEWAY,
                format!("Upstream error: {}", err),
            )),
        };
    }

    let body_bytes = match incoming_body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => return Err(e),
    };
    let total_attempts = if auto_retry { max_retries + 1 } else { 1 };

    let mut current_node = if chain_proxy {
        ctx.scheduler
            .current_node_name()
            .unwrap_or_else(|| "DIRECT".to_string())
    } else {
        ctx.scheduler
            .prepare_for_request()
            .await
            .unwrap_or_else(|| "DIRECT".to_string())
    };

    for attempt in 0..total_attempts {
        match send_upstream_buffered(&parts, &body_bytes, &host, port, bypass_port, &ctx).await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                let is_grpc_429 = resp
                    .headers()
                    .get("grpc-status")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim() == "8")
                    .unwrap_or(false);
                if intercept_codes.contains(&code) || is_grpc_429 {
                    if chain_proxy {
                        if ctx.scheduler.has_chain_exit_pool() {
                            let _ = ctx.scheduler.report_429_chain_exit(&host).await;
                            if attempt + 1 < total_attempts {
                                continue;
                            }
                            return Ok(resp.map(|b| b.boxed()));
                        }
                        ctx.scheduler.record_429_fixed_exit(&host);
                        if attempt + 1 < total_attempts {
                            ctx.bus.info(format!(
                                "{} 链式固定出口退避 {}ms 后进行重试 #{}",
                                host,
                                chain_backoff_ms,
                                attempt + 1
                            ));
                            tokio::time::sleep(std::time::Duration::from_millis(chain_backoff_ms))
                                .await;
                            continue;
                        }
                        return Ok(resp.map(|b| b.boxed()));
                    }

                    let next_node = ctx
                        .scheduler
                        .report_429(&current_node, &host)
                        .await
                        .unwrap_or_else(|| current_node.clone());

                    if attempt + 1 < total_attempts {
                        current_node = next_node;
                        continue;
                    }
                    return Ok(resp.map(|b| b.boxed()));
                } else {
                    let was_retry = attempt > 0;
                    ctx.scheduler.record_success(&current_node, was_retry);
                    if was_retry {
                        ctx.bus.ok(format!(
                            "{} 透明重试 #{} 成功 -> HTTP {} ({}: {})",
                            host,
                            attempt,
                            code,
                            if chain_proxy { "链式前置" } else { "节点" },
                            current_node
                        ));
                    } else {
                        ctx.bus.info(format!(
                            "[MITM] {} {} -> HTTP {} ({}: {})",
                            parts.method,
                            host,
                            code,
                            if chain_proxy { "链式前置" } else { "节点" },
                            current_node
                        ));
                    }
                    return Ok(resp.map(|b| b.boxed()));
                }
            }
            Err(err) => {
                ctx.bus.error(format!(
                    "上游连接失败 ({} via {}): {}",
                    host, current_node, err
                ));
                if attempt + 1 < total_attempts {
                    let reason = if chain_proxy {
                        "链式前置连接异常，自动切换前置节点"
                    } else {
                        "上游连接异常自动切换"
                    };
                    if let Some(next) = ctx.scheduler.rotate_next(reason).await {
                        current_node = next;
                        continue;
                    }
                }
                return Ok(error_response(
                    StatusCode::BAD_GATEWAY,
                    format!("Upstream proxy error: {}", err),
                ));
            }
        }
    }

    Ok(error_response(
        StatusCode::TOO_MANY_REQUESTS,
        "All node retry attempts exhausted".to_string(),
    ))
}

enum UpstreamConn {
    H2(client_h2::SendRequest<ProxyBody>),
    H1(client_h1::SendRequest<ProxyBody>),
}

async fn dial_upstream_conn(
    host: &str,
    port: u16,
    bypass_port: u16,
    tls_cfg: &Arc<ClientConfig>,
) -> Result<UpstreamConn> {
    let mut tcp = TcpStream::connect(("127.0.0.1", bypass_port)).await?;
    let _ = tcp.set_nodelay(true);

    let connect_req = format!(
        "CONNECT {0}:{1} HTTP/1.1\r\nHost: {0}:{1}\r\nProxy-Connection: keep-alive\r\n\r\n",
        host, port
    );
    tcp.write_all(connect_req.as_bytes()).await?;

    // Fast chunked read of the upstream HTTP CONNECT response (1 syscall instead of byte-by-byte)
    let mut resp_buf = Vec::with_capacity(256);
    let mut chunk = [0u8; 1024];
    loop {
        let n = tcp.read(&mut chunk).await?;
        if n == 0 {
            return Err(anyhow!("上游代理在 CONNECT 握手期间断开"));
        }
        resp_buf.extend_from_slice(&chunk[..n]);
        if find_header_end(&resp_buf).is_some() {
            break;
        }
        if resp_buf.len() > 8192 {
            return Err(anyhow!("上游 CONNECT 响应头过长"));
        }
    }
    let status_line = String::from_utf8_lossy(&resp_buf);
    if !status_line.contains(" 200") {
        return Err(anyhow!(
            "上游代理拒绝 CONNECT: {}",
            status_line.lines().next().unwrap_or("")
        ));
    }

    let server_name = ServerName::try_from(host.to_string())?;
    let connector = TlsConnector::from(tls_cfg.clone());
    let tls_stream = connector.connect(server_name, tcp).await?;
    let is_h2 = tls_stream.get_ref().1.alpn_protocol() == Some(b"h2");
    let io = TokioIo::new(tls_stream);

    if is_h2 {
        let (sender, conn) = client_h2::handshake(TokioExecutor::new(), io).await?;
        tokio::spawn(async move {
            let _ = conn.await;
        });
        Ok(UpstreamConn::H2(sender))
    } else {
        let (sender, conn) = client_h1::handshake(io).await?;
        tokio::spawn(async move {
            let _ = conn.await;
        });
        Ok(UpstreamConn::H1(sender))
    }
}

async fn send_upstream_buffered(
    parts: &http::request::Parts,
    body_bytes: &Bytes,
    host: &str,
    port: u16,
    bypass_port: u16,
    ctx: &MitmProxyContext,
) -> Result<Response<Incoming>> {
    let epoch = ctx.scheduler.route_epoch();
    ctx.evict_stale_epochs(epoch);
    let key: PoolKey = (host.to_ascii_lowercase(), port, bypass_port, epoch);

    // 1. Try pooled multiplexed HTTP/2 connection first (zero handshake latency!)
    let cached_h2 = ctx.h2_pool.get(&key).map(|entry| entry.value().clone());
    if let Some(mut h2_sender) = cached_h2 {
        if h2_sender.ready().await.is_ok() {
            let body = Full::new(body_bytes.clone())
                .map_err(|never| match never {})
                .boxed();
            let req = build_upstream_request(parts, body, host, port, true)?;
            match h2_sender.send_request(req).await {
                Ok(resp) => return Ok(resp),
                Err(_) => {
                    ctx.h2_pool.remove(&key);
                }
            }
        } else {
            ctx.h2_pool.remove(&key);
        }
    }

    // 2. Try pooled idle HTTP/1.1 Keep-Alive connection
    let cached_h1 = ctx
        .h1_pool
        .get_mut(&key)
        .and_then(|mut list| list.pop());
    if let Some(mut h1_sender) = cached_h1 {
        if h1_sender.ready().await.is_ok() {
            let body = Full::new(body_bytes.clone())
                .map_err(|never| match never {})
                .boxed();
            let req = build_upstream_request(parts, body, host, port, false)?;
            if let Ok(resp) = h1_sender.send_request(req).await {
                ctx.h1_pool.entry(key).or_default().push(h1_sender);
                return Ok(resp);
            }
        }
    }

    // 3. Dial fresh TLS connection through Mihomo bypass listener and store in pool
    match dial_upstream_conn(host, port, bypass_port, &ctx.upstream_tls).await? {
        UpstreamConn::H2(mut sender) => {
            ctx.h2_pool.insert(key, sender.clone());
            let body = Full::new(body_bytes.clone())
                .map_err(|never| match never {})
                .boxed();
            let req = build_upstream_request(parts, body, host, port, true)?;
            Ok(sender.send_request(req).await?)
        }
        UpstreamConn::H1(mut sender) => {
            let body = Full::new(body_bytes.clone())
                .map_err(|never| match never {})
                .boxed();
            let req = build_upstream_request(parts, body, host, port, false)?;
            let resp = sender.send_request(req).await?;
            ctx.h1_pool.entry(key).or_default().push(sender);
            Ok(resp)
        }
    }
}

async fn send_upstream_streaming(
    parts: &http::request::Parts,
    stream_body: ProxyBody,
    host: &str,
    port: u16,
    bypass_port: u16,
    ctx: &MitmProxyContext,
) -> Result<Response<Incoming>> {
    let epoch = ctx.scheduler.route_epoch();
    let key: PoolKey = (host.to_ascii_lowercase(), port, bypass_port, epoch);

    let cached_h2 = ctx.h2_pool.get(&key).map(|entry| entry.value().clone());
    if let Some(mut h2_sender) = cached_h2 {
        if h2_sender.ready().await.is_ok() {
            let req = build_upstream_request(parts, stream_body, host, port, true)?;
            return Ok(h2_sender.send_request(req).await?);
        }
        ctx.h2_pool.remove(&key);
    }

    match dial_upstream_conn(host, port, bypass_port, &ctx.upstream_tls).await? {
        UpstreamConn::H2(mut sender) => {
            ctx.h2_pool.insert(key, sender.clone());
            let req = build_upstream_request(parts, stream_body, host, port, true)?;
            Ok(sender.send_request(req).await?)
        }
        UpstreamConn::H1(mut sender) => {
            let req = build_upstream_request(parts, stream_body, host, port, false)?;
            let resp = sender.send_request(req).await?;
            ctx.h1_pool.entry(key).or_default().push(sender);
            Ok(resp)
        }
    }
}

fn error_response(status: StatusCode, msg: String) -> Response<ProxyBody> {
    let body = Full::new(Bytes::from(msg))
        .map_err(|never| match never {})
        .boxed();
    Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body)
        .unwrap()
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

fn split_host_port(target: &str, default_port: u16) -> (String, u16) {
    if let Some((h, p)) = target.rsplit_once(':') {
        if let Ok(port) = p.parse::<u16>() {
            return (h.trim_matches(['[', ']']).to_string(), port);
        }
    }
    (target.to_string(), default_port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RotationMode;
    use crate::mihomo::ProxySwitcher;
    use crate::subscription::ProxyNodeMeta;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    struct SharedNodeSwitcher {
        active: Arc<Mutex<String>>,
    }

    impl ProxySwitcher for SharedNodeSwitcher {
        fn select_node<'a>(
            &'a self,
            node_name: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
            let name = node_name.to_string();
            Box::pin(async move {
                *self.active.lock().unwrap() = name;
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn test_e2e_mitm_429_rotation_and_transparent_retry() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let temp_dir =
            std::env::temp_dir().join(format!("vertex_vpn_test_{}", rand::random::<u32>()));
        let ca = CertAuthority::load_or_create_in(&temp_dir).unwrap();

        let active_node_shared = Arc::new(Mutex::new("Node-A".to_string()));

        // 1. Mock HTTPS Vertex AI server supporting both HTTP/2 and HTTP/1.1
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = upstream_listener.local_addr().unwrap().port();
        let upstream_server_cfg = ca.server_config_for_host("aiplatform.googleapis.com").unwrap();
        let active_for_srv = active_node_shared.clone();

        tokio::spawn(async move {
            let acceptor = TlsAcceptor::from(upstream_server_cfg);
            while let Ok((stream, _)) = upstream_listener.accept().await {
                let acceptor = acceptor.clone();
                let active_for_srv = active_for_srv.clone();
                tokio::spawn(async move {
                    if let Ok(tls) = acceptor.accept(stream).await {
                        let is_h2 = tls.get_ref().1.alpn_protocol() == Some(b"h2");
                        let io = TokioIo::new(tls);
                        let svc = service_fn(move |_req: Request<Incoming>| {
                            let cur = active_for_srv.lock().unwrap().clone();
                            async move {
                                if cur == "Node-A" {
                                    Ok::<_, hyper::Error>(
                                        Response::builder()
                                            .status(StatusCode::TOO_MANY_REQUESTS)
                                            .body(Full::new(Bytes::from("quota exceeded")))
                                            .unwrap(),
                                    )
                                } else {
                                    Ok::<_, hyper::Error>(
                                        Response::builder()
                                            .status(StatusCode::OK)
                                            .body(Full::new(Bytes::from("vertex ok from Node-B")))
                                            .unwrap(),
                                    )
                                }
                            }
                        });
                        if is_h2 {
                            let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                                .serve_connection(io, svc)
                                .await;
                        } else {
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(io, svc)
                                .await;
                        }
                    }
                });
            }
        });

        // 2. Mock mihomo bypass CONNECT forwarder
        let bypass_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bypass_port = bypass_listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut client, _)) = bypass_listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 512];
                    while let Ok(n) = client.read(&mut tmp).await {
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        if find_header_end(&buf).is_some() {
                            break;
                        }
                    }
                    let _ = client
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await;
                    if let Ok(mut up) = TcpStream::connect(("127.0.0.1", upstream_port)).await {
                        let _ = tokio::io::copy_bidirectional(&mut client, &mut up).await;
                    }
                });
            }
        });

        // 3. Real HTTP/2-capable MitmProxy + NodeScheduler
        let bus = EventBus::new();
        let scheduler = NodeScheduler::new(RotationMode::On429Only, 60, bus.clone());
        scheduler.set_switcher(Some(Arc::new(SharedNodeSwitcher {
            active: active_node_shared.clone(),
        })));
        scheduler.load_nodes(vec![
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
        ]);

        let mut cfg = AppConfig::default();
        cfg.mihomo_bypass_port = bypass_port;
        cfg.auto_retry_on_429 = true;
        cfg.max_retries = 2;

        let upstream_tls = MitmProxyContext::build_default_upstream_tls(vec![ca.ca_der().clone()]);
        let ctx = MitmProxyContext::new(
            Arc::new(RwLock::new(cfg)),
            ca.clone(),
            Arc::new(AtomicBool::new(true)),
            scheduler.clone(),
            bus.clone(),
            upstream_tls.clone(),
        );

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = proxy_listener.local_addr().unwrap().port();

        let (shut_tx, shut_rx) = watch::channel(false);
        tokio::spawn(async move {
            let _ = run_proxy_listener_on(proxy_listener, true, ctx, shut_rx).await;
        });

        // 4. Client negotiates HTTP/2 over MITM tunnel and gets transparent 429 failover -> 200 OK
        let mut tcp = TcpStream::connect(("127.0.0.1", proxy_port)).await.unwrap();
        tcp.write_all(b"CONNECT aiplatform.googleapis.com:443 HTTP/1.1\r\nHost: aiplatform.googleapis.com:443\r\n\r\n")
            .await
            .unwrap();
        let mut connect_resp = Vec::new();
        let mut tmp = [0u8; 256];
        while let Ok(n) = tcp.read(&mut tmp).await {
            if n == 0 {
                break;
            }
            connect_resp.extend_from_slice(&tmp[..n]);
            if find_header_end(&connect_resp).is_some() {
                break;
            }
        }
        assert!(String::from_utf8_lossy(&connect_resp).contains("200"));

        let connector = TlsConnector::from(upstream_tls);
        let tls = connector
            .connect(
                ServerName::try_from("aiplatform.googleapis.com".to_string()).unwrap(),
                tcp,
            )
            .await
            .unwrap();
        assert_eq!(tls.get_ref().1.alpn_protocol(), Some(b"h2".as_slice()));

        let (mut sender, conn) = client_h2::handshake(TokioExecutor::new(), TokioIo::new(tls))
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });

        let req = Request::builder()
            .method("POST")
            .uri("https://aiplatform.googleapis.com/v1/projects/demo/locations/us-central1/publishers/google/models/gemini:streamGenerateContent")
            .version(Version::HTTP_2)
            .body(Full::new(Bytes::from(r#"{"contents":[{"text":"hello"}]}"#)))
            .unwrap();

        let resp = sender.send_request(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body, Bytes::from("vertex ok from Node-B"));

        let snap = scheduler.snapshot();
        assert_eq!(snap.active_node.as_deref(), Some("Node-B"));
        assert_eq!(snap.total_429, 1);
        assert_eq!(snap.total_retries_ok, 1);
        assert_eq!(snap.cooldown_count, 1);

        let _ = shut_tx.send(true);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
