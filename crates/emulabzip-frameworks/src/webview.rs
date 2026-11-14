//! Web pages inside an Android app (a WebView) or in Chrome, through the Chrome DevTools protocol.
//!
//! A debuggable WebView opens an abstract socket, `webview_devtools_remote_<pid>`, that serves the same HTTP and
//! WebSocket interface as Chrome's remote debugging port: `GET /json/list` lists the pages, and each page has a
//! WebSocket that speaks the protocol. The connection goes through ADB's `localabstract:` service, so nothing is
//! forwarded on this computer.
//!
//! The raw protocol events are handed on as they are: [`parse_event`] reads console messages and errors from them,
//! and the network events can be turned into flows by `emulabzip_net::cdp`.

use std::time::Duration;

use emulabzip_core::{DevtoolsSocket, Error, Lab, Result, Target};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::react_native::{http_get, parse_console};
use crate::rpc::Rpc;

/// The longest expression that is evaluated, and the longest value that is handed back.
pub const MAX_EXPRESSION: usize = 10_000;
pub const MAX_VALUE: usize = 64 * 1024;
/// The most a page's response body is read for a flow.
pub const MAX_BODY: usize = 10 * 1024 * 1024;

/// A page that can be debugged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub title: String,
    pub url: String,
    /// `page`, `iframe`, `service_worker`… as the browser says.
    pub kind: String,
    /// Where the protocol listens: `/devtools/page/<id>`.
    #[serde(skip)]
    pub ws_path: String,
}

/// Reads the answer of `GET /json/list`.
pub fn parse_pages(list: &Value) -> Vec<Page> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|t| {
                    let text = |key: &str| t.get(key).and_then(Value::as_str).map(str::to_string);
                    let url = t.get("webSocketDebuggerUrl")?.as_str()?;
                    let path =
                        url.split_once("://").map_or(url, |(_, rest)| rest.find('/').map_or("/", |i| &rest[i..]));
                    Some(Page {
                        id: text("id")?,
                        title: text("title").unwrap_or_default(),
                        url: text("url").unwrap_or_default(),
                        kind: text("type").unwrap_or_else(|| "page".into()),
                        ws_path: path.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The pages behind a DevTools socket.
pub async fn pages(lab: &Lab, target: &Target, socket: &str) -> Result<Vec<Page>> {
    let mut stream = lab.connect_socket(target, socket).await?;
    let body = http_get(&mut stream, "/json/list", "The WebView").await?;
    let value: Value =
        serde_json::from_slice(&body).map_err(|_| Error::failed("The WebView did not answer with a list of pages."))?;
    Ok(parse_pages(&value))
}

/// The page to look at: the one asked for by id, a prefix of its id or part of its URL, else the only real page.
pub fn pick<'a>(pages: &'a [Page], wanted: Option<&str>) -> Option<&'a Page> {
    match wanted.filter(|w| !w.is_empty()) {
        Some(w) => pages
            .iter()
            .find(|p| p.id == w)
            .or_else(|| pages.iter().find(|p| p.id.starts_with(w)))
            .or_else(|| pages.iter().find(|p| p.url.contains(w))),
        None => {
            let real: Vec<&Page> = pages.iter().filter(|p| p.kind == "page" || p.kind == "webview").collect();
            if real.len() == 1 { real.first().copied() } else { None }
        }
    }
}

/// The socket to use: the one named, the one of `app`, or the only one there is.
pub fn pick_socket<'a>(
    sockets: &'a [DevtoolsSocket],
    app: Option<&str>,
    socket: Option<&str>,
) -> Result<&'a DevtoolsSocket> {
    if sockets.is_empty() {
        return Err(Error::failed(
            "No WebView can be inspected on this device. A WebView needs `WebView.setWebContentsDebuggingEnabled(true)` in a debuggable app.",
        ));
    }
    let found = match (socket, app) {
        (Some(name), _) => sockets.iter().find(|s| s.socket == name),
        (None, Some(app)) => sockets.iter().find(|s| s.app.as_deref() == Some(app)),
        (None, None) if sockets.len() == 1 => sockets.first(),
        (None, None) => None,
    };
    found.ok_or_else(|| {
        let names: Vec<String> =
            sockets.iter().map(|s| format!("{} ({})", s.socket, s.app.as_deref().unwrap_or("unknown app"))).collect();
        Error::failed(format!("Say which one with a socket or an app. Found: {}", names.join(", ")))
    })
}

/// Finds the socket and the page and connects to it. Each can be named; what is left out is taken when there is
/// only one choice.
pub async fn connect(
    lab: &Lab,
    target: &Target,
    app: Option<&str>,
    socket: Option<&str>,
    page: Option<&str>,
) -> Result<(DevtoolsSocket, Page, WebView)> {
    let sockets = lab.devtools_sockets(target).await?;
    let socket = pick_socket(&sockets, app, socket)?.clone();
    let found = pages(lab, target, &socket.socket).await?;
    let Some(page) = pick(&found, page).cloned() else {
        let list: Vec<String> = found.iter().map(|p| format!("{} {}", &p.id[..p.id.len().min(8)], p.url)).collect();
        return Err(Error::failed(format!(
            "Say which page (an id, or part of its address). Found: {}",
            list.join(", ")
        )));
    };
    let view = WebView::attach(lab, target, &socket.socket, &page).await?;
    Ok((socket, page, view))
}

/// Something a page wrote to its console, or an error it did not catch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLine {
    /// `log`, `info`, `warn`, `error`, `debug`.
    pub level: String,
    pub text: String,
    /// `https://example.com/app.js:12`, when the page says where it came from.
    pub source: Option<String>,
}

fn level(raw: &str) -> String {
    match raw {
        "warning" => "warn",
        "verbose" => "debug",
        "assert" | "exception" => "error",
        other => other,
    }
    .to_string()
}

fn source(url: &str, line: Option<i64>) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    Some(match line {
        // The protocol counts lines from 0.
        Some(l) if l >= 0 => format!("{url}:{}", l + 1),
        _ => url.to_string(),
    })
}

/// Reads a console message, a log entry or an uncaught error from a protocol event. Other events give `None`.
pub fn parse_event(event: &Value) -> Option<WebLine> {
    let params = event.get("params")?;
    match event.get("method")?.as_str()? {
        "Runtime.consoleAPICalled" => {
            let line = parse_console(event)?;
            let frame = params.pointer("/stackTrace/callFrames/0");
            let src = frame.and_then(|f| source(f.get("url")?.as_str()?, f.get("lineNumber").and_then(Value::as_i64)));
            Some(WebLine { level: level(&line.level), text: line.text, source: src })
        }
        "Log.entryAdded" => {
            let entry = params.get("entry")?;
            let url = entry.get("url").and_then(Value::as_str).unwrap_or("");
            Some(WebLine {
                level: level(entry.get("level")?.as_str()?),
                text: entry.get("text")?.as_str()?.to_string(),
                source: source(url, entry.get("lineNumber").and_then(Value::as_i64)),
            })
        }
        "Runtime.exceptionThrown" => {
            let details = params.get("exceptionDetails")?;
            let text = details
                .pointer("/exception/description")
                .and_then(Value::as_str)
                .or_else(|| details.get("text").and_then(Value::as_str))?
                .to_string();
            let url = details.get("url").and_then(Value::as_str).unwrap_or("");
            Some(WebLine {
                level: "error".into(),
                text,
                source: source(url, details.get("lineNumber").and_then(Value::as_i64)),
            })
        }
        _ => None,
    }
}

/// What evaluating an expression gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluated {
    /// The value as text: strings as they are, everything else as JSON or the browser's own description.
    pub value: String,
    /// `string`, `number`, `object`, `undefined`…
    pub kind: String,
    /// The expression threw.
    pub error: bool,
}

/// Reads the answer of `Runtime.evaluate`.
pub fn parse_evaluated(answer: &Value) -> Evaluated {
    let cut = |mut s: String| {
        if s.len() > MAX_VALUE {
            let mut end = MAX_VALUE;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            s.truncate(end);
            s.push('…');
        }
        s
    };
    if let Some(details) = answer.get("exceptionDetails") {
        let text = details
            .pointer("/exception/description")
            .and_then(Value::as_str)
            .or_else(|| details.get("text").and_then(Value::as_str))
            .unwrap_or("The expression threw.");
        return Evaluated { value: cut(text.to_string()), kind: "error".into(), error: true };
    }
    let result = answer.get("result").cloned().unwrap_or(Value::Null);
    let kind = result.get("type").and_then(Value::as_str).unwrap_or("undefined").to_string();
    let value = match result.get("value") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => match kind.as_str() {
            "undefined" => "undefined".to_string(),
            _ => result.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
        },
    };
    Evaluated { value: cut(value), kind, error: false }
}

/// A connection to one page.
pub struct WebView {
    rpc: Rpc,
}

impl WebView {
    /// Connects to `page` through the DevTools `socket` and starts the domains that report what the page does.
    pub async fn attach(lab: &Lab, target: &Target, socket: &str, page: &Page) -> Result<WebView> {
        let stream = lab.connect_socket(target, socket).await?;
        let rpc = Rpc::connect_chrome(stream, &format!("ws://localhost{}", page.ws_path)).await?;
        Ok(WebView { rpc })
    }

    /// Starts the console, log and network reports.
    pub async fn start(&self, network: bool) -> Result<()> {
        self.rpc.call("Runtime.enable", json!({})).await?;
        self.rpc.call("Log.enable", json!({})).await?;
        if network {
            self.rpc.call("Network.enable", json!({})).await?;
        }
        Ok(())
    }

    /// The protocol's events, as they came.
    pub fn events(&self) -> broadcast::Receiver<Value> {
        self.rpc.events()
    }

    /// Runs `expression` in the page.
    pub async fn evaluate(&self, expression: &str) -> Result<Evaluated> {
        if expression.trim().is_empty() || expression.len() > MAX_EXPRESSION {
            return Err(Error::invalid("expression", format!("{} characters", expression.len())));
        }
        let answer = self
            .rpc
            .call(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true, "awaitPromise": true, "userGesture": true,
                        "replMode": true }),
            )
            .await?;
        Ok(parse_evaluated(&answer))
    }

    pub async fn reload(&self) -> Result<()> {
        self.rpc.call("Page.reload", json!({ "ignoreCache": true })).await?;
        Ok(())
    }

    /// The body of a response the network reported, and whether it is base64.
    pub async fn response_body(&self, request_id: &str) -> Result<(Vec<u8>, bool)> {
        let answer = self.rpc.call("Network.getResponseBody", json!({ "requestId": request_id })).await?;
        let base64 = answer.get("base64Encoded").and_then(Value::as_bool).unwrap_or(false);
        let body = answer.get("body").and_then(Value::as_str).unwrap_or("");
        Ok((body.as_bytes().to_vec(), base64))
    }

    /// The console and errors written for `seconds`, up to `max` lines.
    pub async fn console(&self, seconds: u64, max: usize) -> Result<Vec<WebLine>> {
        let mut events = self.events();
        self.start(false).await?;
        let mut lines = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(seconds), async {
            loop {
                match events.recv().await {
                    Ok(event) => {
                        if let Some(line) = parse_event(&event)
                            && lines.len() < max
                        {
                            lines.push(line);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        })
        .await;
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Value {
        json!([
          { "id": "AB12CD", "type": "page", "title": "Shop", "url": "https://shop.example/cart",
            "webSocketDebuggerUrl": "ws://localhost/devtools/page/AB12CD" },
          { "id": "EF34", "type": "service_worker", "title": "", "url": "https://shop.example/sw.js",
            "webSocketDebuggerUrl": "ws://localhost/devtools/page/EF34" },
          { "id": "no-socket", "type": "page" }
        ])
    }

    #[test]
    fn reads_the_pages_a_webview_lists() {
        let pages = parse_pages(&list());
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].ws_path, "/devtools/page/AB12CD");
        assert_eq!((pages[0].title.as_str(), pages[0].kind.as_str()), ("Shop", "page"));
        assert!(parse_pages(&json!({})).is_empty());
    }

    #[test]
    fn picks_a_page_by_id_prefix_or_url_and_the_only_real_one() {
        let pages = parse_pages(&list());
        assert_eq!(pick(&pages, None).unwrap().id, "AB12CD");
        assert_eq!(pick(&pages, Some("EF3")).unwrap().id, "EF34");
        assert_eq!(pick(&pages, Some("/cart")).unwrap().id, "AB12CD");
        assert!(pick(&pages, Some("nothing")).is_none());
        // Nothing asked for is the same as no page named.
        assert_eq!(pick(&pages, Some("")).unwrap().id, "AB12CD");
    }

    fn socket(name: &str, app: Option<&str>) -> DevtoolsSocket {
        DevtoolsSocket { socket: name.into(), kind: "webview".into(), pid: None, app: app.map(str::to_string) }
    }

    #[test]
    fn the_socket_is_found_by_name_app_or_because_it_is_the_only_one() {
        let two =
            [socket("webview_devtools_remote_1", Some("com.a")), socket("webview_devtools_remote_2", Some("com.b"))];
        let named = pick_socket(&two, None, Some("webview_devtools_remote_2")).unwrap();
        assert_eq!(named.app.as_deref(), Some("com.b"));
        assert_eq!(pick_socket(&two, Some("com.a"), None).unwrap().socket, "webview_devtools_remote_1");
        let error = pick_socket(&two, None, None).unwrap_err().to_string();
        assert!(error.contains("com.a") && error.contains("com.b"), "{error}");
        assert!(pick_socket(&two[..1], None, None).is_ok());
        assert!(pick_socket(&[], None, None).unwrap_err().to_string().contains("setWebContentsDebuggingEnabled"));
    }

    #[test]
    fn reads_console_messages_with_where_they_came_from() {
        let event = json!({ "method": "Runtime.consoleAPICalled", "params": { "type": "warning",
            "args": [{ "type": "string", "value": "low stock" }],
            "stackTrace": { "callFrames": [{ "url": "https://shop.example/app.js", "lineNumber": 11 }] } } });
        let line = parse_event(&event).unwrap();
        assert_eq!(line.level, "warn");
        assert_eq!(line.text, "low stock");
        assert_eq!(line.source.as_deref(), Some("https://shop.example/app.js:12"));
    }

    #[test]
    fn reads_log_entries_and_uncaught_errors() {
        let log = json!({ "method": "Log.entryAdded", "params": { "entry": { "level": "error", "source": "network",
            "text": "Failed to load resource: 404", "url": "https://shop.example/x.png" } } });
        let line = parse_event(&log).unwrap();
        assert_eq!((line.level.as_str(), line.source.as_deref()), ("error", Some("https://shop.example/x.png")));
        let thrown = json!({ "method": "Runtime.exceptionThrown", "params": { "exceptionDetails": {
            "text": "Uncaught", "lineNumber": 2, "url": "u.js",
            "exception": { "description": "TypeError: x is undefined\n    at f (u.js:3:1)" } } } });
        let line = parse_event(&thrown).unwrap();
        assert_eq!(line.level, "error");
        assert!(line.text.starts_with("TypeError"));
        assert_eq!(line.source.as_deref(), Some("u.js:3"));
        assert!(parse_event(&json!({ "method": "Network.requestWillBeSent", "params": {} })).is_none());
    }

    #[test]
    fn evaluated_values_are_text() {
        let s = parse_evaluated(&json!({ "result": { "type": "string", "value": "Shop" } }));
        assert_eq!((s.value.as_str(), s.kind.as_str(), s.error), ("Shop", "string", false));
        let n = parse_evaluated(&json!({ "result": { "type": "number", "value": 42 } }));
        assert_eq!(n.value, "42");
        let o = parse_evaluated(&json!({ "result": { "type": "object", "value": { "a": 1 } } }));
        assert_eq!(o.value, r#"{"a":1}"#);
        let u = parse_evaluated(&json!({ "result": { "type": "undefined" } }));
        assert_eq!(u.value, "undefined");
        let f = parse_evaluated(&json!({ "result": { "type": "function", "description": "function f() {}" } }));
        assert_eq!(f.value, "function f() {}");
        let e = parse_evaluated(&json!({ "result": {}, "exceptionDetails": { "text": "Uncaught",
            "exception": { "description": "ReferenceError: nope is not defined" } } }));
        assert!(e.error);
        assert!(e.value.starts_with("ReferenceError"));
    }

    #[test]
    fn a_huge_value_is_cut_on_a_character_boundary() {
        let big = "é".repeat(MAX_VALUE);
        let v = parse_evaluated(&json!({ "result": { "type": "string", "value": big } }));
        assert!(v.value.len() <= MAX_VALUE + 3);
        assert!(v.value.ends_with('…'));
    }

    #[tokio::test]
    async fn talks_to_a_page_over_a_stream() {
        // A page's WebSocket, served from the other end of an in-memory pipe.
        let (client, server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            use futures_util::{SinkExt, StreamExt};
            use tokio_tungstenite::tungstenite::Message;
            let mut ws = tokio_tungstenite::accept_async(server).await.unwrap();
            while let Some(Ok(Message::Text(text))) = ws.next().await {
                let call: Value = serde_json::from_str(text.as_str()).unwrap();
                let id = call["id"].clone();
                match call["method"].as_str().unwrap() {
                    "Runtime.enable" => {
                        ws.send(Message::text(json!({ "id": id, "result": {} }).to_string())).await.unwrap();
                        let event = json!({ "method": "Runtime.consoleAPICalled", "params": { "type": "log",
                            "args": [{ "type": "string", "value": "hello" }] } });
                        ws.send(Message::text(event.to_string())).await.unwrap();
                    }
                    "Runtime.evaluate" => {
                        let answer = json!({ "id": id, "result": { "result": { "type": "string", "value": "Shop" } } });
                        ws.send(Message::text(answer.to_string())).await.unwrap();
                    }
                    _ => ws.send(Message::text(json!({ "id": id, "result": {} }).to_string())).await.unwrap(),
                }
            }
        });
        let view = WebView { rpc: Rpc::connect(client, "ws://localhost/devtools/page/AB12CD").await.unwrap() };
        let mut events = view.events();
        view.start(false).await.unwrap();
        let line = parse_event(&events.recv().await.unwrap()).unwrap();
        assert_eq!(line.text, "hello");
        assert_eq!(view.evaluate("document.title").await.unwrap().value, "Shop");
        assert!(view.evaluate("   ").await.is_err());
        assert!(view.evaluate(&"x".repeat(MAX_EXPRESSION + 1)).await.is_err());
    }
}
