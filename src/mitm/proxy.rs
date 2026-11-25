use crate::config::AppConfig;
use crate::mitm::ca::CertAuthority;
use crate::scheduler::NodeScheduler;
use crate::state::EventBus;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_rustls::{TlsAcceptor, TlsConnector};

type ProxyBody = BoxBody<Bytes, hyper::Error>;

#[derive(Clone)]
pub struct MitmProxyContext {
    pub config: Arc<RwLock<AppConfig>>,
    pub ca: Arc<CertAuthority>,
    pub scheduler: Arc<NodeScheduler>,
    pub bus: Arc<EventBus>,
    pub upstream_tls: Arc<ClientConfig>,
}

impl MitmProxyContext {
    pub fn build_default_upstream_tls(extra_roots: Vec<CertificateDer<'static>>) -> Arc<ClientConfig> {
        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        for cert in extra_roots {
            let _ = root_store.add(cert);
        }
        let mut cfg = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
        Arc::new(cfg)
    }
}

pub async fn run_proxy_listener(
    bind_addr: String,
    force_mitm: bool,
    ctx: MitmProxyContext,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let listener = TcpListener::bind(&bind_addr).await?;
    ctx.bus.info(format!(
        "代理监听已启动: {} ({})",
        bind_addr,
        if force_mitm { "TUN-MITM" } else { "HTTP/HTTPS" }
    ));

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
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_client_connection(stream, force_mitm, ctx.clone()).await {
                        let msg = err.to_string();
                        if !msg.contains("unexpected eof") && !msg.contains("connection reset") && !msg.contains("early eof") {
                            ctx.bus.error(format!("代理连接处理异常: {}", msg));
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
    let mut header_buf = Vec::with_capacity(4096);
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
        let should_mitm = force_mitm || {
            ctx.config
                .read()
                .map(|c| c.matches_watch_domain(&host))
                .unwrap_or(false)
        };

        if !should_mitm {
            // Non-watch domain: blind tunnel straight through mihomo mixed port (zero decryption)
            let mixed_port = ctx
                .config
                .read()
                .map(|c| c.mihomo_mixed_port)
                .unwrap_or(17890);
            let mut upstream = TcpStream::connect(("127.0.0.1", mixed_port)).await?;
            upstream.write_all(&header_buf).await?;
            let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            return Ok(());
        }

        // Selective HTTPS MITM for watch domains (e.g. Vertex AI *.googleapis.com)
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        let server_cfg = ctx.ca.server_config_for_host(&host)?;
        let acceptor = TlsAcceptor::from(server_cfg);
        let tls_stream = acceptor.accept(client).await?;

        let io = TokioIo::new(tls_stream);
        let host_arc = Arc::new(host);
        let service = service_fn(move |req: Request<Incoming>| {
            let ctx = ctx.clone();
            let host = host_arc.clone();
            async move { handle_mitm_http_request(req, host, port, ctx).await }
        });

        let _ = hyper::server::conn::http1::Builder::new()
            .preserve_header_case(true)
            .serve_connection(io, service)
            .await;
        Ok(())
    } else {
        // Plain HTTP proxy request: forward straight to mihomo mixed port
        let mixed_port = ctx
            .config
            .read()
            .map(|c| c.mihomo_mixed_port)
            .unwrap_or(17890);
        let mut upstream = TcpStream::connect(("127.0.0.1", mixed_port)).await?;
        upstream.write_all(&header_buf).await?;
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
        Ok(())
    }
}

async fn handle_mitm_http_request(
    req: Request<Incoming>,
    host: Arc<String>,
    port: u16,
    ctx: MitmProxyContext,
) -> std::result::Result<Response<ProxyBody>, hyper::Error> {
    let (bypass_port, auto_retry, max_retries, max_body_bytes, intercept_codes) = {
        let cfg = ctx.config.read().unwrap();
        (
            cfg.mihomo_bypass_port,
            cfg.auto_retry_on_429,
            cfg.max_retries,
            cfg.max_retry_body_bytes,
            cfg.intercept_status_codes.clone(),
        )
    };

    let (parts, incoming_body) = req.into_parts();
    let content_len = parts
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());

    let can_buffer = auto_retry && content_len.map(|l| l <= max_body_bytes).unwrap_or(true);

    let buffered_body: Option<Bytes> = if can_buffer {
        match incoming_body.collect().await {
            Ok(collected) => Some(collected.to_bytes()),
            Err(e) => return Err(e),
        }
    } else {
        // Too large to buffer; forward once without retry
        let used_node = ctx
            .scheduler
            .prepare_for_request()
            .await
            .unwrap_or_else(|| "DIRECT".to_string());
        let forward_req = Request::from_parts(parts, incoming_body.boxed());
        return match send_upstream_https(forward_req, &host, port, bypass_port, &ctx.upstream_tls)
            .await
        {
            Ok(resp) => {
                let code = resp.status().as_u16();
                if intercept_codes.contains(&code) {
                    let _ = ctx.scheduler.report_429(&used_node, &host).await;
                } else {
                    ctx.scheduler.record_success(&used_node, false);
                }
                Ok(resp.map(|b| b.boxed()))
            }
            Err(err) => Ok(error_response(
                StatusCode::BAD_GATEWAY,
                format!("Upstream error: {}", err),
            )),
        };
    };

    let body_bytes = buffered_body.unwrap_or_default();
    let total_attempts = if auto_retry { max_retries + 1 } else { 1 };

    let mut current_node = ctx
        .scheduler
        .prepare_for_request()
        .await
        .unwrap_or_else(|| "DIRECT".to_string());

    for attempt in 0..total_attempts {
        let mut req_builder = Request::builder()
            .method(parts.method.clone())
            .uri(parts.uri.clone())
            .version(http::Version::HTTP_11);

        if let Some(headers) = req_builder.headers_mut() {
            for (k, v) in &parts.headers {
                if !k.as_str().eq_ignore_ascii_case("proxy-connection") {
                    headers.append(k.clone(), v.clone());
                }
            }
        }

        let full_body = Full::new(body_bytes.clone())
            .map_err(|never| match never {})
            .boxed();
        let forward_req = match req_builder.body(full_body) {
            Ok(r) => r,
            Err(e) => {
                return Ok(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Build request error: {}", e),
                ))
            }
        };

        match send_upstream_https(forward_req, &host, port, bypass_port, &ctx.upstream_tls).await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                if intercept_codes.contains(&code) {
                    let next_node = ctx
                        .scheduler
                        .report_429(&current_node, &host)
                        .await
                        .unwrap_or_else(|| current_node.clone());

                    if attempt + 1 < total_attempts {
                        current_node = next_node;
                        continue;
                    }
                    // Exhausted retries; return the last 429 response to client
                    return Ok(resp.map(|b| b.boxed()));
                } else {
                    let was_retry = attempt > 0;
                    ctx.scheduler.record_success(&current_node, was_retry);
                    if was_retry {
                        ctx.bus.ok(format!(
                            "{} 透明重试 #{} 成功 -> HTTP {} (节点: {})",
                            host, attempt, code, current_node
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
                    if let Some(next) = ctx.scheduler.rotate_next("上游连接异常自动切换").await {
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

async fn send_upstream_https(
    req: Request<ProxyBody>,
    host: &str,
    port: u16,
    bypass_port: u16,
    tls_cfg: &Arc<ClientConfig>,
) -> Result<Response<Incoming>> {
    let mut tcp = TcpStream::connect(("127.0.0.1", bypass_port)).await?;

    // Tunnel through mihomo's loop-free bypass listener via HTTP CONNECT
    let connect_req = format!(
        "CONNECT {0}:{1} HTTP/1.1\r\nHost: {0}:{1}\r\n\r\n",
        host, port
    );
    tcp.write_all(connect_req.as_bytes()).await?;

    let mut resp_buf = Vec::with_capacity(512);
    let mut b = [0u8; 1];
    loop {
        let n = tcp.read(&mut b).await?;
        if n == 0 {
            return Err(anyhow!("上游代理在 CONNECT 握手期间断开"));
        }
        resp_buf.push(b[0]);
        if resp_buf.ends_with(b"\r\n\r\n") {
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

    let io = TokioIo::new(tls_stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let resp = sender.send_request(req).await?;
    Ok(resp)
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

    /// End-to-end integration test:
    /// 1. Starts a mock HTTPS Vertex server that returns 429 when active node == "Node-A" and 200 OK when == "Node-B".
    /// 2. Starts a mock mihomo HTTP CONNECT bypass forwarder pointing to the mock HTTPS server.
    /// 3. Starts our real selective HTTPS MITM proxy.
    /// 4. Client sends an HTTPS POST request through the MITM proxy and receives 200 OK transparently!
    #[tokio::test]
    async fn test_e2e_mitm_429_rotation_and_transparent_retry() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let temp_dir = std::env::temp_dir().join(format!("vertex_vpn_test_{}", rand::random::<u32>()));
        let ca = CertAuthority::load_or_create_in(&temp_dir).unwrap();

        let active_node_shared = Arc::new(Mutex::new("Node-A".to_string()));

        // 1. Mock HTTPS Vertex AI server (using a leaf cert signed by our test CA for "aiplatform.googleapis.com")
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
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, svc)
                            .await;
                    }
                });
            }
        });

        // 2. Mock mihomo bypass CONNECT forwarder -> redirects everything to our mock HTTPS server
        let bypass_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bypass_port = bypass_listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut client, _)) = bypass_listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut b = [0u8; 1];
                    while let Ok(1) = client.read(&mut b).await {
                        buf.push(b[0]);
                        if buf.ends_with(b"\r\n\r\n") {
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

        // 3. Real MitmProxy + NodeScheduler
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
        let ctx = MitmProxyContext {
            config: Arc::new(RwLock::new(cfg)),
            ca: ca.clone(),
            scheduler: scheduler.clone(),
            bus: bus.clone(),
            upstream_tls: upstream_tls.clone(),
        };

        let proxy_probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = proxy_probe.local_addr().unwrap().port();
        drop(proxy_probe);

        let (shut_tx, shut_rx) = watch::channel(false);
        tokio::spawn(async move {
            let _ = run_proxy_listener(format!("127.0.0.1:{}", proxy_port), false, ctx, shut_rx).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        // 4. Client connects via HTTP CONNECT to aiplatform.googleapis.com:443 through our MITM proxy
        let mut tcp = TcpStream::connect(("127.0.0.1", proxy_port)).await.unwrap();
        tcp.write_all(b"CONNECT aiplatform.googleapis.com:443 HTTP/1.1\r\nHost: aiplatform.googleapis.com:443\r\n\r\n")
            .await
            .unwrap();
        let mut connect_resp = Vec::new();
        let mut b = [0u8; 1];
        while let Ok(1) = tcp.read(&mut b).await {
            connect_resp.push(b[0]);
            if connect_resp.ends_with(b"\r\n\r\n") {
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

        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });

        let req = Request::builder()
            .method("POST")
            .uri("/v1/projects/demo/locations/us-central1/publishers/google/models/gemini:streamGenerateContent")
            .header("Host", "aiplatform.googleapis.com")
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
