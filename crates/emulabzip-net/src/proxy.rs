//! The proxy server.

use std::collections::HashSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::Bytes;
use emulabzip_core::{Error, Result};
use http::{HeaderMap, Method, Request, Response, StatusCode, Uri, header};
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::{Body, Frame, Incoming};
use hyper::service::service_fn;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use parking_lot::Mutex;
use rustls::crypto::ring::default_provider;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::breakpoints::{Breakpoints, Edits, Hold, Phase, Resume};
use crate::ca::CertificateAuthority;
use crate::flow::{BODY_LIMIT, Flow, FlowId, FlowKind, FlowState, Headers};
use crate::http3::{self, Http3};
use crate::io::{BoxIo, Prefixed};
use crate::rules::{Action, Rule, RuleSet, mock_bytes};
use crate::store::FlowStore;
use crate::ws::{Deflate, Direction, FrameParser, MAX_MESSAGES, parse_extensions};

type BoxErr = Box<dyn std::error::Error + Send + Sync>;
type RespBody = UnsyncBoxBody<Bytes, BoxErr>;
type Resp = Response<RespBody>;
type UpstreamClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;
type Tunnel = TokioIo<hyper::upgrade::Upgraded>;

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub addr: SocketAddr,
    /// Most bytes stored per request or response body. The rest is forwarded but not kept.
    pub body_limit: usize,
    /// Extra certificate authorities (PEM) trusted when talking to servers, for example a
    /// company root, on top of the system's and the built-in list.
    pub extra_roots_pem: Vec<String>,
    /// Rules that mock, delay, block or hold requests. Shared, so they can change while it runs.
    pub rules: Arc<RuleSet>,
    /// Where held requests and responses wait. Shared with whoever shows them.
    pub breakpoints: Arc<Breakpoints>,
    /// Send requests to servers that offer HTTP/3 over QUIC. They are found through `Alt-Svc`.
    pub http3: bool,
    /// Remove HTTP/3 from the `Alt-Svc` the device is shown, so apps do not try QUIC around the proxy.
    pub hide_http3: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            addr: SocketAddr::from(([127, 0, 0, 1], 9090)),
            body_limit: BODY_LIMIT,
            extra_roots_pem: Vec::new(),
            rules: Arc::new(RuleSet::new()),
            breakpoints: Arc::new(Breakpoints::new()),
            http3: false,
            hide_http3: true,
        }
    }
}

struct Ctx {
    ca: Arc<CertificateAuthority>,
    store: Arc<FlowStore>,
    client: UpstreamClient,
    /// For WebSocket upstream connections: HTTP/1.1 only.
    upgrade_tls: TlsConnector,
    body_limit: usize,
    /// Hosts that rejected our certificate. They are passed through from then on, so an app that
    /// pins its certificates keeps working instead of failing on every request.
    bypass: Mutex<HashSet<String>>,
    rules: Arc<RuleSet>,
    breakpoints: Arc<Breakpoints>,
    http3: Http3,
    shutdown: watch::Receiver<bool>,
}

/// A running proxy. Dropping it stops the proxy and closes its connections.
pub struct ProxyHandle {
    addr: SocketAddr,
    ctx: Arc<Ctx>,
    shutdown: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl ProxyHandle {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Hosts that are passed through because they rejected our certificate.
    pub fn bypassed(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self.ctx.bypass.lock().iter().cloned().collect();
        hosts.sort();
        hosts
    }

    /// Tries to decrypt the passed-through hosts again, for example after installing the certificate.
    pub fn clear_bypass(&self) {
        self.ctx.bypass.lock().clear();
    }

    /// The rules the proxy applies.
    pub fn rules(&self) -> Arc<RuleSet> {
        self.ctx.rules.clone()
    }

    /// Whether requests go to servers over HTTP/3 when they offer it.
    pub fn http3(&self) -> bool {
        self.ctx.http3.enabled()
    }

    pub fn set_http3(&self, on: bool) {
        self.ctx.http3.set_enabled(on);
    }

    /// Whether HTTP/3 is removed from the `Alt-Svc` the device sees.
    pub fn hides_http3(&self) -> bool {
        self.ctx.http3.hides()
    }

    pub fn set_hide_http3(&self, on: bool) {
        self.ctx.http3.set_hide(on);
    }

    /// Where requests and responses held at breakpoints wait.
    pub fn breakpoints(&self) -> Arc<Breakpoints> {
        self.ctx.breakpoints.clone()
    }

    pub fn stop(self) {}
}

impl Drop for ProxyHandle {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        self.task.abort();
    }
}

pub struct Proxy;

impl Proxy {
    /// Binds the address and starts serving. Use port 0 to let the system pick one.
    pub async fn start(
        config: ProxyConfig,
        ca: Arc<CertificateAuthority>,
        store: Arc<FlowStore>,
    ) -> Result<ProxyHandle> {
        let tls = client_tls_config(&config.extra_roots_pem)?;
        let https =
            HttpsConnectorBuilder::new().with_tls_config(tls.clone()).https_or_http().enable_all_versions().build();
        let client = Client::builder(TokioExecutor::new()).build::<_, Full<Bytes>>(https);
        let http3 = Http3::new(&tls, config.http3, config.hide_http3);
        let mut http1_only = tls;
        http1_only.alpn_protocols = vec![b"http/1.1".to_vec()];

        let listener = TcpListener::bind(config.addr).await?;
        let addr = listener.local_addr()?;
        let (shutdown, rx) = watch::channel(false);
        let ctx = Arc::new(Ctx {
            ca,
            store,
            client,
            upgrade_tls: TlsConnector::from(Arc::new(http1_only)),
            body_limit: config.body_limit,
            bypass: Mutex::new(HashSet::new()),
            rules: config.rules,
            breakpoints: config.breakpoints,
            http3,
            shutdown: rx,
        });
        tracing::info!(%addr, "proxy listening");
        let task = tokio::spawn(accept_loop(listener, ctx.clone()));
        Ok(ProxyHandle { addr, ctx, shutdown, task })
    }
}

fn client_tls_config(extra_pem: &[String]) -> Result<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    for pem in extra_pem {
        for cert in rustls_pemfile::certs(&mut pem.as_bytes()) {
            let cert = cert.map_err(|e| Error::parse("trusted certificate", e.to_string()))?;
            roots.add(cert).map_err(|e| Error::parse("trusted certificate", e.to_string()))?;
        }
    }
    Ok(rustls::ClientConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::parse("TLS versions", e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth())
}

/// Waits until the proxy is stopped (or its handle is gone).
async fn cancelled(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow() || rx.changed().await.is_err() {
            return;
        }
    }
}

async fn accept_loop(listener: TcpListener, ctx: Arc<Ctx>) {
    let mut shutdown = ctx.shutdown.clone();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => { tokio::spawn(serve_client(stream, ctx.clone())); }
                Err(e) => tracing::warn!("accept failed: {e}"),
            },
            _ = cancelled(&mut shutdown) => break,
        }
    }
}

async fn serve_client(stream: TcpStream, ctx: Arc<Ctx>) {
    let _ = stream.set_nodelay(true);
    let mut shutdown = ctx.shutdown.clone();
    let service = service_fn(move |req| {
        let ctx = ctx.clone();
        async move { Ok::<_, Infallible>(handle(req, ctx).await) }
    });
    let conn =
        hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), service).with_upgrades();
    tokio::select! {
        _ = conn => {}
        _ = cancelled(&mut shutdown) => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Requests addressed to the proxy
// ---------------------------------------------------------------------------------------------

fn full(body: impl Into<Bytes>) -> RespBody {
    Full::new(body.into()).map_err(|never: Infallible| -> BoxErr { match never {} }).boxed_unsync()
}

fn empty() -> RespBody {
    Empty::new().map_err(|never: Infallible| -> BoxErr { match never {} }).boxed_unsync()
}

fn text(status: StatusCode, message: &str) -> Resp {
    let mut res = Response::new(full(message.to_string()));
    *res.status_mut() = status;
    res.headers_mut().insert(header::CONTENT_TYPE, "text/plain; charset=utf-8".parse().expect("static"));
    res
}

async fn handle(req: Request<Incoming>, ctx: Arc<Ctx>) -> Resp {
    if req.method() == Method::CONNECT {
        return connect(req, ctx);
    }
    let uri = req.uri();
    let Some(authority) = uri.authority().cloned() else { return local_page(&req, &ctx) };
    let (scheme, default_port) = if uri.scheme_str() == Some("https") { ("https", 443) } else { ("http", 80) };
    forward(req, ctx, scheme, authority.host(), authority.port_u16().unwrap_or(default_port)).await
}

/// What the proxy answers when it is opened directly in a browser: a page with the certificate.
fn local_page(req: &Request<Incoming>, ctx: &Ctx) -> Resp {
    let attachment = |name: &str, mime: &str, body: Vec<u8>| {
        let mut res = Response::new(full(body));
        res.headers_mut().insert(header::CONTENT_TYPE, mime.parse().expect("static"));
        res.headers_mut()
            .insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"").parse().expect("static"));
        res
    };
    match req.uri().path() {
        "/ca.pem" => attachment("emulabzip-ca.pem", "application/x-pem-file", ctx.ca.cert_pem().as_bytes().to_vec()),
        "/ca.crt" | "/ca.cer" => {
            attachment("emulabzip-ca.crt", "application/x-x509-ca-cert", ctx.ca.cert_der().to_vec())
        }
        _ => {
            let mut res = Response::new(full(
                "<!doctype html><meta charset=utf-8><title>EmuLabZip</title>\
                 <body style=\"font:16px system-ui;max-width:32rem;margin:3rem auto;padding:0 1rem\">\
                 <h1>EmuLabZip proxy</h1><p>The proxy is running.</p>\
                 <p>Install its certificate to inspect HTTPS traffic: \
                 <a href=\"/ca.crt\">ca.crt</a> (Android, iOS) or <a href=\"/ca.pem\">ca.pem</a>.</p>",
            ));
            res.headers_mut().insert(header::CONTENT_TYPE, "text/html; charset=utf-8".parse().expect("static"));
            res
        }
    }
}

// ---------------------------------------------------------------------------------------------
// CONNECT tunnels
// ---------------------------------------------------------------------------------------------

fn connect(req: Request<Incoming>, ctx: Arc<Ctx>) -> Resp {
    let Some(authority) = req.uri().authority().cloned() else {
        return text(StatusCode::BAD_REQUEST, "CONNECT needs a host and a port");
    };
    let (host, port) = (authority.host().to_string(), authority.port_u16().unwrap_or(443));
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => tunnel(TokioIo::new(upgraded), host, port, ctx).await,
            Err(e) => tracing::debug!("CONNECT upgrade failed: {e}"),
        }
    });
    Response::new(empty())
}

async fn tunnel(mut client: Tunnel, host: String, port: u16, ctx: Arc<Ctx>) {
    let key = host.to_ascii_lowercase();
    if ctx.bypass.lock().contains(&key) {
        return raw_tunnel(client, host, port, ctx).await;
    }
    // A TLS handshake starts with the record type 0x16. Anything else is not for us to decode.
    let mut first = [0u8; 1];
    if client.read_exact(&mut first).await.is_err() {
        return;
    }
    let client = Prefixed::new(first.to_vec(), client);
    if first[0] != 0x16 {
        return raw_tunnel(client, host, port, ctx).await;
    }
    let config = match ctx.ca.server_config(&host) {
        Ok(c) => c,
        Err(e) => return tracing::warn!("could not prepare TLS for {host}: {e}"),
    };
    match TlsAcceptor::from(config).accept(client).await {
        Ok(tls) => serve_decrypted(tls, host, port, ctx).await,
        Err(e) => {
            // The client closed the handshake: it does not trust our certificate, or it pins the
            // server's. Remember the host so the next attempt goes straight through.
            ctx.bypass.lock().insert(key);
            let mut flow = Flow::new(FlowKind::Tunnel, "CONNECT", "tcp", &host, port, "");
            flow.state = FlowState::Failed;
            flow.error = Some(format!(
                "The app did not accept the EmuLabZip certificate ({e}). Either the certificate is not installed \
                 on the device, or the app pins its server's certificate. Later connections to this host are \
                 passed through without being decoded."
            ));
            ctx.store.begin(flow);
        }
    }
}

async fn serve_decrypted(
    tls: tokio_rustls::server::TlsStream<Prefixed<Tunnel>>,
    host: String,
    port: u16,
    ctx: Arc<Ctx>,
) {
    let mut shutdown = ctx.shutdown.clone();
    let service = service_fn(move |req| {
        let (ctx, host) = (ctx.clone(), host.clone());
        async move { Ok::<_, Infallible>(forward(req, ctx, "https", &host, port).await) }
    });
    let builder = auto::Builder::new(TokioExecutor::new());
    let conn = builder.serve_connection_with_upgrades(TokioIo::new(tls), service);
    tokio::select! {
        _ = conn => {}
        _ = cancelled(&mut shutdown) => {}
    }
}

/// Copies bytes both ways without looking at them.
async fn raw_tunnel<C: AsyncRead + AsyncWrite + Unpin>(mut client: C, host: String, port: u16, ctx: Arc<Ctx>) {
    let mut flow = Flow::new(FlowKind::Tunnel, "CONNECT", "tcp", &host, port, "");
    flow.http_version = String::new();
    let id = ctx.store.begin(flow);
    let started = Instant::now();
    let mut shutdown = ctx.shutdown.clone();

    let upstream = TcpStream::connect((host.trim_matches(['[', ']']), port)).await;
    let outcome = match upstream {
        Ok(mut upstream) => {
            tokio::select! {
                r = copy_bidirectional(&mut client, &mut upstream) => r.map_err(|e| format!("The connection was interrupted: {e}")),
                _ = cancelled(&mut shutdown) => Ok((0, 0)),
            }
        }
        Err(e) => Err(format!("Could not connect to {host}:{port}: {e}")),
    };
    let total = started.elapsed().as_millis() as u64;
    ctx.store.update(id, |f| {
        f.timing.total_ms = Some(total);
        match outcome {
            Ok((sent, received)) => {
                f.request_size = sent;
                f.response_size = received;
                f.state = FlowState::Completed;
            }
            Err(message) => {
                f.state = FlowState::Failed;
                f.error = Some(message);
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// HTTP requests
// ---------------------------------------------------------------------------------------------

fn header_list(headers: &HeaderMap) -> Headers {
    headers.iter().map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned())).collect()
}

fn is_websocket(headers: &HeaderMap) -> bool {
    let has = |name, value: &str| {
        headers.get_all(name).iter().any(|v| v.to_str().is_ok_and(|v| v.to_ascii_lowercase().contains(value)))
    };
    has(header::UPGRADE, "websocket") && has(header::CONNECTION, "upgrade")
}

/// Headers that describe one hop, not the message. Each side of the proxy has its own.
fn is_hop_by_hop(name: &header::HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "proxy-connection"
            | "keep-alive"
            | "transfer-encoding"
            | "te"
            | "upgrade"
            | "proxy-authorization"
    )
}

fn error_chain(e: &dyn std::error::Error) -> String {
    let mut message = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        let part = s.to_string();
        if !message.contains(&part) {
            message.push_str(": ");
            message.push_str(&part);
        }
        source = s.source();
    }
    message
}

/// A client that reached us by IP address (`CONNECT 93.184.216.34:443`) still names the site in
/// the `Host` header. Use that for the name the server certificate is checked against.
fn effective_host(connect_host: &str, headers: &HeaderMap) -> String {
    if connect_host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok()
        && let Some(named) = headers.get(header::HOST).and_then(|h| h.to_str().ok())
    {
        let name = named.rsplit_once(':').filter(|(_, p)| p.parse::<u16>().is_ok()).map_or(named, |(h, _)| h);
        if name.parse::<std::net::IpAddr>().is_err() && !name.is_empty() {
            return name.to_string();
        }
    }
    connect_host.to_string()
}

fn fail(ctx: &Ctx, id: FlowId, status: StatusCode, message: String) -> Resp {
    ctx.store.update(id, |f| {
        f.state = FlowState::Failed;
        f.error = Some(message.clone());
    });
    text(status, &format!("EmuLabZip could not complete the request.\n\n{message}\n"))
}

async fn forward(req: Request<Incoming>, ctx: Arc<Ctx>, scheme: &str, host: &str, port: u16) -> Resp {
    let started = Instant::now();
    let host = effective_host(host, req.headers());
    let upgrade = is_websocket(req.headers());
    let path = req.uri().path_and_query().map_or("/", |p| p.as_str()).to_string();

    let shown_scheme = match (upgrade, scheme) {
        (true, "https") => "wss",
        (true, _) => "ws",
        (false, s) => s,
    };
    let kind = if upgrade { FlowKind::WebSocket } else { FlowKind::Http };
    let mut flow = Flow::new(kind, req.method().as_str(), shown_scheme, &host, port, &path);
    flow.http_version = format!("{:?}", req.version());
    flow.request_headers = header_list(req.headers());

    if upgrade {
        return forward_upgrade(req, ctx, scheme, &host, port, flow, started).await;
    }

    let client_version = req.version();
    let (parts, body) = req.into_parts();
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            flow.state = FlowState::Failed;
            flow.error = Some(format!("Could not read the request body: {e}"));
            ctx.store.begin(flow);
            return text(StatusCode::BAD_REQUEST, "Could not read the request body.");
        }
    };
    flow.request_size = body.len() as u64;
    flow.request_truncated = body.len() > ctx.body_limit;
    flow.request_body = body[..body.len().min(ctx.body_limit)].to_vec();
    let id = ctx.store.begin(flow);

    // A rule may answer, delay, fail or hold the request.
    let rule = ctx.rules.find(parts.method.as_str(), &host, &path);
    let mut headers = parts.headers.clone();
    let mut body = body;
    if let Some(rule) = &rule {
        ctx.store.update(id, |f| f.rule = Some(rule.name.clone()));
        match &rule.action {
            Action::Mock { .. } => return mock(&ctx, id, started, client_version, rule).await,
            Action::Delay { ms } => tokio::time::sleep(std::time::Duration::from_millis(*ms)).await,
            Action::Block => {
                return fail(&ctx, id, StatusCode::BAD_GATEWAY, format!("Blocked by the rule “{}”.", rule.name));
            }
            Action::Breakpoint { request: true, .. } => {
                let hold = Hold {
                    flow: id,
                    phase: Phase::Request,
                    method: parts.method.to_string(),
                    url: format!("{scheme}://{host}:{port}{path}"),
                    status: None,
                    headers: header_list(&headers),
                    body: body.to_vec(),
                };
                ctx.store.update(id, |f| f.state = FlowState::Paused);
                match ctx.breakpoints.pause(hold).await {
                    Resume::Abort => return fail(&ctx, id, StatusCode::BAD_GATEWAY, "Stopped at a breakpoint.".into()),
                    Resume::Continue(edits) => {
                        apply_request_edits(&mut headers, &mut body, edits);
                        let shown = header_list(&headers);
                        let (size, kept) = (body.len() as u64, body[..body.len().min(ctx.body_limit)].to_vec());
                        ctx.store.update(id, |f| {
                            f.state = FlowState::Pending;
                            f.request_headers = shown;
                            f.request_size = size;
                            f.request_body = kept;
                        });
                    }
                }
            }
            Action::Breakpoint { .. } => {}
        }
    }

    let uri =
        match Uri::builder().scheme(scheme).authority(format!("{host}:{port}")).path_and_query(path.as_str()).build() {
            Ok(uri) => uri,
            Err(e) => return fail(&ctx, id, StatusCode::BAD_REQUEST, format!("The address is not valid: {e}")),
        };
    let h3_body = body.clone();
    let mut upstream = Request::new(Full::new(body));
    *upstream.method_mut() = parts.method.clone();
    *upstream.uri_mut() = uri;
    for (name, value) in &headers {
        // `Host` and the length come from the address and the body.
        if !is_hop_by_hop(name) && name != header::HOST && name != header::CONTENT_LENGTH {
            upstream.headers_mut().append(name.clone(), value.clone());
        }
    }

    let holds_response = matches!(rule.as_ref().map(|r| &r.action), Some(Action::Breakpoint { response: true, .. }));
    if scheme == "https"
        && !holds_response
        && let Some(udp_port) = ctx.http3.port_for(&host, port)
    {
        let sent =
            ctx.http3.request(&host, udp_port, parts.method.clone(), &path, upstream.headers(), h3_body.clone()).await;
        match sent {
            Ok((head, bytes)) => return respond_h3(&ctx, id, started, client_version, &host, port, head, bytes),
            Err(e) => {
                tracing::debug!(%host, error = %error_chain(&*e), "HTTP/3 failed; using TCP");
                ctx.http3.failed(&host, port);
            }
        }
    }

    match ctx.client.request(upstream).await {
        Ok(res) if holds_response => hold_response(&ctx, id, started, client_version, res, &parts.method).await,
        Ok(res) => respond(&ctx, id, started, client_version, res),
        Err(e) => fail(&ctx, id, StatusCode::BAD_GATEWAY, error_chain(&e)),
    }
}

fn protocol_name(version: http::Version) -> &'static str {
    match version {
        http::Version::HTTP_3 => "h3",
        http::Version::HTTP_2 => "h2",
        _ => "h1",
    }
}

/// Notes what the server said about HTTP/3, then takes it out of what the device will see.
fn learn_and_hide(ctx: &Ctx, id: FlowId, headers: &mut HeaderMap) {
    if let Some((host, port)) = ctx.store.get(id).map(|f| (f.host, f.port)) {
        ctx.http3.learn(&host, port, headers);
    }
    if ctx.http3.hides() {
        http3::hide_from_headers(headers);
    }
}

/// Records an answer that came over HTTP/3. It is read in full, so it is not streamed.
#[allow(clippy::too_many_arguments)]
fn respond_h3(
    ctx: &Ctx,
    id: FlowId,
    started: Instant,
    version: http::Version,
    host: &str,
    port: u16,
    mut parts: http::response::Parts,
    bytes: Bytes,
) -> Resp {
    let total = started.elapsed().as_millis() as u64;
    ctx.http3.learn(host, port, &parts.headers);
    let listed = header_list(&parts.headers);
    let (status, size) = (parts.status.as_u16(), bytes.len() as u64);
    ctx.store.update(id, |f| {
        f.status = Some(status);
        f.response_headers = listed;
        f.server_protocol = "h3".into();
        f.response_body = bytes[..bytes.len().min(ctx.body_limit)].to_vec();
        f.response_size = size;
        f.response_truncated = bytes.len() > ctx.body_limit;
        f.timing.ttfb_ms = Some(total);
        f.timing.total_ms = Some(total);
        f.state = FlowState::Completed;
    });
    if ctx.http3.hides() {
        http3::hide_from_headers(&mut parts.headers);
    }
    for name in parts.headers.keys().filter(|k| is_hop_by_hop(k)).cloned().collect::<Vec<_>>() {
        parts.headers.remove(name);
    }
    parts.version = version;
    Response::from_parts(parts, full(bytes))
}

/// Applies what was edited at a request breakpoint.
fn apply_request_edits(headers: &mut HeaderMap, body: &mut Bytes, edits: Edits) {
    if let Some(list) = edits.headers {
        *headers = to_header_map(&list);
    }
    if let Some(new_body) = edits.body {
        *body = Bytes::from(new_body);
    }
}

/// A header map from a list, skipping what is not a valid header.
fn to_header_map(list: &[(String, String)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in list {
        if let (Ok(name), Ok(value)) = (header::HeaderName::from_bytes(k.as_bytes()), header::HeaderValue::from_str(v))
        {
            map.append(name, value);
        }
    }
    map
}

/// Answers from a mock rule, without asking the server.
async fn mock(ctx: &Ctx, id: FlowId, started: Instant, version: http::Version, rule: &Rule) -> Resp {
    let Action::Mock { status, headers, body, base64, delay_ms } = &rule.action else {
        return fail(ctx, id, StatusCode::INTERNAL_SERVER_ERROR, "Not a mock rule.".into());
    };
    if *delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
    }
    let bytes = match mock_bytes(body, *base64) {
        Ok(bytes) => bytes,
        Err(e) => return fail(ctx, id, StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut map = to_header_map(headers);
    // The length is the body's, whatever the rule said.
    map.remove(header::CONTENT_LENGTH);
    map.remove(header::TRANSFER_ENCODING);
    let mut res = Response::new(full(bytes.clone()));
    *res.status_mut() = StatusCode::from_u16(*status).unwrap_or(StatusCode::OK);
    *res.headers_mut() = map;
    *res.version_mut() = version;
    let total = started.elapsed().as_millis() as u64;
    let (code, shown) = (*status, header_list(res.headers()));
    let size = bytes.len() as u64;
    ctx.store.update(id, |f| {
        f.status = Some(code);
        f.response_headers = shown;
        f.response_body = bytes[..bytes.len().min(ctx.body_limit)].to_vec();
        f.response_size = size;
        f.response_truncated = bytes.len() > ctx.body_limit;
        f.timing.ttfb_ms = Some(total);
        f.timing.total_ms = Some(total);
        f.mocked = true;
        f.state = FlowState::Completed;
    });
    res
}

/// Holds a response at a breakpoint. The body is read in full and decoded, so it can be edited as
/// text; what continues is sent uncompressed.
async fn hold_response(
    ctx: &Ctx,
    id: FlowId,
    started: Instant,
    version: http::Version,
    res: Response<Incoming>,
    method: &Method,
) -> Resp {
    let (mut parts, body) = res.into_parts();
    let ttfb = started.elapsed().as_millis() as u64;
    let raw = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return fail(
                ctx,
                id,
                StatusCode::BAD_GATEWAY,
                format!("The response was interrupted: {}", error_chain(&e)),
            );
        }
    };
    let encoding = parts.headers.get(header::CONTENT_ENCODING).and_then(|v| v.to_str().ok()).map(str::to_string);
    let decoded = crate::decode::decode(encoding.as_deref(), &raw).unwrap_or_else(|_| raw.to_vec());
    let mut shown = header_list(&parts.headers);
    if decoded.len() != raw.len() || encoding.is_some() {
        shown.retain(|(k, _)| !k.eq_ignore_ascii_case("content-encoding") && !k.eq_ignore_ascii_case("content-length"));
    }
    let url = ctx.store.get(id).map(|f| f.url).unwrap_or_default();
    ctx.store.update(id, |f| {
        f.status = Some(parts.status.as_u16());
        f.response_headers = shown.clone();
        f.server_protocol = protocol_name(parts.version).to_string();
        f.timing.ttfb_ms = Some(ttfb);
        f.state = FlowState::Paused;
    });
    let hold = Hold {
        flow: id,
        phase: Phase::Response,
        method: method.to_string(),
        url,
        status: Some(parts.status.as_u16()),
        headers: shown,
        body: decoded.clone(),
    };
    let (mut headers, mut bytes, mut status) = (to_header_map(&hold.headers), Bytes::from(decoded), parts.status);
    match ctx.breakpoints.pause(hold).await {
        Resume::Abort => return fail(ctx, id, StatusCode::BAD_GATEWAY, "Stopped at a breakpoint.".into()),
        Resume::Continue(edits) => {
            if let Some(list) = edits.headers {
                headers = to_header_map(&list);
            }
            if let Some(new_body) = edits.body {
                bytes = Bytes::from(new_body);
            }
            if let Some(code) = edits.status.and_then(|c| StatusCode::from_u16(c).ok()) {
                status = code;
            }
        }
    }
    for name in headers.keys().filter(|k| is_hop_by_hop(k)).cloned().collect::<Vec<_>>() {
        headers.remove(name);
    }
    headers.remove(header::CONTENT_LENGTH);
    headers.remove(header::CONTENT_ENCODING);
    parts.status = status;
    parts.headers = headers;
    parts.version = version;
    let total = started.elapsed().as_millis() as u64;
    let listed = header_list(&parts.headers);
    let size = bytes.len() as u64;
    ctx.store.update(id, |f| {
        f.status = Some(status.as_u16());
        f.response_headers = listed;
        f.response_body = bytes[..bytes.len().min(ctx.body_limit)].to_vec();
        f.response_size = size;
        f.response_truncated = bytes.len() > ctx.body_limit;
        f.timing.total_ms = Some(total);
        f.state = FlowState::Completed;
    });
    Response::from_parts(parts, full(bytes))
}

/// Records the response head and returns the response, with a body that records itself as it
/// streams to the client.
fn respond(ctx: &Ctx, id: FlowId, started: Instant, version: http::Version, res: Response<Incoming>) -> Resp {
    let (mut parts, body) = res.into_parts();
    let ttfb = started.elapsed().as_millis() as u64;
    let headers = header_list(&parts.headers);
    let status = parts.status.as_u16();
    let protocol = protocol_name(parts.version);
    ctx.store.update(id, |f| {
        f.status = Some(status);
        f.response_headers = headers;
        f.server_protocol = protocol.to_string();
        f.timing.ttfb_ms = Some(ttfb);
    });
    learn_and_hide(ctx, id, &mut parts.headers);
    // Set aside the headers that only made sense on the upstream connection.
    let hop: Vec<_> = parts.headers.keys().filter(|k| is_hop_by_hop(k)).cloned().collect();
    for name in hop {
        parts.headers.remove(name);
    }
    parts.version = version;
    let tapped = TapBody {
        inner: body,
        store: ctx.store.clone(),
        id,
        started,
        limit: ctx.body_limit,
        buf: Vec::new(),
        size: 0,
        finished: false,
    };
    Response::from_parts(parts, tapped.map_err(|e| -> BoxErr { Box::new(e) }).boxed_unsync())
}

/// A response body that copies what passes through it into the flow, so downloads and event
/// streams keep streaming while they are recorded.
struct TapBody {
    inner: Incoming,
    store: Arc<FlowStore>,
    id: FlowId,
    started: Instant,
    limit: usize,
    buf: Vec<u8>,
    size: u64,
    finished: bool,
}

impl TapBody {
    fn finish(&mut self, error: Option<String>) {
        if std::mem::replace(&mut self.finished, true) {
            return;
        }
        let total = self.started.elapsed().as_millis() as u64;
        let (body, size) = (std::mem::take(&mut self.buf), self.size);
        let truncated = size as usize > self.limit;
        self.store.update(self.id, |f| {
            f.response_body = body;
            f.response_size = size;
            f.response_truncated = truncated;
            f.timing.total_ms = Some(total);
            match error {
                Some(e) => {
                    f.state = FlowState::Failed;
                    f.error = Some(e);
                }
                None => f.state = FlowState::Completed,
            }
        });
    }
}

impl Body for TapBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Bytes>, hyper::Error>>> {
        let this = &mut *self;
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.size += data.len() as u64;
                    let room = this.limit.saturating_sub(this.buf.len());
                    this.buf.extend_from_slice(&data[..data.len().min(room)]);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => {
                this.finish(Some(format!("The response was interrupted: {}", error_chain(&e))));
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                this.finish(None);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for TapBody {
    fn drop(&mut self) {
        // Responses without a body are never polled; the client leaving early is an interruption.
        let error = (!self.inner.is_end_stream())
            .then(|| "The client closed the connection before the response ended.".to_string());
        self.finish(error);
    }
}

// ---------------------------------------------------------------------------------------------
// WebSockets
// ---------------------------------------------------------------------------------------------

async fn connect_upstream(ctx: &Ctx, scheme: &str, host: &str, port: u16) -> std::result::Result<BoxIo, String> {
    let bare = host.trim_matches(['[', ']']);
    let tcp = TcpStream::connect((bare, port)).await.map_err(|e| format!("Could not connect to {host}:{port}: {e}"))?;
    let _ = tcp.set_nodelay(true);
    if scheme != "https" {
        return Ok(Box::new(tcp));
    }
    let name = ServerName::try_from(bare.to_string()).map_err(|e| format!("{host} is not a valid server name: {e}"))?;
    let tls = ctx.upgrade_tls.connect(name, tcp).await.map_err(|e| format!("TLS handshake with {host} failed: {e}"))?;
    Ok(Box::new(tls))
}

async fn forward_upgrade(
    mut req: Request<Incoming>,
    ctx: Arc<Ctx>,
    scheme: &str,
    host: &str,
    port: u16,
    flow: Flow,
    started: Instant,
) -> Resp {
    let id = ctx.store.begin(flow);
    let client_version = req.version();
    let client_upgrade = hyper::upgrade::on(&mut req);

    let io = match connect_upstream(&ctx, scheme, host, port).await {
        Ok(io) => io,
        Err(message) => return fail(&ctx, id, StatusCode::BAD_GATEWAY, message),
    };
    let (mut sender, conn) = match hyper::client::conn::http1::handshake::<_, Empty<Bytes>>(TokioIo::new(io)).await {
        Ok(pair) => pair,
        Err(e) => return fail(&ctx, id, StatusCode::BAD_GATEWAY, error_chain(&e)),
    };
    tokio::spawn(conn.with_upgrades());

    let (parts, _) = req.into_parts();
    let mut upstream = Request::new(Empty::<Bytes>::new());
    *upstream.method_mut() = parts.method;
    *upstream.uri_mut() = parts
        .uri
        .path_and_query()
        .map_or(Uri::from_static("/"), |p| Uri::try_from(p.as_str()).unwrap_or(Uri::from_static("/")));
    *upstream.headers_mut() = parts.headers;
    let default_port = if scheme == "https" { 443 } else { 80 };
    let host_header = if port == default_port { host.to_string() } else { format!("{host}:{port}") };
    if let Ok(value) = host_header.parse() {
        upstream.headers_mut().insert(header::HOST, value);
    }

    let mut res = match sender.send_request(upstream).await {
        Ok(res) => res,
        Err(e) => return fail(&ctx, id, StatusCode::BAD_GATEWAY, error_chain(&e)),
    };
    if res.status() != StatusCode::SWITCHING_PROTOCOLS {
        // The server declined the upgrade; this is an ordinary response now.
        return respond(&ctx, id, started, client_version, res);
    }

    let upstream_upgrade = hyper::upgrade::on(&mut res);
    let (status, headers) = (res.status(), res.headers().clone());
    let listed = header_list(&headers);
    let ttfb = started.elapsed().as_millis() as u64;
    ctx.store.update(id, |f| {
        f.status = Some(status.as_u16());
        f.response_headers = listed;
        f.timing.ttfb_ms = Some(ttfb);
    });

    let deflate = headers.get("sec-websocket-extensions").and_then(|v| v.to_str().ok()).and_then(parse_extensions);
    let task_ctx = ctx.clone();
    tokio::spawn(async move {
        let mut shutdown = task_ctx.shutdown.clone();
        let outcome = match (client_upgrade.await, upstream_upgrade.await) {
            (Ok(client), Ok(server)) => {
                let copy = tapped_copy(TokioIo::new(client), TokioIo::new(server), &task_ctx, id, started, deflate);
                tokio::select! {
                    r = copy => r.map_err(|e| format!("The connection was interrupted: {e}")),
                    _ = cancelled(&mut shutdown) => Ok((0, 0)),
                }
            }
            (Err(e), _) | (_, Err(e)) => Err(format!("The upgrade failed: {e}")),
        };
        let total = started.elapsed().as_millis() as u64;
        task_ctx.store.update(id, |f| {
            f.timing.total_ms = Some(total);
            match outcome {
                Ok((sent, received)) => {
                    f.request_size = sent;
                    f.response_size = received;
                    f.state = FlowState::Completed;
                }
                Err(message) => {
                    f.state = FlowState::Failed;
                    f.error = Some(message);
                }
            }
        });
    });

    let mut out = Response::new(empty());
    *out.status_mut() = status;
    *out.headers_mut() = headers;
    out
}

/// Copies both ways like `copy_bidirectional`, and reads the WebSocket messages on the way.
async fn tapped_copy<C, S>(
    client: C,
    server: S,
    ctx: &Ctx,
    id: FlowId,
    started: Instant,
    deflate: Option<Deflate>,
) -> std::io::Result<(u64, u64)>
where
    C: AsyncRead + AsyncWrite + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (client_read, client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let up = tap(
        client_read,
        server_write,
        FrameParser::new(deflate.map(|d| d.client_no_context_takeover)),
        Direction::Up,
        ctx,
        id,
        started,
    );
    let down = tap(
        server_read,
        client_write,
        FrameParser::new(deflate.map(|d| d.server_no_context_takeover)),
        Direction::Down,
        ctx,
        id,
        started,
    );
    tokio::try_join!(up, down)
}

/// Moves the bytes of one direction and shows them to the parser. The parser never holds them
/// back: what is forwarded is exactly what was read.
async fn tap<R, W>(
    mut from: R,
    mut to: W,
    mut parser: FrameParser,
    direction: Direction,
    ctx: &Ctx,
    id: FlowId,
    started: Instant,
) -> std::io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;
    let (mut buf, mut total) = (vec![0u8; 16 * 1024], 0u64);
    loop {
        let n = from.read(&mut buf).await?;
        if n == 0 {
            let _ = to.shutdown().await;
            return Ok(total);
        }
        total += n as u64;
        if !parser.is_broken() {
            for message in parser.feed(&buf[..n]) {
                let stored = message.into_stored(started.elapsed().as_millis() as u64, direction);
                ctx.store.update(id, |f| {
                    if f.messages.len() < MAX_MESSAGES {
                        f.messages.push(stored);
                    } else {
                        f.messages_dropped += 1;
                    }
                });
            }
        }
        to.write_all(&buf[..n]).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(header::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
        }
        h
    }

    #[test]
    fn recognises_websocket_upgrades() {
        assert!(is_websocket(&headers(&[("upgrade", "WebSocket"), ("connection", "keep-alive, Upgrade")])));
        assert!(!is_websocket(&headers(&[("upgrade", "h2c"), ("connection", "Upgrade")])));
        assert!(!is_websocket(&headers(&[("accept", "*/*")])));
    }

    #[test]
    fn prefers_the_host_header_when_connected_by_address() {
        let h = headers(&[("host", "api.example.com:8443")]);
        assert_eq!(effective_host("93.184.216.34", &h), "api.example.com");
        assert_eq!(effective_host("example.org", &h), "example.org");
        assert_eq!(effective_host("93.184.216.34", &headers(&[("host", "10.0.0.1")])), "93.184.216.34");
    }

    #[test]
    fn lists_hop_by_hop_headers() {
        assert!(is_hop_by_hop(&header::CONNECTION));
        assert!(is_hop_by_hop(&header::TRANSFER_ENCODING));
        assert!(!is_hop_by_hop(&header::CONTENT_TYPE));
    }
}
