//! Talks to a stand-in for Metro: its target list over HTTP and a Hermes debugger over a WebSocket, on one port.

use emulabzip_frameworks::react_native::{ReactNative, list_targets, pick};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// `hook` says whether the app has the React DevTools hook.
async fn start_metro(hook: bool) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut head = [0u8; 20];
                let n = stream.peek(&mut head).await.unwrap();
                if head[..n].starts_with(b"GET /json/list") {
                    let body = json!([{ "id": "d1", "title": "Hermes React Native", "appId": "dev.emulabzip.rn",
                        "deviceName": "sdk", "vm": "Hermes",
                        "webSocketDebuggerUrl": format!("ws://localhost:{port}/inspector/debug?device=d&page=1") }])
                    .to_string();
                    // Sent in two chunks, the way a server that streams its answer does.
                    let (a, b) = body.split_at(body.len() / 2);
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{a}\r\n{:x}\r\n{b}\r\n0\r\n\r\n",
                        a.len(),
                        b.len()
                    );
                    stream.write_all(reply.as_bytes()).await.unwrap();
                    return;
                }
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    let id = request["id"].clone();
                    let method = request["method"].as_str().unwrap().to_string();
                    let expression = request["params"]["expression"].as_str().unwrap_or("").to_string();
                    let result = match method.as_str() {
                        "Runtime.evaluate" if expression.contains("__REACT_DEVTOOLS_GLOBAL_HOOK__") => {
                            let value = if hook {
                                json!({ "name": "Root", "host": false, "props": [], "children": [
                                    { "name": "App", "host": false, "props": ["title=\"Hi\""], "children": [] } ] })
                            } else {
                                json!({ "error": "hook" })
                            };
                            json!({ "result": { "type": "string", "value": value.to_string() } })
                        }
                        "Runtime.evaluate" => json!({ "result": { "type": "string", "value":
                            json!({ "OSS Release Version": "0.12.0", "Bytecode Version": "96" }).to_string() } }),
                        "Runtime.getHeapUsage" => json!({ "usedSize": 2_000_000, "totalSize": 8_000_000 }),
                        _ => json!({}),
                    };
                    ws.send(Message::text(json!({ "id": id, "result": result }).to_string())).await.unwrap();
                    if method == "Runtime.enable" {
                        let event = json!({ "method": "Runtime.consoleAPICalled", "params": { "type": "log",
                            "args": [{ "type": "string", "value": "hello from JS" }] } });
                        ws.send(Message::text(event.to_string())).await.unwrap();
                    }
                }
            });
        }
    });
    port
}

async fn connect(hook: bool) -> ReactNative {
    let port = start_metro(hook).await;
    let targets = list_targets(port).await.unwrap();
    let target = pick(&targets, "dev.emulabzip.rn").expect("the app's runtime");
    ReactNative::connect(port, target).await.unwrap()
}

#[tokio::test]
async fn lists_the_targets_even_when_the_answer_is_chunked() {
    let targets = list_targets(start_metro(true).await).await.unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].app_id.as_deref(), Some("dev.emulabzip.rn"));
    assert_eq!(targets[0].vm.as_deref(), Some("Hermes"));
}

#[tokio::test]
async fn says_so_when_metro_is_not_running() {
    let free = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let err = list_targets(free).await.unwrap_err().to_string();
    assert!(err.contains("Metro is not running"), "{err}");
}

#[tokio::test]
async fn reads_the_component_tree() {
    let rn = connect(true).await;
    let tree = rn.tree().await.unwrap();
    assert_eq!(tree.count(), 2);
    assert_eq!(tree.children[0].name, "App");
    assert_eq!(tree.children[0].props, ["title=\"Hi\""]);
}

#[tokio::test]
async fn a_release_build_has_no_hook() {
    let rn = connect(false).await;
    let err = rn.tree().await.unwrap_err().to_string();
    assert!(err.contains("no React DevTools hook"), "{err}");
}

#[tokio::test]
async fn reads_the_hermes_heap_and_runtime() {
    let rn = connect(true).await;
    let heap = rn.heap().await.unwrap();
    assert_eq!((heap.used_bytes, heap.total_bytes), (2_000_000, 8_000_000));
    assert_eq!(heap.runtime.as_deref(), Some("0.12.0 (bytecode 96)"));
}

#[tokio::test]
async fn collects_console_messages() {
    let rn = connect(true).await;
    let lines = rn.console(1).await.unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!((lines[0].level.as_str(), lines[0].text.as_str()), ("log", "hello from JS"));
}
