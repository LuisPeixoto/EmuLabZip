//! A flow is one request and its response, or one tunnel, as the proxy saw it.

use serde::Serialize;

pub type FlowId = u64;
pub type Headers = Vec<(String, String)>;

/// How much of a body is kept. The rest is still forwarded, only not stored.
pub const BODY_LIMIT: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlowKind {
    /// A decoded HTTP request and response.
    Http,
    /// A connection the proxy did not decode (not TLS, or an app that rejects our certificate).
    Tunnel,
    /// An upgraded connection. The frames are not decoded.
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlowState {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    /// Until the response headers arrived.
    pub ttfb_ms: Option<u64>,
    /// Until the response body was complete.
    pub total_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub id: FlowId,
    /// Milliseconds since the Unix epoch.
    pub started_ms: u64,
    pub kind: FlowKind,
    pub state: FlowState,
    pub method: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    /// Path and query.
    pub path: String,
    pub url: String,
    /// The version the client spoke to the proxy.
    pub http_version: String,
    pub status: Option<u16>,
    pub request_headers: Headers,
    pub response_headers: Headers,
    #[serde(skip)]
    pub request_body: Vec<u8>,
    #[serde(skip)]
    pub response_body: Vec<u8>,
    /// Bytes on the wire, which can exceed what is stored.
    pub request_size: u64,
    pub response_size: u64,
    pub request_truncated: bool,
    pub response_truncated: bool,
    pub timing: Timing,
    pub error: Option<String>,
}

impl Flow {
    pub fn new(kind: FlowKind, method: &str, scheme: &str, host: &str, port: u16, path: &str) -> Self {
        let default_port = if scheme == "https" || scheme == "wss" { 443 } else { 80 };
        let port_part = if port == default_port { String::new() } else { format!(":{port}") };
        Flow {
            id: 0,
            started_ms: now_ms(),
            kind,
            state: FlowState::Pending,
            method: method.to_string(),
            scheme: scheme.to_string(),
            host: host.to_string(),
            port,
            path: path.to_string(),
            url: format!("{scheme}://{host}{port_part}{path}"),
            http_version: String::new(),
            status: None,
            request_headers: Vec::new(),
            response_headers: Vec::new(),
            request_body: Vec::new(),
            response_body: Vec::new(),
            request_size: 0,
            response_size: 0,
            request_truncated: false,
            response_truncated: false,
            timing: Timing::default(),
            error: None,
        }
    }

    pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The response's `Content-Type` without parameters, such as `application/json`.
    pub fn content_type(&self) -> Option<String> {
        Self::header(&self.response_headers, "content-type")
            .map(|v| v.split(';').next().unwrap_or(v).trim().to_ascii_lowercase())
    }

    pub fn summary(&self) -> FlowSummary {
        FlowSummary {
            id: self.id,
            started_ms: self.started_ms,
            kind: self.kind,
            state: self.state,
            method: self.method.clone(),
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            port: self.port,
            path: self.path.clone(),
            url: self.url.clone(),
            http_version: self.http_version.clone(),
            status: self.status,
            content_type: self.content_type(),
            request_size: self.request_size,
            response_size: self.response_size,
            duration_ms: self.timing.total_ms,
            error: self.error.clone(),
        }
    }
}

/// What the flow list needs, without headers or bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    pub id: FlowId,
    pub started_ms: u64,
    pub kind: FlowKind,
    pub state: FlowState,
    pub method: String,
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub url: String,
    pub http_version: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub request_size: u64,
    pub response_size: u64,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls_without_default_ports() {
        assert_eq!(Flow::new(FlowKind::Http, "GET", "https", "a.com", 443, "/x?y=1").url, "https://a.com/x?y=1");
        assert_eq!(Flow::new(FlowKind::Http, "GET", "http", "a.com", 8080, "/").url, "http://a.com:8080/");
    }

    #[test]
    fn reads_the_content_type_without_parameters() {
        let mut f = Flow::new(FlowKind::Http, "GET", "https", "a.com", 443, "/");
        f.response_headers.push(("Content-Type".into(), "Application/JSON; charset=utf-8".into()));
        assert_eq!(f.content_type().as_deref(), Some("application/json"));
        assert_eq!(f.summary().content_type.as_deref(), Some("application/json"));
    }
}
