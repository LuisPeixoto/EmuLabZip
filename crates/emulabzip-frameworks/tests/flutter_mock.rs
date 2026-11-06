//! Talks to a stand-in for the Dart VM service, over a real WebSocket on the loopback interface.

use std::time::Duration;

use emulabzip_frameworks::flutter::{Flutter, VmUri};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

/// A VM service with one `main` isolate that runs a Flutter app. `inspector` says whether it knows the newer
/// widget tree call; `frames` is how many frame events it sends once the `Extension` stream is listened to.
async fn start_vm(inspector: bool, frames: u64) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    let (id, method) = (request["id"].clone(), request["method"].as_str().unwrap().to_string());
                    let result = match method.as_str() {
                        "getVM" => Ok(json!({ "isolates": [
                            { "id": "isolates/2", "name": "worker" },
                            { "id": "isolates/1", "name": "main" } ] })),
                        "getIsolate" => {
                            let flutter = request["params"]["isolateId"] == "isolates/1";
                            Ok(
                                json!({ "extensionRPCs": if flutter { vec!["ext.flutter.inspector.show"] } else { vec![] } }),
                            )
                        }
                        "ext.flutter.inspector.getRootWidgetTree" if inspector => Ok(json!({
                            "type": "_extensionType", "method": method,
                            "result": { "description": "MyApp", "createdByLocalProject": true,
                                "children": [{ "description": "Text(\"hi\")" }] } })),
                        "ext.flutter.inspector.getRootWidgetTree" => Err("Method not found"),
                        "ext.flutter.inspector.getRootWidgetSummaryTree" => Ok(json!({
                            "result": { "description": "OldApp", "children": [] } })),
                        "ext.flutter.inspector.disposeGroup" => Ok(json!({})),
                        "streamListen" => Ok(json!({ "type": "Success" })),
                        "streamCancel" => Ok(json!({ "type": "Success" })),
                        "getMemoryUsage" => Ok(json!({ "heapUsage": 1000, "heapCapacity": 4000, "externalUsage": 50 })),
                        "getAllocationProfile" => Ok(json!({ "members": [
                            { "class": { "name": "String" }, "instancesCurrent": 3, "bytesCurrent": 300 } ] })),
                        _ => Err("Method not found"),
                    };
                    let reply = match result {
                        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
                        Err(m) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": m } }),
                    };
                    ws.send(Message::text(reply.to_string())).await.unwrap();
                    if method == "streamListen" {
                        for n in 1..=frames {
                            let slow = n % 2 == 0;
                            let event = json!({ "jsonrpc": "2.0", "method": "streamNotify", "params": {
                                "streamId": "Extension", "event": { "kind": "Extension", "extensionKind": "Flutter.Frame",
                                "extensionData": { "number": n, "startTime": n * 1000, "elapsed": 9000,
                                    "build": if slow { 30_000 } else { 4_000 }, "raster": 3_000, "vsyncOverhead": 0 } } } });
                            ws.send(Message::text(event.to_string())).await.unwrap();
                        }
                    }
                }
            });
        }
    });
    port
}

async fn connect(port: u16) -> emulabzip_core::Result<Flutter> {
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    Flutter::connect(stream, &VmUri { port, path: "/tok=/".into() }).await
}

#[tokio::test]
async fn reads_the_widget_tree_of_the_main_isolate() {
    let flutter = connect(start_vm(true, 0).await).await.unwrap();
    let tree = flutter.widget_tree().await.unwrap();
    assert_eq!(tree.description, "MyApp");
    assert!(tree.local);
    assert_eq!(tree.children[0].description, "Text(\"hi\")");
}

#[tokio::test]
async fn falls_back_to_the_summary_tree_of_older_flutter() {
    let flutter = connect(start_vm(false, 0).await).await.unwrap();
    assert_eq!(flutter.widget_tree().await.unwrap().description, "OldApp");
}

#[tokio::test]
async fn counts_the_frames_that_arrive_while_it_listens() {
    let flutter = connect(start_vm(true, 10).await).await.unwrap();
    let started = std::time::Instant::now();
    let stats = flutter.frames(1).await.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(900));
    assert_eq!((stats.frames, stats.janky), (10, 5));
    assert_eq!(stats.worst_us, 30_000);
    assert_eq!(stats.timeline.len(), 10);
}

#[tokio::test]
async fn reads_the_memory_of_the_isolate() {
    let flutter = connect(start_vm(true, 0).await).await.unwrap();
    let memory = flutter.memory(5).await.unwrap();
    assert_eq!((memory.heap_used, memory.heap_capacity, memory.external), (1000, 4000, 50));
    assert_eq!(memory.classes[0].name, "String");
}

#[tokio::test]
async fn says_so_when_no_isolate_runs_flutter() {
    // A server whose isolates have no Flutter extensions.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        while let Some(Ok(Message::Text(text))) = ws.next().await {
            let request: Value = serde_json::from_str(&text).unwrap();
            let result = match request["method"].as_str().unwrap() {
                "getVM" => json!({ "isolates": [{ "id": "isolates/1", "name": "main" }] }),
                _ => json!({ "extensionRPCs": [] }),
            };
            ws.send(Message::text(json!({ "id": request["id"], "result": result }).to_string())).await.unwrap();
        }
    });
    let err = connect(port).await.err().expect("no Flutter here").to_string();
    assert!(err.contains("no Flutter app"), "{err}");
}

#[tokio::test]
async fn an_error_from_the_service_reaches_the_caller() {
    let port = start_vm(true, 0).await;
    let rpc =
        emulabzip_frameworks::Rpc::connect(TcpStream::connect(("127.0.0.1", port)).await.unwrap(), "ws://127.0.0.1/ws")
            .await
            .unwrap();
    assert_eq!(rpc.call("nope", json!({})).await.unwrap_err().to_string(), "Method not found");
}

#[tokio::test]
async fn a_connection_the_app_closes_ends_the_calls_waiting_on_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        // Reads the first request and hangs up without answering.
        let _ = ws.next().await;
    });
    let rpc =
        emulabzip_frameworks::Rpc::connect(TcpStream::connect(("127.0.0.1", port)).await.unwrap(), "ws://127.0.0.1/ws")
            .await
            .unwrap();
    let err = rpc.call("getVM", json!({})).await.unwrap_err().to_string();
    assert!(err.contains("closed the debugger connection"), "{err}");
    let again = rpc.call("getVM", json!({})).await.unwrap_err().to_string();
    assert!(again.contains("closed the debugger connection"), "{again}");
}
