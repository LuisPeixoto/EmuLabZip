//! The requests a page makes, as the browser's DevTools protocol reports them, shown as flows.
//!
//! A WebView or Chrome tells a debugger about every request with `Network.requestWillBeSent`, `responseReceived`,
//! `loadingFinished` and `loadingFailed`. [`CdpFlows`] turns those events into flows in a [`FlowStore`], marked
//! with the source `webview`, so they appear in the Network view next to what the proxy saw. Unlike the proxy this
//! also sees traffic that is not decrypted (certificate pinning does not matter to the page), but it cannot change
//! a request or hold it at a breakpoint. It only reads JSON values, so it needs no connection of its own.

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use hyper::Uri;
use serde_json::Value;

use crate::flow::{BODY_LIMIT, Flow, FlowId, FlowKind, FlowState, Headers};
use crate::store::FlowStore;

/// The source of every flow this makes.
pub const SOURCE: &str = "webview";
/// The most requests followed at once. A page that never finishes them cannot grow this without a limit.
const MAX_LIVE: usize = 2_000;

struct Live {
    id: FlowId,
    /// The protocol's own clock, in seconds, when the request started.
    started: f64,
}

/// A response whose body should be read from the page and given to [`CdpFlows::set_body`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyNeeded {
    pub request_id: String,
    pub flow: FlowId,
}

/// Follows the network events of one page.
pub struct CdpFlows {
    store: Arc<FlowStore>,
    live: HashMap<String, Live>,
}

impl CdpFlows {
    pub fn new(store: Arc<FlowStore>) -> Self {
        CdpFlows { store, live: HashMap::new() }
    }

    /// Handles one protocol event. When a response has finished, says that its body can now be read.
    pub fn on_event(&mut self, event: &Value) -> Option<BodyNeeded> {
        let method = event.get("method")?.as_str()?;
        let params = event.get("params")?;
        let request_id = params.get("requestId")?.as_str()?.to_string();
        match method {
            "Network.requestWillBeSent" => {
                self.request(&request_id, params);
                None
            }
            "Network.responseReceived" => {
                self.response(&request_id, params);
                None
            }
            "Network.dataReceived" => {
                if let Some(live) = self.live.get(&request_id) {
                    let n = params.get("dataLength").and_then(Value::as_u64).unwrap_or(0);
                    self.store.update(live.id, |f| f.response_size += n);
                }
                None
            }
            "Network.loadingFinished" => self.finished(&request_id, params),
            "Network.loadingFailed" => {
                self.failed(&request_id, params);
                None
            }
            _ => None,
        }
    }

    /// Stores a response body, which the protocol sends as text or base64.
    pub fn set_body(&self, flow: FlowId, body: &[u8], base64: bool) {
        let bytes = if base64 {
            base64::engine::general_purpose::STANDARD.decode(body).unwrap_or_else(|_| body.to_vec())
        } else {
            body.to_vec()
        };
        self.store.update(flow, |f| {
            f.response_size = f.response_size.max(bytes.len() as u64);
            f.response_truncated = bytes.len() > BODY_LIMIT;
            f.response_body = bytes.into_iter().take(BODY_LIMIT).collect();
        });
    }

    fn request(&mut self, request_id: &str, params: &Value) {
        let Some(request) = params.get("request") else { return };
        // A redirect reuses the request id: the earlier hop ends with the redirect's answer.
        if let Some(previous) = params.get("redirectResponse")
            && let Some(live) = self.live.remove(request_id)
        {
            let ended = params.get("timestamp").and_then(Value::as_f64).unwrap_or(live.started);
            self.store.update(live.id, |f| {
                apply_response(f, previous);
                f.state = FlowState::Completed;
                f.timing.total_ms = Some(millis(live.started, ended));
            });
        }
        let url = request.get("url").and_then(Value::as_str).unwrap_or("");
        let Some((scheme, host, port, path)) = split(url) else { return };
        let method = request.get("method").and_then(Value::as_str).unwrap_or("GET");
        let mut flow = Flow::new(FlowKind::Http, method, &scheme, &host, port, &path);
        flow.source = Some(SOURCE.into());
        flow.request_headers = headers(request.get("headers"));
        if let Some(body) = request.get("postData").and_then(Value::as_str) {
            flow.request_size = body.len() as u64;
            flow.request_truncated = body.len() > BODY_LIMIT;
            flow.request_body = body.as_bytes().iter().copied().take(BODY_LIMIT).collect();
        }
        if self.live.len() >= MAX_LIVE {
            // Drop the oldest unfinished request rather than grow without a limit.
            if let Some(oldest) = self.live.iter().min_by_key(|(_, l)| l.id).map(|(k, _)| k.clone()) {
                self.live.remove(&oldest);
            }
        }
        let started = params.get("timestamp").and_then(Value::as_f64).unwrap_or(0.0);
        let id = self.store.begin(flow);
        self.live.insert(request_id.to_string(), Live { id, started });
    }

    fn response(&mut self, request_id: &str, params: &Value) {
        let (Some(live), Some(response)) = (self.live.get(request_id), params.get("response")) else { return };
        let started = live.started;
        let at = params.get("timestamp").and_then(Value::as_f64).unwrap_or(started);
        self.store.update(live.id, |f| {
            apply_response(f, response);
            // `receiveHeadersEnd` is the best measure of the time to the first byte; the clock difference will do.
            let ttfb = response
                .pointer("/timing/receiveHeadersEnd")
                .and_then(Value::as_f64)
                .filter(|v| *v >= 0.0)
                .unwrap_or_else(|| millis(started, at) as f64);
            f.timing.ttfb_ms = Some(ttfb as u64);
        });
    }

    fn finished(&mut self, request_id: &str, params: &Value) -> Option<BodyNeeded> {
        let live = self.live.remove(request_id)?;
        let at = params.get("timestamp").and_then(Value::as_f64).unwrap_or(live.started);
        let sent = params.get("encodedDataLength").and_then(Value::as_f64).unwrap_or(0.0).max(0.0) as u64;
        let mut has_response = false;
        self.store.update(live.id, |f| {
            f.state = FlowState::Completed;
            f.timing.total_ms = Some(millis(live.started, at));
            f.response_size = f.response_size.max(sent);
            has_response = f.status.is_some();
        });
        has_response.then(|| BodyNeeded { request_id: request_id.to_string(), flow: live.id })
    }

    fn failed(&mut self, request_id: &str, params: &Value) {
        let Some(live) = self.live.remove(request_id) else { return };
        let canceled = params.get("canceled").and_then(Value::as_bool).unwrap_or(false);
        let text = params.get("errorText").and_then(Value::as_str).unwrap_or("The request failed.");
        let blocked = params.get("blockedReason").and_then(Value::as_str);
        let error = match (canceled, blocked) {
            (_, Some(reason)) => format!("{text} (blocked: {reason})"),
            (true, None) => format!("{text} (canceled)"),
            (false, None) => text.to_string(),
        };
        self.store.update(live.id, |f| {
            f.state = FlowState::Failed;
            f.error = Some(error);
        });
    }
}

fn apply_response(f: &mut Flow, response: &Value) {
    f.status = response.get("status").and_then(Value::as_u64).map(|s| s as u16);
    f.response_headers = headers(response.get("headers"));
    let protocol = response.get("protocol").and_then(Value::as_str).unwrap_or("");
    f.http_version = match protocol {
        "h2" => "HTTP/2.0",
        "h3" | "h3-29" => "HTTP/3.0",
        "http/1.0" => "HTTP/1.0",
        _ => "HTTP/1.1",
    }
    .into();
    f.server_protocol = match protocol {
        "h2" => "h2",
        "h3" | "h3-29" => "h3",
        _ => "h1",
    }
    .into();
}

/// The milliseconds between two readings of the protocol's clock, which counts seconds.
fn millis(from: f64, to: f64) -> u64 {
    ((to - from).max(0.0) * 1000.0).round() as u64
}

/// Headers as an object; a header sent twice is one value with the lines joined by a newline.
fn headers(value: Option<&Value>) -> Headers {
    let Some(object) = value.and_then(Value::as_object) else { return Vec::new() };
    object
        .iter()
        .flat_map(|(name, v)| {
            let text = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
            text.split('\n').map(|line| (name.clone(), line.to_string())).collect::<Vec<_>>()
        })
        .collect()
}

/// Scheme, host, port and path with query of an address a page can request. `data:`, `blob:` and the like are
/// not network traffic and give `None`.
fn split(url: &str) -> Option<(String, String, u16, String)> {
    let uri: Uri = url.parse().ok()?;
    let scheme = uri.scheme_str()?.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") {
        return None;
    }
    let host = uri.host()?.to_string();
    let port = uri.port_u16().unwrap_or(if scheme == "https" || scheme == "wss" { 443 } else { 80 });
    let path = uri.path_and_query().map_or("/", |p| p.as_str()).to_string();
    Some((scheme, host, port, path))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn will(id: &str, url: &str, t: f64) -> Value {
        json!({ "method": "Network.requestWillBeSent", "params": { "requestId": id, "timestamp": t,
            "request": { "url": url, "method": "POST", "headers": { "Content-Type": "application/json" },
                         "postData": "{\"a\":1}" } } })
    }

    fn store() -> Arc<FlowStore> {
        FlowStore::new(100)
    }

    #[test]
    fn a_request_and_its_answer_become_a_flow() {
        let store = store();
        let mut cdp = CdpFlows::new(store.clone());
        assert!(cdp.on_event(&will("1", "https://shop.example/api/cart?x=1", 10.0)).is_none());
        cdp.on_event(&json!({ "method": "Network.responseReceived", "params": { "requestId": "1", "timestamp": 10.05,
            "response": { "status": 201, "protocol": "h2", "headers": { "Content-Type": "application/json",
            "Set-Cookie": "a=1\nb=2" }, "timing": { "receiveHeadersEnd": 42.0 } } } }));
        let need = cdp
            .on_event(&json!({ "method": "Network.loadingFinished", "params": { "requestId": "1", "timestamp": 10.1,
                "encodedDataLength": 120 } }))
            .expect("a finished response asks for its body");
        cdp.set_body(need.flow, b"{\"ok\":true}", false);
        let f = store.get(need.flow).unwrap();
        assert_eq!((f.method.as_str(), f.host.as_str(), f.port), ("POST", "shop.example", 443));
        assert_eq!(f.path, "/api/cart?x=1");
        assert_eq!(f.status, Some(201));
        assert_eq!(f.state, FlowState::Completed);
        assert_eq!(f.source.as_deref(), Some("webview"));
        assert_eq!((f.http_version.as_str(), f.server_protocol.as_str()), ("HTTP/2.0", "h2"));
        assert_eq!(f.request_body, b"{\"a\":1}");
        assert_eq!(f.response_body, b"{\"ok\":true}");
        assert_eq!(f.timing.ttfb_ms, Some(42));
        assert_eq!(f.timing.total_ms, Some(100));
        assert_eq!(f.response_headers.iter().filter(|(k, _)| k == "Set-Cookie").count(), 2);
    }

    #[test]
    fn a_failed_request_says_why() {
        let store = store();
        let mut cdp = CdpFlows::new(store.clone());
        cdp.on_event(&will("1", "http://10.0.2.2:3000/x", 1.0));
        cdp.on_event(&json!({ "method": "Network.loadingFailed", "params": { "requestId": "1", "timestamp": 1.2,
            "errorText": "net::ERR_CONNECTION_REFUSED", "canceled": false } }));
        let f = store.get(1).unwrap();
        assert_eq!(f.state, FlowState::Failed);
        assert!(f.error.as_deref().unwrap().contains("CONNECTION_REFUSED"));
        assert_eq!(f.port, 3000);
    }

    #[test]
    fn a_redirect_ends_one_flow_and_starts_the_next() {
        let store = store();
        let mut cdp = CdpFlows::new(store.clone());
        cdp.on_event(&will("1", "http://shop.example/a", 1.0));
        cdp.on_event(&json!({ "method": "Network.requestWillBeSent", "params": { "requestId": "1", "timestamp": 1.1,
            "redirectResponse": { "status": 301, "headers": { "Location": "https://shop.example/b" } },
            "request": { "url": "https://shop.example/b", "method": "GET", "headers": {} } } }));
        let first = store.get(1).unwrap();
        assert_eq!((first.status, first.state), (Some(301), FlowState::Completed));
        assert_eq!(store.get(2).unwrap().path, "/b");
    }

    #[test]
    fn things_that_are_not_network_traffic_are_left_out() {
        let store = store();
        let mut cdp = CdpFlows::new(store.clone());
        cdp.on_event(&will("1", "data:text/plain;base64,aGk=", 1.0));
        cdp.on_event(&will("2", "blob:https://shop.example/abc", 1.0));
        cdp.on_event(&will("3", "chrome-extension://x/y.js", 1.0));
        assert!(store.get(1).is_none());
        // An answer for a request that was never seen is ignored.
        assert!(
            cdp.on_event(
                &json!({ "method": "Network.loadingFinished", "params": { "requestId": "9", "timestamp": 2.0 } })
            )
            .is_none()
        );
        assert!(cdp.on_event(&json!({ "method": "Page.loadEventFired", "params": { "requestId": "x" } })).is_none());
    }

    #[test]
    fn a_base64_body_is_decoded_and_a_huge_one_is_cut() {
        let store = store();
        let mut cdp = CdpFlows::new(store.clone());
        cdp.on_event(&will("1", "https://a.example/img", 1.0));
        cdp.set_body(1, b"aGVsbG8=", true);
        assert_eq!(store.get(1).unwrap().response_body, b"hello");
        let big = vec![b'x'; BODY_LIMIT + 5];
        cdp.set_body(1, &big, false);
        let f = store.get(1).unwrap();
        assert_eq!(f.response_body.len(), BODY_LIMIT);
        assert!(f.response_truncated);
    }

    #[test]
    fn no_more_than_the_limit_is_followed_at_once() {
        let store = FlowStore::new(10_000);
        let mut cdp = CdpFlows::new(store);
        for i in 0..MAX_LIVE + 50 {
            cdp.on_event(&will(&i.to_string(), "https://a.example/x", 1.0));
        }
        assert_eq!(cdp.live.len(), MAX_LIVE);
    }
}
