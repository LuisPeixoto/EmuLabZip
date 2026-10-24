//! Runs real clients and servers through the proxy on the loopback interface.

use std::convert::Infallible;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use emulabzip_net::{
    Action, Breakpoints, CertificateAuthority, Edits, Flow, FlowKind, FlowState, FlowStore, Matcher, Proxy,
    ProxyConfig, ProxyHandle, Resume, Rule, decode::decode, to_har,
};
use http_body_util::{BodyExt, Empty, Full};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use rustls::RootCertStore;
use rustls::crypto::ring::default_provider;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

const JSON: &str = r#"{"hello":"world","items":[1,2,3]}"#;

// ---------------------------------------------------------------------------------------------
// Servers
// ---------------------------------------------------------------------------------------------

struct Upstream {
    addr: SocketAddr,
    ca_pem: String,
}

async fn upstream_handler(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let version = format!("{:?}", req.version());
    let res = match req.uri().path() {
        "/hello" => Response::new(Full::new(Bytes::from("hello"))),
        "/gzip" => {
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            gz.write_all(JSON.as_bytes()).unwrap();
            Response::builder()
                .header("content-type", "application/json; charset=utf-8")
                .header("content-encoding", "gzip")
                .body(Full::new(Bytes::from(gz.finish().unwrap())))
                .unwrap()
        }
        "/echo" => {
            let body = req.into_body().collect().await.unwrap().to_bytes();
            Response::builder().header("x-seen-version", version).body(Full::new(body)).unwrap()
        }
        "/ws" => {
            let on = hyper::upgrade::on(req);
            tokio::spawn(async move {
                let mut io = TokioIo::new(on.await.unwrap());
                let mut buf = [0u8; 64];
                loop {
                    match io.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let _ = io.write_all(&buf[..n]).await;
                        }
                    }
                }
            });
            Response::builder()
                .status(StatusCode::SWITCHING_PROTOCOLS)
                .header("upgrade", "websocket")
                .header("connection", "upgrade")
                .body(Full::default())
                .unwrap()
        }
        _ => Response::builder().status(StatusCode::NOT_FOUND).body(Full::default()).unwrap(),
    };
    Ok(res)
}

/// A server that speaks HTTP/1.1 and HTTP/2, over TLS when `tls` is set (certificate for
/// `localhost`, signed by a CA of its own that the test hands to the proxy).
async fn start_upstream(tls: bool) -> Upstream {
    let ca = Arc::new(CertificateAuthority::generate().unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = tls.then(|| TlsAcceptor::from(ca.server_config("localhost").unwrap()));
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let builder = auto::Builder::new(TokioExecutor::new());
                match acceptor {
                    Some(acceptor) => {
                        let Ok(tls) = acceptor.accept(stream).await else { return };
                        let _ = builder
                            .serve_connection_with_upgrades(TokioIo::new(tls), service_fn(upstream_handler))
                            .await;
                    }
                    None => {
                        let _ = builder
                            .serve_connection_with_upgrades(TokioIo::new(stream), service_fn(upstream_handler))
                            .await;
                    }
                }
            });
        }
    });
    Upstream { addr, ca_pem: ca.cert_pem().to_string() }
}

async fn start_echo() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 64];
                while let Ok(n) = stream.read(&mut buf).await {
                    if n == 0 || stream.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    addr
}

struct Rig {
    proxy: ProxyHandle,
    store: Arc<FlowStore>,
    ca: Arc<CertificateAuthority>,
}

async fn start_proxy(extra_roots: Vec<String>) -> Rig {
    let ca = Arc::new(CertificateAuthority::generate().unwrap());
    let store = FlowStore::new(1000);
    let config =
        ProxyConfig { addr: "127.0.0.1:0".parse().unwrap(), extra_roots_pem: extra_roots, ..Default::default() };
    let proxy = Proxy::start(config, ca.clone(), store.clone()).await.unwrap();
    Rig { proxy, store, ca }
}

// ---------------------------------------------------------------------------------------------
// Clients
// ---------------------------------------------------------------------------------------------

async fn connect_tunnel(proxy: SocketAddr, authority: &str) -> TcpStream {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    stream.write_all(format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes()).await.unwrap();
    let (mut head, mut byte) = (Vec::new(), [0u8; 1]);
    while !head.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).await.unwrap();
        head.push(byte[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 200"), "CONNECT answered {}", String::from_utf8_lossy(&head));
    stream
}

fn tls_connector(trusted_pem: &str, alpn: &[u8]) -> TlsConnector {
    let mut roots = RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut trusted_pem.as_bytes()) {
        roots.add(cert.unwrap()).unwrap();
    }
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![alpn.to_vec()];
    TlsConnector::from(Arc::new(config))
}

async fn tls(
    stream: TcpStream,
    trusted_pem: &str,
    name: &str,
    alpn: &[u8],
) -> std::io::Result<tokio_rustls::client::TlsStream<TcpStream>> {
    tls_connector(trusted_pem, alpn).connect(ServerName::try_from(name.to_string()).unwrap(), stream).await
}

type Reply = (StatusCode, hyper::HeaderMap, Bytes);

async fn collect(res: Response<hyper::body::Incoming>) -> Reply {
    let (parts, body) = res.into_parts();
    (parts.status, parts.headers, body.collect().await.unwrap().to_bytes())
}

async fn send_h1<T: AsyncRead + AsyncWrite + Unpin + Send + 'static>(io: T, req: Request<Full<Bytes>>) -> Reply {
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(io)).await.unwrap();
    tokio::spawn(conn);
    collect(sender.send_request(req).await.unwrap()).await
}

async fn send_h2<T: AsyncRead + AsyncWrite + Unpin + Send + 'static>(io: T, req: Request<Full<Bytes>>) -> Reply {
    let (mut sender, conn) =
        hyper::client::conn::http2::handshake(TokioExecutor::new(), TokioIo::new(io)).await.unwrap();
    tokio::spawn(conn);
    collect(sender.send_request(req).await.unwrap()).await
}

fn get(uri: &str) -> Request<Full<Bytes>> {
    Request::get(uri).body(Full::default()).unwrap()
}

/// The proxy records a flow as soon as the response ends, which can be a moment after the client
/// has read it.
async fn flow_where(store: &FlowStore, what: &str, ok: impl Fn(&Flow) -> bool) -> Flow {
    for _ in 0..100 {
        if let Some(f) = store.all().into_iter().find(&ok) {
            return f;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no flow matched: {what}; flows: {:#?}", store.summaries());
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn plain_http_is_forwarded_and_recorded() {
    let upstream = start_upstream(false).await;
    let rig = start_proxy(vec![]).await;

    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (status, _, body) = send_h1(client, get(&format!("http://{}/hello", upstream.addr))).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"hello"[..]));

    let flow = flow_where(&rig.store, "a completed GET", |f| f.state == FlowState::Completed).await;
    assert_eq!(flow.kind, FlowKind::Http);
    assert_eq!((flow.method.as_str(), flow.scheme.as_str(), flow.path.as_str()), ("GET", "http", "/hello"));
    assert_eq!(flow.status, Some(200));
    assert_eq!(flow.response_body, b"hello");
    assert!(flow.timing.ttfb_ms.is_some() && flow.timing.total_ms.is_some());

    let har = to_har(&rig.store.all());
    assert_eq!(har["log"]["entries"].as_array().unwrap().len(), 1);
    assert_eq!(har["log"]["entries"][0]["response"]["content"]["text"], "hello");
}

#[tokio::test]
async fn https_is_decrypted_over_http1_and_http2() {
    let upstream = start_upstream(true).await;
    let rig = start_proxy(vec![upstream.ca_pem.clone()]).await;
    let authority = format!("localhost:{}", upstream.addr.port());

    // HTTP/1.1 client, with a request body.
    let tunnel = connect_tunnel(rig.proxy.addr(), &authority).await;
    let io = tls(tunnel, rig.ca.cert_pem(), "localhost", b"http/1.1").await.unwrap();
    let req = Request::post("/echo").header("host", &authority).body(Full::new(Bytes::from("ping"))).unwrap();
    let (status, headers, body) = send_h1(io, req).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"ping"[..]));
    // The proxy talks to the server over HTTP/2 whatever the client speaks.
    assert_eq!(headers["x-seen-version"], "HTTP/2.0");

    let post = flow_where(&rig.store, "the POST", |f| f.method == "POST" && f.state == FlowState::Completed).await;
    assert_eq!((post.scheme.as_str(), post.host.as_str(), post.path.as_str()), ("https", "localhost", "/echo"));
    assert_eq!(post.http_version, "HTTP/1.1");
    assert_eq!(post.request_body, b"ping");
    assert_eq!(post.response_body, b"ping");

    // HTTP/2 client.
    let tunnel = connect_tunnel(rig.proxy.addr(), &authority).await;
    let io = tls(tunnel, rig.ca.cert_pem(), "localhost", b"h2").await.unwrap();
    let (status, _, body) = send_h2(io, get(&format!("https://{authority}/hello"))).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"hello"[..]));

    let h2 = flow_where(&rig.store, "the h2 GET", |f| f.method == "GET" && f.state == FlowState::Completed).await;
    assert_eq!(h2.http_version, "HTTP/2.0");
    assert_eq!(h2.url, format!("https://localhost:{}/hello", upstream.addr.port()));
}

#[tokio::test]
async fn bodies_are_forwarded_as_sent_and_can_be_decoded() {
    let upstream = start_upstream(true).await;
    let rig = start_proxy(vec![upstream.ca_pem.clone()]).await;
    let authority = format!("localhost:{}", upstream.addr.port());

    let tunnel = connect_tunnel(rig.proxy.addr(), &authority).await;
    let io = tls(tunnel, rig.ca.cert_pem(), "localhost", b"http/1.1").await.unwrap();
    let (_, headers, body) =
        send_h1(io, Request::get("/gzip").header("host", &authority).body(Full::default()).unwrap()).await;
    assert_eq!(headers["content-encoding"], "gzip");
    assert_eq!(&body[..2], &[0x1f, 0x8b], "the client must receive the compressed bytes");

    let flow =
        flow_where(&rig.store, "the gzip response", |f| f.path == "/gzip" && f.state == FlowState::Completed).await;
    assert_eq!(flow.response_body, body, "the proxy stores what was on the wire");
    assert_eq!(flow.content_type().as_deref(), Some("application/json"));
    let encoding = Flow::header(&flow.response_headers, "content-encoding");
    assert_eq!(decode(encoding, &flow.response_body).unwrap(), JSON.as_bytes());

    let har = to_har(&rig.store.all());
    assert_eq!(har["log"]["entries"][0]["response"]["content"]["text"], JSON);
}

#[tokio::test]
async fn other_protocols_are_tunnelled() {
    let echo = start_echo().await;
    let rig = start_proxy(vec![]).await;

    let mut tunnel = connect_tunnel(rig.proxy.addr(), &echo.to_string()).await;
    tunnel.write_all(b"ping\n").await.unwrap();
    let mut reply = [0u8; 5];
    tunnel.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply, b"ping\n");
    drop(tunnel);

    let flow = flow_where(&rig.store, "the finished tunnel", |f| {
        f.kind == FlowKind::Tunnel && f.state == FlowState::Completed
    })
    .await;
    assert_eq!((flow.request_size, flow.response_size), (5, 5));
    assert!(to_har(&rig.store.all())["log"]["entries"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn websockets_are_upgraded_and_recorded() {
    let upstream = start_upstream(false).await;
    let rig = start_proxy(vec![]).await;

    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(client)).await.unwrap();
    tokio::spawn(conn.with_upgrades());
    let req = Request::get(format!("http://{}/ws", upstream.addr))
        .header("upgrade", "websocket")
        .header("connection", "Upgrade")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let res = sender.send_request(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SWITCHING_PROTOCOLS);

    let mut io = TokioIo::new(hyper::upgrade::on(res).await.unwrap());
    io.write_all(b"hello").await.unwrap();
    let mut echoed = [0u8; 5];
    io.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"hello");
    drop(io);

    let flow = flow_where(&rig.store, "the finished WebSocket", |f| {
        f.kind == FlowKind::WebSocket && f.state == FlowState::Completed
    })
    .await;
    assert_eq!((flow.status, flow.scheme.as_str()), (Some(101), "ws"));
    assert_eq!((flow.request_size, flow.response_size), (5, 5));
}

#[tokio::test]
async fn an_app_that_rejects_the_certificate_is_passed_through_next_time() {
    let upstream = start_upstream(true).await;
    let rig = start_proxy(vec![upstream.ca_pem.clone()]).await;
    let authority = format!("localhost:{}", upstream.addr.port());

    // The app trusts no one but the server's own CA, as an app that pins its certificate would.
    let tunnel = connect_tunnel(rig.proxy.addr(), &authority).await;
    assert!(tls(tunnel, &upstream.ca_pem, "localhost", b"http/1.1").await.is_err());
    let failed = flow_where(&rig.store, "the rejected handshake", |f| f.state == FlowState::Failed).await;
    assert_eq!(failed.kind, FlowKind::Tunnel);
    assert!(failed.error.unwrap().contains("did not accept the EmuLabZip certificate"));
    assert_eq!(rig.proxy.bypassed(), ["localhost"]);

    // The retry goes straight to the server, and the app's own trust is enough.
    let tunnel = connect_tunnel(rig.proxy.addr(), &authority).await;
    let io = tls(tunnel, &upstream.ca_pem, "localhost", b"http/1.1").await.unwrap();
    let (status, _, body) =
        send_h1(io, Request::get("/hello").header("host", &authority).body(Full::default()).unwrap()).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"hello"[..]));

    // Once the certificate is installed, the host can be decrypted again.
    rig.proxy.clear_bypass();
    assert!(rig.proxy.bypassed().is_empty());
}

#[tokio::test]
async fn serves_the_certificate_to_browsers() {
    let rig = start_proxy(vec![]).await;
    for (path, expected) in
        [("/ca.crt", rig.ca.cert_der().to_vec()), ("/ca.pem", rig.ca.cert_pem().as_bytes().to_vec())]
    {
        let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
        let (status, _, body) =
            send_h1(client, Request::get(path).header("host", "emulabzip.local").body(Full::default()).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.to_vec(), expected, "{path}");
    }
    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (_, headers, page) =
        send_h1(client, Request::get("/").header("host", "x").body(Full::default()).unwrap()).await;
    assert!(headers["content-type"].to_str().unwrap().starts_with("text/html"));
    assert!(String::from_utf8_lossy(&page).contains("/ca.crt"));
    assert!(rig.store.is_empty(), "requests to the proxy itself are not traffic");
}

#[tokio::test]
async fn unreachable_servers_become_failed_flows() {
    let rig = start_proxy(vec![]).await;
    // Nothing listens on port 1.
    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (status, _, body) = send_h1(client, get("http://127.0.0.1:1/")).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(String::from_utf8_lossy(&body).contains("could not complete"));
    let flow = flow_where(&rig.store, "the failed request", |f| f.state == FlowState::Failed).await;
    assert!(flow.error.is_some());
    assert_eq!(flow.status, None);
}

#[tokio::test]
async fn stopping_the_proxy_frees_the_port() {
    let rig = start_proxy(vec![]).await;
    let addr = rig.proxy.addr();
    rig.proxy.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(TcpStream::connect(addr).await.is_err());
}

// ---------------------------------------------------------------------------------------------
// Rules and breakpoints
// ---------------------------------------------------------------------------------------------

fn rule(name: &str, path: &str, action: Action) -> Rule {
    Rule {
        id: 0,
        name: name.into(),
        enabled: true,
        matcher: Matcher { path: Some(path.into()), ..Default::default() },
        action,
    }
}

async fn start_proxy_with_rules(rules: Vec<Rule>) -> Rig {
    let rig = start_proxy(vec![]).await;
    rig.proxy.rules().set(rules).unwrap();
    rig
}

async fn held(breakpoints: &Breakpoints) -> emulabzip_net::Paused {
    for _ in 0..200 {
        if let Some(p) = breakpoints.pending().into_iter().next() {
            return p;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("nothing was held at a breakpoint");
}

#[tokio::test]
async fn a_mock_rule_answers_without_asking_the_server() {
    let upstream = start_upstream(false).await;
    let rig = start_proxy_with_rules(vec![rule(
        "teapot",
        "/hello",
        Action::Mock {
            status: 418,
            headers: vec![("content-type".into(), "text/plain".into()), ("x-mock".into(), "yes".into())],
            body: "short and stout".into(),
            base64: false,
            delay_ms: 0,
        },
    )])
    .await;

    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (status, headers, body) = send_h1(client, get(&format!("http://{}/hello", upstream.addr))).await;
    assert_eq!((status.as_u16(), &body[..]), (418, &b"short and stout"[..]));
    assert_eq!(headers["x-mock"], "yes");

    let flow = flow_where(&rig.store, "the mocked flow", |f| f.state == FlowState::Completed).await;
    assert!(flow.mocked);
    assert_eq!((flow.rule.as_deref(), flow.status), (Some("teapot"), Some(418)));
    assert_eq!(flow.response_body, b"short and stout");
    // A request no rule matches still goes to the server.
    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (status, _, _) = send_h1(client, get(&format!("http://{}/nothing", upstream.addr))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_mock_can_have_a_binary_body_and_a_delay() {
    let upstream = start_upstream(false).await;
    let rig = start_proxy_with_rules(vec![rule(
        "bytes",
        "/hello",
        Action::Mock { status: 200, headers: vec![], body: "//4A".into(), base64: true, delay_ms: 300 },
    )])
    .await;
    let started = std::time::Instant::now();
    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (_, _, body) = send_h1(client, get(&format!("http://{}/hello", upstream.addr))).await;
    assert_eq!(&body[..], &[0xff, 0xfe, 0x00]);
    assert!(started.elapsed() >= Duration::from_millis(300));
}

#[tokio::test]
async fn a_delay_rule_slows_the_request_down_and_a_block_rule_fails_it() {
    let upstream = start_upstream(false).await;
    let rig = start_proxy_with_rules(vec![
        rule("slow", "/hello", Action::Delay { ms: 400 }),
        rule("down", "/blocked", Action::Block),
    ])
    .await;

    let started = std::time::Instant::now();
    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (status, _, body) = send_h1(client, get(&format!("http://{}/hello", upstream.addr))).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"hello"[..]));
    assert!(started.elapsed() >= Duration::from_millis(400), "{:?}", started.elapsed());

    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (status, _, body) = send_h1(client, get(&format!("http://{}/blocked", upstream.addr))).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(String::from_utf8_lossy(&body).contains("Blocked by the rule “down”"));
    let flow = flow_where(&rig.store, "the blocked flow", |f| f.state == FlowState::Failed).await;
    assert_eq!(flow.rule.as_deref(), Some("down"));
}

#[tokio::test]
async fn a_request_breakpoint_holds_the_request_and_sends_the_edit() {
    let upstream = start_upstream(false).await;
    let rig =
        start_proxy_with_rules(vec![rule("hold", "/echo", Action::Breakpoint { request: true, response: false })])
            .await;
    let breakpoints = rig.proxy.breakpoints();

    let addr = rig.proxy.addr();
    let target = format!("http://{}/echo", upstream.addr);
    let client = tokio::spawn(async move {
        let io = TcpStream::connect(addr).await.unwrap();
        send_h1(io, Request::post(target).body(Full::new(Bytes::from("ping"))).unwrap()).await
    });

    let paused = held(&breakpoints).await;
    assert_eq!((paused.method.as_str(), paused.body_text.as_deref()), ("POST", Some("ping")));
    let flow = flow_where(&rig.store, "the held flow", |f| f.state == FlowState::Paused).await;
    assert_eq!(flow.id, paused.flow);
    assert!(!client.is_finished(), "the client is still waiting");

    breakpoints.resume(paused.id, Resume::Continue(Edits { body: Some(b"pong".to_vec()), ..Default::default() }));
    let (status, _, body) = client.await.unwrap();
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"pong"[..]), "the server saw the edited body");
    let flow = flow_where(&rig.store, "the finished flow", |f| f.state == FlowState::Completed).await;
    assert_eq!(flow.request_body, b"pong");
}

#[tokio::test]
async fn aborting_at_a_breakpoint_fails_the_request() {
    let upstream = start_upstream(false).await;
    let rig =
        start_proxy_with_rules(vec![rule("hold", "/hello", Action::Breakpoint { request: true, response: false })])
            .await;
    let breakpoints = rig.proxy.breakpoints();
    let addr = rig.proxy.addr();
    let target = format!("http://{}/hello", upstream.addr);
    let client = tokio::spawn(async move { send_h1(TcpStream::connect(addr).await.unwrap(), get(&target)).await });
    let paused = held(&breakpoints).await;
    breakpoints.resume(paused.id, Resume::Abort);
    let (status, _, _) = client.await.unwrap();
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    flow_where(&rig.store, "the failed flow", |f| f.state == FlowState::Failed).await;
}

#[tokio::test]
async fn a_response_breakpoint_shows_the_decoded_body_and_sends_the_edit() {
    let upstream = start_upstream(false).await;
    let rig =
        start_proxy_with_rules(vec![rule("hold", "/gzip", Action::Breakpoint { request: false, response: true })])
            .await;
    let breakpoints = rig.proxy.breakpoints();
    let addr = rig.proxy.addr();
    let target = format!("http://{}/gzip", upstream.addr);
    let client = tokio::spawn(async move { send_h1(TcpStream::connect(addr).await.unwrap(), get(&target)).await });

    let paused = held(&breakpoints).await;
    assert_eq!(paused.status, Some(200));
    assert_eq!(paused.body_text.as_deref(), Some(JSON), "a compressed response is shown decoded");
    assert!(!paused.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-encoding")));

    let edits = Edits { body: Some(b"{\"hello\":\"edited\"}".to_vec()), status: Some(201), ..Default::default() };
    breakpoints.resume(paused.id, Resume::Continue(edits));
    let (status, headers, body) = client.await.unwrap();
    assert_eq!(status.as_u16(), 201);
    assert_eq!(&body[..], b"{\"hello\":\"edited\"}");
    assert!(headers.get("content-encoding").is_none(), "what continues is not compressed");
    let flow = flow_where(&rig.store, "the finished flow", |f| f.state == FlowState::Completed).await;
    assert_eq!((flow.status, flow.response_body.as_slice()), (Some(201), &b"{\"hello\":\"edited\"}"[..]));
}

#[tokio::test]
async fn a_breakpoint_nobody_answers_lets_the_request_through() {
    let upstream = start_upstream(false).await;
    let ca = Arc::new(CertificateAuthority::generate().unwrap());
    let store = FlowStore::new(100);
    let breakpoints = Arc::new(Breakpoints::with_timeout(Duration::from_millis(300)));
    let config = ProxyConfig { addr: "127.0.0.1:0".parse().unwrap(), breakpoints, ..Default::default() };
    let proxy = Proxy::start(config, ca, store).await.unwrap();
    proxy
        .rules()
        .set(vec![rule("forgotten", "/hello", Action::Breakpoint { request: true, response: false })])
        .unwrap();
    let client = TcpStream::connect(proxy.addr()).await.unwrap();
    let (status, _, body) = send_h1(client, get(&format!("http://{}/hello", upstream.addr))).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"hello"[..]));
}

// ---------------------------------------------------------------------------------------------
// WebSocket messages
// ---------------------------------------------------------------------------------------------

/// A masked text frame, as a browser sends it.
fn client_text_frame(text: &str) -> Vec<u8> {
    let key = [0x37, 0xfa, 0x21, 0x3d];
    let mut frame = vec![0x81, 0x80 | text.len() as u8];
    frame.extend_from_slice(&key);
    frame.extend(text.bytes().enumerate().map(|(i, b)| b ^ key[i % 4]));
    frame
}

#[tokio::test]
async fn websocket_messages_are_read_in_both_directions_and_exported() {
    use emulabzip_net::ws::{Direction, Kind};

    let upstream = start_upstream(false).await;
    let rig = start_proxy(vec![]).await;
    let client = TcpStream::connect(rig.proxy.addr()).await.unwrap();
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(client)).await.unwrap();
    tokio::spawn(conn.with_upgrades());
    let req = Request::get(format!("http://{}/ws", upstream.addr))
        .header("upgrade", "websocket")
        .header("connection", "Upgrade")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let res = sender.send_request(req).await.unwrap();
    let mut io = TokioIo::new(hyper::upgrade::on(res).await.unwrap());

    for text in ["hello socket", "second message"] {
        let frame = client_text_frame(text);
        io.write_all(&frame).await.unwrap();
        let mut echoed = vec![0u8; frame.len()];
        io.read_exact(&mut echoed).await.unwrap();
        assert_eq!(echoed, frame, "the echo server's bytes reach the client untouched");
    }
    drop(io);

    let flow = flow_where(&rig.store, "the finished WebSocket", |f| {
        f.kind == FlowKind::WebSocket && f.state == FlowState::Completed
    })
    .await;
    let seen: Vec<_> =
        flow.messages.iter().map(|m| (m.direction, m.kind, m.text.clone().unwrap_or_default())).collect();
    assert_eq!(
        seen,
        [
            (Direction::Up, Kind::Text, "hello socket".to_string()),
            (Direction::Down, Kind::Text, "hello socket".to_string()),
            (Direction::Up, Kind::Text, "second message".to_string()),
            (Direction::Down, Kind::Text, "second message".to_string()),
        ]
    );
    assert_eq!(flow.summary().message_count, 4);

    let har = to_har(&rig.store.all());
    let entry = &har["log"]["entries"][0];
    assert_eq!(entry["_resourceType"], "websocket");
    let messages = entry["_webSocketMessages"].as_array().unwrap();
    assert_eq!(
        (messages[0]["type"].as_str(), messages[0]["opcode"].as_i64(), messages[1]["type"].as_str()),
        (Some("send"), Some(1), Some("receive"))
    );
    assert_eq!(messages[2]["data"], "second message");
}
