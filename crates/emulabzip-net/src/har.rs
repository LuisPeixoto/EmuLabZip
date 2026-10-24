//! HAR 1.2 export, for opening a capture in a browser's developer tools or another debugger.

use base64::Engine;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::decode::decode;
use crate::flow::{Flow, FlowKind, Headers};
use crate::ws::{Direction, Kind, WsMessage};

fn iso(ms: u64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000)
        .ok()
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn headers(h: &Headers) -> Value {
    h.iter().map(|(k, v)| json!({ "name": k, "value": v })).collect()
}

fn query(path: &str) -> Value {
    let Some((_, q)) = path.split_once('?') else { return json!([]) };
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            json!({ "name": k, "value": v })
        })
        .collect()
}

/// A body as HAR wants it: text when it is valid UTF-8, base64 otherwise.
fn content(body: &[u8], encoding: Option<&str>, mime: &str, wire_size: u64) -> Value {
    let decoded = decode(encoding, body).unwrap_or_else(|_| body.to_vec());
    let mut v = json!({ "size": decoded.len(), "mimeType": mime });
    if wire_size > 0 && wire_size as usize != decoded.len() {
        v["compression"] = json!(decoded.len() as i64 - wire_size as i64);
    }
    match std::str::from_utf8(&decoded) {
        Ok(text) => v["text"] = json!(text),
        Err(_) => {
            v["text"] = json!(base64::engine::general_purpose::STANDARD.encode(&decoded));
            v["encoding"] = json!("base64");
        }
    }
    v
}

fn entry(f: &Flow) -> Value {
    let req_encoding = Flow::header(&f.request_headers, "content-encoding");
    let res_encoding = Flow::header(&f.response_headers, "content-encoding");
    let res_mime = f.content_type().unwrap_or_default();
    let ttfb = f.timing.ttfb_ms.unwrap_or(0) as i64;
    let total = f.timing.total_ms.unwrap_or(0) as i64;
    let mut request = json!({
        "method": f.method,
        "url": f.url,
        "httpVersion": f.http_version,
        "cookies": [],
        "headers": headers(&f.request_headers),
        "queryString": query(&f.path),
        "headersSize": -1,
        "bodySize": f.request_size,
    });
    if !f.request_body.is_empty() {
        let mime = Flow::header(&f.request_headers, "content-type").unwrap_or("application/octet-stream");
        let c = content(&f.request_body, req_encoding, mime, f.request_size);
        request["postData"] = json!({ "mimeType": mime, "text": c["text"] });
    }
    let mut entry = json!({
        "startedDateTime": iso(f.started_ms),
        "time": total,
        "request": request,
        "response": {
            "status": f.status.unwrap_or(0),
            "statusText": f.status.and_then(|s| http::StatusCode::from_u16(s).ok()).and_then(|s| s.canonical_reason()).unwrap_or(""),
            "httpVersion": f.http_version,
            "cookies": [],
            "headers": headers(&f.response_headers),
            "content": content(&f.response_body, res_encoding, &res_mime, f.response_size),
            "redirectURL": Flow::header(&f.response_headers, "location").unwrap_or(""),
            "headersSize": -1,
            "bodySize": f.response_size,
        },
        "cache": {},
        "timings": { "send": 0, "wait": ttfb, "receive": (total - ttfb).max(0) },
        "comment": f.error.clone().unwrap_or_default(),
    });
    if f.kind == FlowKind::WebSocket {
        // The same fields Chrome's HAR export uses for WebSockets.
        entry["_resourceType"] = json!("websocket");
        entry["_webSocketMessages"] = f.messages.iter().map(|m| message(f, m)).collect();
    }
    entry
}

fn message(f: &Flow, m: &WsMessage) -> Value {
    let (opcode, data) = match (m.kind, &m.text, &m.base64) {
        (Kind::Text, Some(text), _) => (1, text.clone()),
        (kind, _, base64) => (
            match kind {
                Kind::Text => 1,
                Kind::Binary => 2,
                Kind::Close => 8,
                Kind::Ping => 9,
                Kind::Pong => 10,
            },
            base64.clone().unwrap_or_default(),
        ),
    };
    json!({
        "type": if m.direction == Direction::Up { "send" } else { "receive" },
        "time": (f.started_ms + m.t_ms) as f64 / 1000.0,
        "opcode": opcode,
        "data": data,
    })
}

/// Builds a HAR document from the HTTP and WebSocket flows. Tunnels have no requests to
/// describe and are left out.
pub fn to_har(flows: &[Flow]) -> Value {
    json!({
        "log": {
            "version": "1.2",
            "creator": { "name": "EmuLabZip", "version": env!("CARGO_PKG_VERSION") },
            "entries": flows.iter().filter(|f| matches!(f.kind, FlowKind::Http | FlowKind::WebSocket)).map(entry).collect::<Vec<_>>(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowState;

    fn sample() -> Flow {
        let mut f = Flow::new(FlowKind::Http, "POST", "https", "api.example.com", 443, "/v1/items?page=2&q=a%20b");
        f.http_version = "HTTP/2.0".into();
        f.status = Some(201);
        f.state = FlowState::Completed;
        f.request_headers = vec![("Content-Type".into(), "application/json".into())];
        f.request_body = br#"{"name":"x"}"#.to_vec();
        f.request_size = f.request_body.len() as u64;
        f.response_headers = vec![("Content-Type".into(), "application/json".into())];
        f.response_body = br#"{"id":1}"#.to_vec();
        f.response_size = f.response_body.len() as u64;
        f.started_ms = 1_792_773_580_000;
        f.timing.ttfb_ms = Some(40);
        f.timing.total_ms = Some(55);
        f
    }

    #[test]
    fn describes_a_request_and_response() {
        let har = to_har(&[sample()]);
        let e = &har["log"]["entries"][0];
        assert_eq!(har["log"]["version"], "1.2");
        assert_eq!(e["startedDateTime"], "2026-10-23T16:39:40Z");
        assert_eq!(e["request"]["method"], "POST");
        assert_eq!(e["request"]["queryString"][0]["name"], "page");
        assert_eq!(e["request"]["postData"]["text"], r#"{"name":"x"}"#);
        assert_eq!(e["response"]["status"], 201);
        assert_eq!(e["response"]["statusText"], "Created");
        assert_eq!(e["response"]["content"]["text"], r#"{"id":1}"#);
        assert_eq!(e["timings"]["wait"], 40);
        assert_eq!(e["timings"]["receive"], 15);
    }

    #[test]
    fn encodes_binary_bodies_as_base64() {
        let mut f = sample();
        f.response_body = vec![0xff, 0xfe, 0x00];
        f.response_size = 3;
        let e = &to_har(&[f])["log"]["entries"][0];
        assert_eq!(e["response"]["content"]["encoding"], "base64");
        assert_eq!(e["response"]["content"]["text"], "//4A");
    }

    #[test]
    fn leaves_tunnels_out() {
        let mut t = sample();
        t.kind = FlowKind::Tunnel;
        assert!(to_har(&[t])["log"]["entries"].as_array().unwrap().is_empty());
    }
}
