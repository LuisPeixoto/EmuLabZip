//! Answering requests from a recording, so an app runs against what a server said once and never reaches the
//! internet.
//!
//! A recording is a HAR file (the proxy writes one into every session). It is turned into a [`ReplaySet`], which the
//! proxy asks before it asks the real server: a request that was recorded gets the recorded answer, and one that
//! was not gets a 504, so a test fails on a request it did not expect instead of quietly using the network.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use parking_lot::Mutex;
use serde_json::Value;

use crate::flow::Headers;

/// Response headers a replay never repeats: the recorded body is stored decoded and re-framed, and `alt-svc` would
/// send the app around the proxy over QUIC.
const DROPPED_HEADERS: &[&str] =
    &["content-encoding", "content-length", "transfer-encoding", "connection", "keep-alive", "alt-svc"];
/// The most distinct misses remembered, for showing what the app asked for that was not recorded.
const MAX_MISSES: usize = 50;
/// The most requests a recording can hold.
const MAX_ENTRIES: usize = 50_000;

/// One recorded exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub method: String,
    pub host: String,
    pub port: u16,
    /// Path and query, as sent.
    pub path: String,
    pub request_body: Vec<u8>,
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
    pub ttfb_ms: u64,
    pub total_ms: u64,
}

/// What identifies a request: its method, host, path and query. The order of query parameters does not matter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    method: String,
    host: String,
    path: String,
    query: String,
}

impl Key {
    fn of(method: &str, host: &str, path_and_query: &str) -> Key {
        let (path, query) = path_and_query.split_once('?').unwrap_or((path_and_query, ""));
        let mut pairs: Vec<&str> = query.split('&').filter(|p| !p.is_empty()).collect();
        pairs.sort_unstable();
        Key {
            method: method.to_ascii_uppercase(),
            host: host.trim_matches(['[', ']']).to_ascii_lowercase(),
            path: if path.is_empty() { "/".into() } else { path.to_string() },
            query: pairs.join("&"),
        }
    }
}

/// How a replay is being used, to show to someone.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayStats {
    pub source: String,
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub passthrough: bool,
    /// The requests that were not in the recording, once each, newest last.
    pub missed: Vec<String>,
}

pub struct ReplaySet {
    source: String,
    entries: Vec<Recorded>,
    index: HashMap<Key, Vec<usize>>,
    /// Which answer a request gets next, for the ones recorded more than once.
    cursors: Mutex<HashMap<(Key, u64), usize>>,
    hits: AtomicU64,
    misses: AtomicU64,
    missed: Mutex<Vec<String>>,
    /// Requests that are not in the recording go to the real server instead of failing.
    passthrough: bool,
    /// Answers wait as long as the server did.
    timing: bool,
}

impl std::fmt::Debug for ReplaySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplaySet").field("source", &self.source).field("entries", &self.entries.len()).finish()
    }
}

impl ReplaySet {
    /// Builds a set from a HAR document. `source` names it for the interface (a file name).
    pub fn from_har(har: &Value, source: &str) -> Result<ReplaySet, String> {
        let entries = har
            .pointer("/log/entries")
            .and_then(Value::as_array)
            .ok_or("This is not a HAR file: it has no log.entries.")?;
        let recorded: Vec<Recorded> = entries.iter().filter_map(read_entry).take(MAX_ENTRIES).collect();
        Ok(ReplaySet::new(recorded, source))
    }

    pub fn new(entries: Vec<Recorded>, source: &str) -> ReplaySet {
        let mut index: HashMap<Key, Vec<usize>> = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            index.entry(Key::of(&e.method, &e.host, &e.path)).or_default().push(i);
        }
        ReplaySet {
            source: source.to_string(),
            entries,
            index,
            cursors: Mutex::new(HashMap::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            missed: Mutex::new(Vec::new()),
            passthrough: false,
            timing: false,
        }
    }

    /// Lets requests that are not in the recording through to the real server.
    pub fn passthrough(mut self, on: bool) -> ReplaySet {
        self.passthrough = on;
        self
    }

    /// Makes each answer wait as long as the recorded one did.
    pub fn with_timing(mut self, on: bool) -> ReplaySet {
        self.timing = on;
        self
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether a request that is not in the recording fails (the default) instead of going to the server.
    pub fn is_strict(&self) -> bool {
        !self.passthrough
    }

    pub fn keeps_timing(&self) -> bool {
        self.timing
    }

    /// The answer to a request, or `None` when it was not recorded. A request recorded several times gets the
    /// answers in the order they were recorded, and the last one again after that. When the recorded requests differ
    /// in their body, the one with the same body is preferred.
    pub fn lookup(&self, method: &str, host: &str, path_and_query: &str, body: &[u8]) -> Option<&Recorded> {
        let key = Key::of(method, host, path_and_query);
        let Some(all) = self.index.get(&key) else {
            self.miss(method, host, path_and_query);
            return None;
        };
        let same_body: Vec<usize> = if !body.is_empty() && all.len() > 1 {
            all.iter().copied().filter(|i| self.entries[*i].request_body == body).collect()
        } else {
            Vec::new()
        };
        let (candidates, which) =
            if same_body.is_empty() { (all.as_slice(), 0) } else { (same_body.as_slice(), hash(body)) };
        let mut cursors = self.cursors.lock();
        let at = cursors.entry((key, which)).or_insert(0);
        let chosen = candidates[(*at).min(candidates.len() - 1)];
        *at += 1;
        self.hits.fetch_add(1, Ordering::Relaxed);
        Some(&self.entries[chosen])
    }

    fn miss(&self, method: &str, host: &str, path_and_query: &str) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        let line = format!("{} {host}{path_and_query}", method.to_ascii_uppercase());
        let mut missed = self.missed.lock();
        if !missed.contains(&line) {
            if missed.len() >= MAX_MISSES {
                missed.remove(0);
            }
            missed.push(line);
        }
    }

    pub fn stats(&self) -> ReplayStats {
        ReplayStats {
            source: self.source.clone(),
            entries: self.entries.len(),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            passthrough: self.passthrough,
            missed: self.missed.lock().clone(),
        }
    }
}

fn hash(bytes: &[u8]) -> u64 {
    // FNV-1a: only to tell bodies apart inside one set.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// One HAR entry as a recorded exchange. Entries that cannot be answered are left out: failed requests, WebSockets
/// (their messages are not replayed) and gRPC calls (their trailers are not in a HAR).
fn read_entry(e: &Value) -> Option<Recorded> {
    if e.get("_resourceType").and_then(Value::as_str) == Some("websocket") || e.get("_webSocketMessages").is_some() {
        return None;
    }
    let status = e.pointer("/response/status").and_then(Value::as_u64)? as u16;
    if status == 0 {
        return None;
    }
    let method = e.pointer("/request/method").and_then(Value::as_str)?.to_ascii_uppercase();
    let url: http::Uri = e.pointer("/request/url").and_then(Value::as_str)?.parse().ok()?;
    let host = url.host()?.to_string();
    let port = url.port_u16().unwrap_or(if url.scheme_str() == Some("http") { 80 } else { 443 });
    let path = url.path_and_query().map_or("/", |p| p.as_str()).to_string();

    let mut headers: Headers = e
        .pointer("/response/headers")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|h| Some((h.get("name")?.as_str()?.to_string(), h.get("value")?.as_str()?.to_string())))
                .filter(|(k, _)| !DROPPED_HEADERS.contains(&k.to_ascii_lowercase().as_str()))
                .collect()
        })
        .unwrap_or_default();
    if headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("content-type") && v.starts_with("application/grpc")) {
        return None;
    }
    let mime = e.pointer("/response/content/mimeType").and_then(Value::as_str).filter(|m| !m.is_empty());
    if let Some(mime) = mime
        && !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        headers.push(("content-type".into(), mime.to_string()));
    }

    let body = e.pointer("/response/content/text").and_then(Value::as_str).map_or_else(Vec::new, |text| {
        if e.pointer("/response/content/encoding").and_then(Value::as_str) == Some("base64") {
            base64::engine::general_purpose::STANDARD.decode(text).unwrap_or_default()
        } else {
            text.as_bytes().to_vec()
        }
    });
    let request_body = e.pointer("/request/postData/text").and_then(Value::as_str).unwrap_or("").as_bytes().to_vec();
    let total = e.get("time").and_then(Value::as_f64).unwrap_or(0.0).max(0.0) as u64;
    let ttfb = e.pointer("/timings/wait").and_then(Value::as_f64).unwrap_or(0.0).max(0.0) as u64;
    Some(Recorded {
        method,
        host,
        port,
        path,
        request_body,
        status,
        headers,
        body,
        ttfb_ms: ttfb.min(total),
        total_ms: total,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn entry(method: &str, url: &str, status: u16, body: &str) -> Value {
        json!({
            "startedDateTime": "2026-11-07T12:00:00.000Z", "time": 120,
            "request": { "method": method, "url": url },
            "response": {
                "status": status,
                "headers": [
                    { "name": "Content-Type", "value": "application/json" },
                    { "name": "Content-Encoding", "value": "gzip" },
                    { "name": "Content-Length", "value": "99" },
                    { "name": "Alt-Svc", "value": "h3=\":443\"" },
                    { "name": "X-Kept", "value": "yes" }
                ],
                "content": { "mimeType": "application/json", "text": body }
            },
            "timings": { "wait": 40, "receive": 80 }
        })
    }

    fn set(entries: Vec<Value>) -> ReplaySet {
        ReplaySet::from_har(&json!({ "log": { "entries": entries } }), "test.har").unwrap()
    }

    #[test]
    fn a_recorded_request_gets_its_recorded_answer() {
        let s = set(vec![entry("GET", "https://api.example.com/items?page=1&sort=asc", 200, r#"{"n":1}"#)]);
        let hit = s.lookup("get", "API.example.com", "/items?sort=asc&page=1", b"").unwrap();
        assert_eq!((hit.status, hit.body.as_slice()), (200, &br#"{"n":1}"#[..]));
        assert_eq!((hit.ttfb_ms, hit.total_ms), (40, 120));
        assert_eq!(s.stats().hits, 1);
    }

    #[test]
    fn framing_headers_are_dropped_and_the_rest_kept() {
        let s = set(vec![entry("GET", "https://a.test/x", 200, "{}")]);
        let names: Vec<&str> = s.entries[0].headers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, vec!["Content-Type", "X-Kept"]);
    }

    #[test]
    fn what_was_not_recorded_is_a_miss_that_is_remembered_once() {
        let s = set(vec![entry("GET", "https://a.test/x", 200, "{}")]);
        assert!(s.lookup("GET", "a.test", "/y", b"").is_none());
        assert!(s.lookup("GET", "a.test", "/y", b"").is_none());
        assert!(s.lookup("POST", "a.test", "/x", b"").is_none(), "another method is another request");
        assert!(s.lookup("GET", "a.test", "/x?extra=1", b"").is_none(), "another query is another request");
        let stats = s.stats();
        assert_eq!((stats.hits, stats.misses), (0, 4));
        assert_eq!(stats.missed, vec!["GET a.test/y", "POST a.test/x", "GET a.test/x?extra=1"]);
    }

    #[test]
    fn a_request_recorded_twice_gets_both_answers_then_the_last_again() {
        let s =
            set(vec![entry("GET", "https://a.test/count", 200, "1"), entry("GET", "https://a.test/count", 200, "2")]);
        let bodies: Vec<_> = (0..4).map(|_| s.lookup("GET", "a.test", "/count", b"").unwrap().body.clone()).collect();
        assert_eq!(bodies, vec![b"1".to_vec(), b"2".to_vec(), b"2".to_vec(), b"2".to_vec()]);
    }

    #[test]
    fn the_request_body_picks_between_recordings() {
        let mut a = entry("POST", "https://a.test/q", 200, "alpha");
        a["request"]["postData"] = json!({ "text": "{\"id\":1}" });
        let mut b = entry("POST", "https://a.test/q", 200, "beta");
        b["request"]["postData"] = json!({ "text": "{\"id\":2}" });
        let s = set(vec![a, b]);
        assert_eq!(s.lookup("POST", "a.test", "/q", b"{\"id\":2}").unwrap().body, b"beta");
        assert_eq!(s.lookup("POST", "a.test", "/q", b"{\"id\":1}").unwrap().body, b"alpha");
        // A body that matches nothing falls back to the order they were recorded in.
        assert_eq!(s.lookup("POST", "a.test", "/q", b"other").unwrap().body, b"alpha");
    }

    #[test]
    fn binary_bodies_are_decoded_from_base64() {
        let mut e = entry("GET", "https://a.test/img", 200, "AAEC/w==");
        e["response"]["content"]["encoding"] = json!("base64");
        let s = set(vec![e]);
        assert_eq!(s.lookup("GET", "a.test", "/img", b"").unwrap().body, vec![0, 1, 2, 255]);
    }

    #[test]
    fn failed_requests_websockets_and_grpc_are_left_out() {
        let failed = entry("GET", "https://a.test/down", 0, "");
        let mut ws = entry("GET", "wss://a.test/live", 101, "");
        ws["_resourceType"] = json!("websocket");
        let mut grpc = entry("POST", "https://a.test/pkg.Svc/Call", 200, "");
        grpc["response"]["headers"] = json!([{ "name": "content-type", "value": "application/grpc" }]);
        let ok = entry("GET", "https://a.test/ok", 404, "");
        let s = set(vec![failed, ws, grpc, ok]);
        assert_eq!(s.len(), 1);
        assert_eq!(s.lookup("GET", "a.test", "/ok", b"").unwrap().status, 404);
    }

    #[test]
    fn a_missing_content_type_comes_from_the_mime_type() {
        let mut e = entry("GET", "https://a.test/x", 200, "hi");
        e["response"]["headers"] = json!([]);
        e["response"]["content"]["mimeType"] = json!("text/plain");
        let s = set(vec![e]);
        assert_eq!(s.entries[0].headers, vec![("content-type".to_string(), "text/plain".to_string())]);
    }

    #[test]
    fn ports_default_by_scheme_and_a_bad_document_is_explained() {
        let s = set(vec![entry("GET", "http://a.test/x", 200, ""), entry("GET", "https://b.test:8443/x", 200, "")]);
        assert_eq!((s.entries[0].port, s.entries[1].port), (80, 8443));
        assert!(ReplaySet::from_har(&json!({"nope": 1}), "x").unwrap_err().contains("not a HAR"));
    }

    #[test]
    fn strict_is_the_default_and_passthrough_lifts_it() {
        let s = set(vec![]);
        assert!(s.is_strict() && s.is_empty());
        assert!(!s.passthrough(true).is_strict());
    }
}
