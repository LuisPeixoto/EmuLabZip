//! HTTP/3 from the proxy to servers.
//!
//! A device behind an HTTP proxy never speaks QUIC to it, so this only concerns the second leg. The
//! proxy learns from `Alt-Svc` which hosts offer HTTP/3, and, when it is switched on, sends the next
//! requests to those hosts over QUIC. A host whose QUIC attempt fails goes back to HTTP/2 or 1.1 and
//! is not tried again for a while.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bytes::{Buf, Bytes};
use http::{HeaderMap, HeaderValue, Method, Request, header};
use parking_lot::Mutex;

/// How long a host stays on TCP after QUIC failed.
const FAILURE_MEMORY: Duration = Duration::from_secs(600);
/// How long the connection and the first bytes of an answer may take.
const TIMEOUT: Duration = Duration::from_secs(8);
/// The longest an advertisement is believed when the server sends none.
const DEFAULT_MAX_AGE: Duration = Duration::from_secs(86_400);

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

/// One `h3` entry of an `Alt-Svc` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advert {
    pub port: u16,
    pub max_age: Duration,
}

/// What the `Alt-Svc` header of a response says about HTTP/3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AltSvc {
    /// The host offers HTTP/3 on this port.
    Offers(Advert),
    /// The host withdrew every alternative.
    Clear,
    /// Nothing about HTTP/3.
    None,
}

/// Splits on the commas that are not inside quotes.
fn entries(value: &str) -> Vec<&str> {
    let (mut out, mut start, mut quoted) = (Vec::new(), 0, false);
    for (i, c) in value.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(value[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(value[start..].trim());
    out.retain(|e| !e.is_empty());
    out
}

/// Reads an `Alt-Svc` value. Only the final `h3` version counts: the draft ones are long gone.
pub fn parse_alt_svc(value: &str) -> AltSvc {
    if value.trim().eq_ignore_ascii_case("clear") {
        return AltSvc::Clear;
    }
    for entry in entries(value) {
        let mut pieces = entry.split(';').map(str::trim);
        let Some((proto, authority)) = pieces.next().and_then(|p| p.split_once('=')) else { continue };
        if proto.trim() != "h3" {
            continue;
        }
        // `":443"` or `"alt.example.com:443"`; a different host is not followed.
        let authority = authority.trim().trim_matches('"');
        let Some((host, port)) = authority.rsplit_once(':') else { continue };
        if !host.is_empty() {
            continue;
        }
        let Ok(port) = port.parse() else { continue };
        let max_age = pieces
            .filter_map(|p| p.split_once('='))
            .find(|(k, _)| k.trim() == "ma")
            .and_then(|(_, v)| v.trim().trim_matches('"').parse().ok())
            .map_or(DEFAULT_MAX_AGE, Duration::from_secs);
        return AltSvc::Offers(Advert { port, max_age });
    }
    AltSvc::None
}

/// Removes the HTTP/3 entries from an `Alt-Svc` value, so that an app does not try QUIC around the
/// proxy. Returns `None` when nothing is left to send.
pub fn strip_h3(value: &str) -> Option<String> {
    let kept: Vec<&str> = entries(value)
        .into_iter()
        .filter(|e| !e.split('=').next().is_some_and(|p| p.trim().starts_with("h3")))
        .collect();
    if kept.is_empty() { None } else { Some(kept.join(", ")) }
}

/// Takes the HTTP/3 entries out of a response's headers.
pub fn hide_from_headers(headers: &mut HeaderMap) {
    let values: Vec<String> =
        headers.get_all(header::ALT_SVC).iter().filter_map(|v| v.to_str().ok()).map(str::to_string).collect();
    if values.is_empty() {
        return;
    }
    headers.remove(header::ALT_SVC);
    for kept in values.iter().filter_map(|v| strip_h3(v)) {
        if let Ok(v) = HeaderValue::from_str(&kept) {
            headers.append(header::ALT_SVC, v);
        }
    }
}

enum Known {
    Offers { port: u16, until: Instant },
    Failed { until: Instant },
}

/// The HTTP/3 side of the proxy: what hosts offer, and the client that talks to them.
pub struct Http3 {
    enabled: AtomicBool,
    hide: AtomicBool,
    hosts: Mutex<HashMap<String, Known>>,
    tls: Arc<rustls::ClientConfig>,
    /// One socket per address family: a socket bound to IPv4 cannot reach an IPv6 address.
    endpoints: Mutex<[Option<quinn::Endpoint>; 2]>,
}

impl Http3 {
    pub fn new(tls: &rustls::ClientConfig, enabled: bool, hide: bool) -> Self {
        let mut tls = tls.clone();
        tls.alpn_protocols = vec![b"h3".to_vec()];
        Http3 {
            enabled: AtomicBool::new(enabled),
            hide: AtomicBool::new(hide),
            hosts: Mutex::new(HashMap::new()),
            tls: Arc::new(tls),
            endpoints: Mutex::new([None, None]),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn hides(&self) -> bool {
        self.hide.load(Ordering::Relaxed)
    }

    pub fn set_hide(&self, on: bool) {
        self.hide.store(on, Ordering::Relaxed);
    }

    fn key(host: &str, port: u16) -> String {
        format!("{}:{port}", host.to_ascii_lowercase())
    }

    /// Notes what a response said about HTTP/3. Learning goes on while the feature is off, so
    /// switching it on takes effect at once.
    pub fn learn(&self, host: &str, port: u16, headers: &HeaderMap) {
        let key = Self::key(host, port);
        for value in headers.get_all(header::ALT_SVC).iter().filter_map(|v| v.to_str().ok()) {
            match parse_alt_svc(value) {
                AltSvc::Offers(Advert { port, max_age }) => {
                    let mut hosts = self.hosts.lock();
                    // A recent failure is not undone by the same server saying it can do HTTP/3.
                    if !matches!(hosts.get(&key), Some(Known::Failed { until }) if *until > Instant::now()) {
                        hosts.insert(key, Known::Offers { port, until: Instant::now() + max_age });
                    }
                    return;
                }
                AltSvc::Clear => {
                    self.hosts.lock().remove(&key);
                    return;
                }
                AltSvc::None => {}
            }
        }
    }

    /// The UDP port to use for this host, when HTTP/3 is on and the host is known to offer it.
    pub fn port_for(&self, host: &str, port: u16) -> Option<u16> {
        if !self.enabled() {
            return None;
        }
        let key = Self::key(host, port);
        let mut hosts = self.hosts.lock();
        match hosts.get(&key) {
            Some(Known::Offers { port, until }) if *until > Instant::now() => Some(*port),
            Some(Known::Failed { until }) if *until > Instant::now() => None,
            Some(_) => {
                hosts.remove(&key);
                None
            }
            None => None,
        }
    }

    /// Keeps a host off QUIC for a while.
    pub fn failed(&self, host: &str, port: u16) {
        self.hosts.lock().insert(Self::key(host, port), Known::Failed { until: Instant::now() + FAILURE_MEMORY });
    }

    fn endpoint(&self, v6: bool) -> Result<quinn::Endpoint, BoxErr> {
        let mut slots = self.endpoints.lock();
        if let Some(endpoint) = &slots[usize::from(v6)] {
            return Ok(endpoint.clone());
        }
        let quic = quinn::crypto::rustls::QuicClientConfig::try_from(self.tls.clone())?;
        let bind = if v6 { SocketAddr::from(([0u16; 8], 0)) } else { SocketAddr::from(([0, 0, 0, 0], 0)) };
        let mut endpoint = quinn::Endpoint::client(bind)?;
        endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic)));
        slots[usize::from(v6)] = Some(endpoint.clone());
        Ok(endpoint)
    }

    /// Sends one request over HTTP/3 and reads the whole answer.
    pub async fn request(
        &self,
        host: &str,
        udp_port: u16,
        method: Method,
        path: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<(http::response::Parts, Bytes), BoxErr> {
        tokio::time::timeout(TIMEOUT, self.exchange(host, udp_port, method, path, headers, body))
            .await
            .map_err(|_| BoxErr::from("the server did not answer over HTTP/3 in time"))?
    }

    async fn exchange(
        &self,
        host: &str,
        udp_port: u16,
        method: Method,
        path: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<(http::response::Parts, Bytes), BoxErr> {
        // IPv4 first: it is the one that works on the most networks.
        let mut addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, udp_port)).await?.collect();
        addrs.sort_by_key(|a| a.is_ipv6());
        let addr = *addrs.first().ok_or("the host has no address")?;
        let connection = self.endpoint(addr.is_ipv6())?.connect(addr, host)?.await?;
        let (mut driver, mut send) = h3::client::new(h3_quinn::Connection::new(connection.clone())).await?;
        let drive = tokio::spawn(async move {
            let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
        });

        let authority = if udp_port == 443 { host.to_string() } else { format!("{host}:{udp_port}") };
        let mut builder = Request::builder().method(method).uri(format!("https://{authority}{path}"));
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        let request = builder.body(())?;
        let mut stream = send.send_request(request).await?;
        if !body.is_empty() {
            stream.send_data(body).await?;
        }
        stream.finish().await?;
        let response = stream.recv_response().await?;
        let (parts, ()) = response.into_parts();
        let mut collected = Vec::new();
        while let Some(mut chunk) = stream.recv_data().await? {
            collected.extend_from_slice(&chunk.copy_to_bytes(chunk.remaining()));
        }
        connection.close(0u32.into(), b"done");
        drive.abort();
        Ok((parts, Bytes::from(collected)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_h3_advertisement() {
        let got = parse_alt_svc(r#"h3=":443"; ma=86400, h3-29=":443"; ma=86400"#);
        assert_eq!(got, AltSvc::Offers(Advert { port: 443, max_age: Duration::from_secs(86_400) }));
    }

    #[test]
    fn a_missing_max_age_means_a_day() {
        assert_eq!(parse_alt_svc(r#"h3=":8443""#), AltSvc::Offers(Advert { port: 8443, max_age: DEFAULT_MAX_AGE }));
    }

    #[test]
    fn other_protocols_and_other_hosts_are_not_followed() {
        assert_eq!(parse_alt_svc(r#"h2=":443"; ma=60"#), AltSvc::None);
        assert_eq!(parse_alt_svc(r#"h3-29=":443""#), AltSvc::None);
        assert_eq!(parse_alt_svc(r#"h3="other.example.com:443""#), AltSvc::None);
        assert_eq!(parse_alt_svc(r#"h3=":nope""#), AltSvc::None);
        assert_eq!(parse_alt_svc(""), AltSvc::None);
    }

    #[test]
    fn clear_withdraws_everything() {
        assert_eq!(parse_alt_svc("clear"), AltSvc::Clear);
        assert_eq!(parse_alt_svc("  Clear "), AltSvc::Clear);
    }

    #[test]
    fn a_comma_inside_quotes_does_not_split() {
        assert_eq!(strip_h3(r#"h2="a,b:443", h3=":443""#).as_deref(), Some(r#"h2="a,b:443""#));
    }

    #[test]
    fn stripping_keeps_what_is_not_http3() {
        assert_eq!(strip_h3(r#"h3=":443"; ma=86400, h3-29=":443""#), None);
        assert_eq!(strip_h3(r#"h3=":443", h2=":443"; ma=60"#).as_deref(), Some(r#"h2=":443"; ma=60"#));
        assert_eq!(strip_h3("clear").as_deref(), Some("clear"));
    }

    #[test]
    fn headers_lose_only_the_h3_entries() {
        let mut headers = HeaderMap::new();
        headers.append(header::ALT_SVC, HeaderValue::from_static(r#"h3=":443"; ma=1"#));
        headers.append(header::ALT_SVC, HeaderValue::from_static(r#"h2=":443""#));
        hide_from_headers(&mut headers);
        let left: Vec<_> = headers.get_all(header::ALT_SVC).iter().collect();
        assert_eq!(left, [&HeaderValue::from_static(r#"h2=":443""#)]);
        let mut only = HeaderMap::new();
        only.append(header::ALT_SVC, HeaderValue::from_static(r#"h3=":443""#));
        hide_from_headers(&mut only);
        assert!(!only.contains_key(header::ALT_SVC));
    }

    fn client() -> Http3 {
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        Http3::new(&tls, true, true)
    }

    fn offering() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::ALT_SVC, HeaderValue::from_static(r#"h3=":443"; ma=60"#));
        h
    }

    #[test]
    fn a_host_is_remembered_only_while_the_feature_is_on() {
        let h3 = client();
        h3.learn("Example.com", 443, &offering());
        assert_eq!(h3.port_for("example.com", 443), Some(443));
        assert_eq!(h3.port_for("example.com", 8443), None);
        h3.set_enabled(false);
        assert_eq!(h3.port_for("example.com", 443), None);
        h3.set_enabled(true);
        assert_eq!(h3.port_for("example.com", 443), Some(443), "learned while off, used once on");
    }

    #[test]
    fn a_failure_keeps_the_host_on_tcp_even_if_it_keeps_advertising() {
        let h3 = client();
        h3.learn("example.com", 443, &offering());
        h3.failed("example.com", 443);
        assert_eq!(h3.port_for("example.com", 443), None);
        h3.learn("example.com", 443, &offering());
        assert_eq!(h3.port_for("example.com", 443), None);
    }

    #[test]
    fn clear_forgets_the_host() {
        let h3 = client();
        h3.learn("example.com", 443, &offering());
        let mut clear = HeaderMap::new();
        clear.insert(header::ALT_SVC, HeaderValue::from_static("clear"));
        h3.learn("example.com", 443, &clear);
        assert_eq!(h3.port_for("example.com", 443), None);
    }
}
