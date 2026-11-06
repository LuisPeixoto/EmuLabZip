//! A JSON-over-WebSocket client for debugging protocols.
//!
//! The Dart VM service and the Chrome DevTools protocol both send `{"id", "method", "params"}` and answer with
//! `{"id", "result"}` or `{"id", "error"}`. Messages without an id are events. The connection can run over any
//! stream, so a device port reached through ADB works the same as a local socket.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use emulabzip_core::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

/// How long a call waits for its answer.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);
/// The most pending events kept for a slow reader.
const EVENT_BACKLOG: usize = 1024;

/// Calls waiting for an answer, by id. `None` once the connection is gone.
type Waiting = Mutex<Option<HashMap<u64, oneshot::Sender<std::result::Result<Value, String>>>>>;
type Pending = Arc<Waiting>;

/// A connection to a debugging protocol.
pub struct Rpc {
    next: AtomicU64,
    out: mpsc::UnboundedSender<String>,
    pending: Pending,
    events: broadcast::Sender<Value>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Rpc {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Rpc {
    /// Shakes hands over `stream` as a WebSocket client for `url` (`ws://127.0.0.1:1234/path`).
    pub async fn connect<S>(stream: S, url: &str) -> Result<Rpc>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut ws, _) = tokio::time::timeout(CALL_TIMEOUT, tokio_tungstenite::client_async(url, stream))
            .await
            .map_err(|_| Error::failed("The debugger did not answer the connection."))?
            .map_err(|e| Error::failed(format!("Could not open the debugger connection: {e}")))?;
        let (out, mut outgoing) = mpsc::unbounded_channel::<String>();
        let pending: Pending = Arc::new(Mutex::new(Some(HashMap::new())));
        let (events, _) = broadcast::channel(EVENT_BACKLOG);
        let task = {
            let (pending, events) = (pending.clone(), events.clone());
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        send = outgoing.recv() => match send {
                            Some(text) => if ws.send(Message::text(text)).await.is_err() { break },
                            None => break,
                        },
                        incoming = ws.next() => match incoming {
                            Some(Ok(Message::Text(text))) => route(text.as_bytes(), &pending, &events),
                            // The Dart VM service sends big answers as binary frames that hold JSON text.
                            Some(Ok(Message::Binary(bytes))) => route(&bytes, &pending, &events),
                            Some(Ok(Message::Ping(p))) => { let _ = ws.send(Message::Pong(p)).await; }
                            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                            Some(Ok(_)) => {}
                        },
                    }
                }
                // Whoever is still waiting will never get an answer.
                if let Some(map) = pending.lock().unwrap().take() {
                    for (_, tx) in map {
                        let _ = tx.send(Err("The app closed the debugger connection.".into()));
                    }
                }
            })
        };
        Ok(Rpc { next: AtomicU64::new(1), out, pending, events, task })
    }

    /// Calls `method` and returns its `result`.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut guard = self.pending.lock().unwrap();
            let Some(map) = guard.as_mut() else {
                return Err(Error::failed("The app closed the debugger connection."));
            };
            map.insert(id, tx);
        }
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if self.out.send(request.to_string()).is_err() {
            return Err(Error::failed("The app closed the debugger connection."));
        }
        let answer = tokio::time::timeout(CALL_TIMEOUT, rx).await;
        if answer.is_err()
            && let Some(map) = self.pending.lock().unwrap().as_mut()
        {
            map.remove(&id);
        }
        match answer {
            Err(_) => Err(Error::failed(format!("{method} did not answer in {} seconds.", CALL_TIMEOUT.as_secs()))),
            Ok(Err(_)) => Err(Error::failed("The app closed the debugger connection.")),
            Ok(Ok(Err(message))) => Err(Error::failed(message)),
            Ok(Ok(Ok(value))) => Ok(value),
        }
    }

    /// The messages that are not answers: stream events and notifications.
    pub fn events(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }
}

/// Sends an answer to the call that waits for it, or an event to whoever listens.
fn route(text: &[u8], pending: &Pending, events: &broadcast::Sender<Value>) {
    let Some(message) = parse(text) else { return };
    let Some(id) = message.get("id").and_then(Value::as_u64) else {
        let _ = events.send(message);
        return;
    };
    let waiting = pending.lock().unwrap().as_mut().and_then(|m| m.remove(&id));
    let Some(tx) = waiting else { return };
    let answer = match message.get("error") {
        Some(error) => Err(describe_error(error)),
        None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
    };
    let _ = tx.send(answer);
}

/// Reads a message. Debuggers nest JSON deeper than `serde_json` allows by default (a Flutter widget tree is
/// hundreds of levels), so the depth limit is lifted and the stack grows as needed.
fn parse(bytes: &[u8]) -> Option<Value> {
    let mut json = serde_json::Deserializer::from_slice(bytes);
    json.disable_recursion_limit();
    let value = serde::Deserialize::deserialize(serde_stacker::Deserializer::new(&mut json)).ok()?;
    json.end().ok()?;
    Some(value)
}

/// The message of a protocol error: JSON-RPC and the DevTools protocol both write `{code, message}`; the
/// Dart VM service adds the details in `data`.
fn describe_error(error: &Value) -> String {
    let message = error.get("message").and_then(Value::as_str).unwrap_or("The debugger reported an error.");
    match error.get("data").and_then(|d| d.get("details")).and_then(Value::as_str) {
        Some(details) if !details.is_empty() => format!("{message}: {details}"),
        _ => message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_messages_nested_deeper_than_serde_json_allows() {
        let depth = 5_000;
        let text = format!("{{\"id\":1,\"result\":{}0{}}}", "[".repeat(depth), "]".repeat(depth));
        let value = parse(text.as_bytes()).expect("a deep message is read");
        assert_eq!(value["id"], 1);
        assert!(parse(b"{\"id\":").is_none());
        assert!(parse(b"{} trailing").is_none());
        // Dropping a value this deep must not overflow the stack either.
        drop(value);
    }

    #[test]
    fn errors_say_what_went_wrong() {
        assert_eq!(describe_error(&json!({ "code": -32601, "message": "Method not found" })), "Method not found");
        let e = json!({ "code": 113, "message": "Service extension error", "data": { "details": "no such isolate" } });
        assert_eq!(describe_error(&e), "Service extension error: no such isolate");
        assert_eq!(describe_error(&json!(1)), "The debugger reported an error.");
    }
}
