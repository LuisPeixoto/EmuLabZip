//! React Native apps running on Hermes, through Metro's inspector.
//!
//! Metro, the development server, lists the JavaScript runtimes of connected apps at `/json/list` and proxies
//! each one's Chrome DevTools protocol on a WebSocket. Through it this reads the React component tree (from the
//! React DevTools hook the app installs in development), Hermes' heap usage and the JavaScript console.
//!
//! Metro lets only one debugger connect to an app at a time: opening this can disconnect React Native DevTools
//! or Chrome's inspector, and the other way round.

use std::time::Duration;

use emulabzip_core::{Error, Result};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::rpc::Rpc;

/// Metro's default port.
pub const METRO_PORT: u16 = 8081;
/// The most nodes read from the React tree, and the deepest level.
const MAX_NODES: usize = 4_000;
const MAX_DEPTH: usize = 100;
/// The most console messages kept.
const MAX_CONSOLE: usize = 500;

/// A JavaScript runtime that Metro knows about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub id: String,
    pub title: String,
    /// The app's id, when Metro says it.
    pub app_id: Option<String>,
    pub device_name: Option<String>,
    /// `Hermes` for the engine that has a debugger.
    pub vm: Option<String>,
    /// Where the protocol listens: `ws://…/inspector/debug?…`.
    #[serde(skip)]
    pub ws_path: String,
}

/// Reads Metro's `/json/list`.
pub fn parse_targets(list: &Value) -> Vec<Target> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|t| {
                    let url = t.get("webSocketDebuggerUrl")?.as_str()?;
                    // Only the path and query matter: the connection goes to Metro's own address.
                    let path =
                        url.split_once("://").map_or(url, |(_, rest)| rest.find('/').map_or("/", |i| &rest[i..]));
                    let text = |key: &str| t.get(key).and_then(Value::as_str).map(str::to_string);
                    Some(Target {
                        id: text("id").unwrap_or_default(),
                        title: text("title").unwrap_or_default(),
                        app_id: text("appId").or_else(|| text("description")),
                        device_name: text("deviceName"),
                        vm: text("vm"),
                        ws_path: path.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The runtime to look at for `app`: the one that says it is that app, or the only one there is. Helper
/// runtimes (Reanimated's UI runtime, Metro's own pages) are not chosen on their own.
pub fn pick<'a>(targets: &'a [Target], app: &str) -> Option<&'a Target> {
    let main: Vec<&Target> =
        targets.iter().filter(|t| !t.title.contains("Reanimated") && !t.title.contains("Experimental")).collect();
    main.iter().copied().find(|t| t.app_id.as_deref() == Some(app)).or(if main.len() == 1 {
        main.first().copied()
    } else {
        None
    })
}

/// Asks Metro what it knows, over plain HTTP.
pub async fn list_targets(port: u16) -> Result<Vec<Target>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.map_err(|_| {
        Error::failed(format!("Metro is not running on port {port}. Start it with `npx react-native start`."))
    })?;
    let body = http_get(&mut stream, "/json/list").await?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| Error::failed("Metro did not answer with a list."))?;
    Ok(parse_targets(&value))
}

/// A bare HTTP/1.1 GET: the body of the answer, whether it has a length or comes in chunks.
async fn http_get<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S, path: &str) -> Result<Vec<u8>> {
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await?;
    let mut raw = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(10), async {
        (&mut *stream).take(8 * 1024 * 1024).read_to_end(&mut raw).await
    })
    .await;
    if read.is_err() {
        return Err(Error::failed("Metro did not answer in time."));
    }
    let split =
        raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| Error::failed("Metro sent a broken answer."))?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    if !head.starts_with("http/1.1 200") && !head.starts_with("http/1.0 200") {
        return Err(Error::failed(format!("Metro answered: {}", head.lines().next().unwrap_or(""))));
    }
    let body = &raw[split + 4..];
    if head.contains("transfer-encoding: chunked") { Ok(dechunk(body)) } else { Ok(body.to_vec()) }
}

fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line_end) = body.windows(2).position(|w| w == b"\r\n") {
        let size = std::str::from_utf8(&body[..line_end])
            .ok()
            .and_then(|l| usize::from_str_radix(l.split(';').next().unwrap_or("").trim(), 16).ok());
        let Some(size) = size else { break };
        body = &body[line_end + 2..];
        if size == 0 || body.len() < size {
            break;
        }
        out.extend_from_slice(&body[..size]);
        body = body.get(size + 2..).unwrap_or_default();
    }
    out
}

/// A component in the React tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactNode {
    pub name: String,
    /// A native view (`RCTView`) rather than a component the app wrote or a library provides.
    pub host: bool,
    pub key: Option<String>,
    /// The props, shortened: `title="Hello"`, `onPress=ƒ`.
    pub props: Vec<String>,
    pub children: Vec<ReactNode>,
}

impl ReactNode {
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(ReactNode::count).sum::<usize>()
    }
}

/// Reads the tree the page script builds.
pub fn parse_node(value: &Value, count: &mut usize, depth: usize) -> Option<ReactNode> {
    if *count >= MAX_NODES {
        return None;
    }
    *count += 1;
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    Some(ReactNode {
        name: text("name")?,
        host: value.get("host").and_then(Value::as_bool).unwrap_or(false),
        key: text("key"),
        props: value
            .get("props")
            .and_then(Value::as_array)
            .map(|p| p.iter().filter_map(|v| v.as_str().map(str::to_string)).take(12).collect())
            .unwrap_or_default(),
        children: if depth < MAX_DEPTH {
            value
                .get("children")
                .and_then(Value::as_array)
                .map(|c| c.iter().filter_map(|n| parse_node(n, count, depth + 1)).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        },
    })
}

/// A page script that walks the fibers React keeps for each root and returns the tree as JSON text. It only reads.
pub const TREE_SCRIPT: &str = r#"(function () {
  var hook = globalThis.__REACT_DEVTOOLS_GLOBAL_HOOK__;
  if (!hook) return JSON.stringify({ error: 'hook' });
  var count = 0;
  function short(v) {
    var t = typeof v;
    if (t === 'function') return 'ƒ';
    if (t === 'string') return JSON.stringify(v.length > 40 ? v.slice(0, 40) + '…' : v);
    if (t === 'number' || t === 'boolean' || v == null) return String(v);
    if (Array.isArray(v)) return '[' + v.length + ']';
    return '{…}';
  }
  function nameOf(f) {
    var t = f.type;
    if (typeof t === 'string') return t;
    if (t && (t.displayName || t.name)) return t.displayName || t.name;
    if (t && t.$$typeof) {
      var inner = t.render || t.type;
      if (inner && (inner.displayName || inner.name)) return inner.displayName || inner.name;
    }
    return null;
  }
  function walk(f, depth) {
    var out = [];
    for (; f && count < 6000; f = f.sibling) {
      var name = nameOf(f);
      var kids = depth < 120 ? walk(f.child, depth + 1) : [];
      if (name === null) { out = out.concat(kids); continue; }
      count++;
      var props = [];
      var p = f.memoizedProps;
      if (p && typeof p === 'object' && typeof f.type !== 'string') {
        var keys = Object.keys(p);
        for (var i = 0; i < keys.length && i < 12; i++) {
          if (keys[i] !== 'children' && p[keys[i]] !== undefined) props.push(keys[i] + '=' + short(p[keys[i]]));
        }
      }
      out.push({ name: name, host: typeof f.type === 'string', key: f.key == null ? null : String(f.key), props: props, children: kids });
    }
    return out;
  }
  var roots = [];
  if (typeof hook.getFiberRoots === 'function' && hook.renderers) {
    hook.renderers.forEach(function (_, id) {
      hook.getFiberRoots(id).forEach(function (r) { roots = roots.concat(walk(r.current, 0)); });
    });
  }
  if (!roots.length) return JSON.stringify({ error: 'empty' });
  return JSON.stringify({ name: 'Root', host: false, key: null, props: [], children: roots });
})()"#;

const JS_COMMON: &str = include_str!("js/common.js");

/// Wraps a script that uses the shared helpers so nothing it declares reaches the page's global scope.
fn page_script(body: &str) -> String {
    format!("(function () {{\n{JS_COMMON}\nreturn ({body});\n}})()")
}

/// The script that collects the app's state: Redux stores, React Query caches, React contexts and the named stores.
pub fn state_script() -> String {
    page_script(include_str!("js/state.js"))
}

/// Wraps `dispatch` of each Redux store to keep the last 200 actions. It is the one script that changes the page, and
/// [`actions_remove_script`] puts everything back.
pub fn actions_install_script() -> String {
    page_script(include_str!("js/actions_install.js"))
}

pub fn actions_read_script(since: u64) -> String {
    page_script(&format!("({})({since})", include_str!("js/actions_read.js").trim()))
}

pub fn actions_remove_script() -> String {
    page_script(include_str!("js/actions_remove.js"))
}

/// One piece of state: a Redux store, a React Query cache, a context or a named store.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateStore {
    /// `redux`, `query`, `context` or `store`.
    pub kind: String,
    pub name: String,
    pub state: Value,
    /// What the page cut to keep the state small, with where.
    pub cut: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReactState {
    pub stores: Vec<StateStore>,
}

/// An action that went through a Redux store's `dispatch`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchedAction {
    pub seq: u64,
    /// Milliseconds since the epoch, on the device.
    pub at: u64,
    pub store: String,
    pub kind: String,
    pub action: Value,
}

/// Reads what the state script returned. `"hook"` means a release build, where React keeps no DevTools hook.
pub fn parse_state(text: &str) -> Result<ReactState> {
    let value: Value = serde_json::from_str(text).map_err(|_| Error::failed("The state script gave no answer."))?;
    if value.get("error").and_then(Value::as_str) == Some("hook") {
        return Err(Error::failed(
            "React has no DevTools hook here, which is the case in a release build. State needs a debug build, or stores \
             registered in globalThis.__EMULABZIP_STORES__.",
        ));
    }
    let stores = value
        .get("stores")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|s| StateStore {
                    kind: s.get("kind").and_then(Value::as_str).unwrap_or("store").to_string(),
                    name: s.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                    state: s.get("state").cloned().unwrap_or(Value::Null),
                    cut: s
                        .get("cut")
                        .and_then(Value::as_array)
                        .map(|c| c.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                    error: s.get("error").and_then(Value::as_str).map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ReactState { stores })
}

/// Reads what the actions script returned, and whether the log is installed.
pub fn parse_actions(text: &str) -> (bool, Vec<DispatchedAction>) {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let installed = value.get("installed").and_then(Value::as_bool).unwrap_or(false);
    let actions = value
        .get("actions")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(DispatchedAction {
                        seq: a.get("seq")?.as_u64()?,
                        at: a.get("at").and_then(Value::as_u64).unwrap_or(0),
                        store: a.get("store").and_then(Value::as_str).unwrap_or("redux").to_string(),
                        kind: a.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
                        action: a.get("action").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (installed, actions)
}

/// One difference between two states.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateChange {
    /// Where, like `redux/cart/items/0/qty`.
    pub path: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

impl StateChange {
    pub fn line(&self) -> String {
        let show = |v: &Value| {
            let text = v.to_string();
            if text.chars().count() > 80 { format!("{}…", text.chars().take(80).collect::<String>()) } else { text }
        };
        match (&self.before, &self.after) {
            (None, Some(a)) => format!("+ {} = {}", self.path, show(a)),
            (Some(b), None) => format!("- {} (was {})", self.path, show(b)),
            (Some(b), Some(a)) => format!("~ {}: {} → {}", self.path, show(b), show(a)),
            (None, None) => self.path.clone(),
        }
    }
}

const MAX_CHANGES: usize = 500;

/// The differences between two states, down to the value that changed. Arrays are compared by position.
pub fn diff_values(path: &str, before: &Value, after: &Value, out: &mut Vec<StateChange>) {
    if out.len() >= MAX_CHANGES || before == after {
        return;
    }
    let join = |key: &str| if path.is_empty() { key.to_string() } else { format!("{path}/{key}") };
    match (before, after) {
        (Value::Object(a), Value::Object(b)) if !is_marker(a) && !is_marker(b) => {
            for (key, old) in a {
                match b.get(key) {
                    Some(new) => diff_values(&join(key), old, new, out),
                    None => out.push(StateChange { path: join(key), before: Some(old.clone()), after: None }),
                }
            }
            for (key, new) in b {
                if !a.contains_key(key) && out.len() < MAX_CHANGES {
                    out.push(StateChange { path: join(key), before: None, after: Some(new.clone()) });
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for i in 0..a.len().max(b.len()) {
                match (a.get(i), b.get(i)) {
                    (Some(old), Some(new)) => diff_values(&join(&i.to_string()), old, new, out),
                    (Some(old), None) => {
                        out.push(StateChange { path: join(&i.to_string()), before: Some(old.clone()), after: None })
                    }
                    (None, Some(new)) => {
                        out.push(StateChange { path: join(&i.to_string()), before: None, after: Some(new.clone()) })
                    }
                    (None, None) => {}
                }
                if out.len() >= MAX_CHANGES {
                    break;
                }
            }
        }
        _ => out.push(StateChange { path: path.to_string(), before: Some(before.clone()), after: Some(after.clone()) }),
    }
}

/// The page script writes `{ "$": "date", … }` for values JSON cannot hold; those are compared whole.
fn is_marker(map: &serde_json::Map<String, Value>) -> bool {
    map.get("$").is_some_and(Value::is_string)
}

/// The differences between two readings, store by store (a store is named by its kind and name).
pub fn diff_states(before: &ReactState, after: &ReactState) -> Vec<StateChange> {
    let mut out = Vec::new();
    for old in &before.stores {
        match after.stores.iter().find(|s| s.kind == old.kind && s.name == old.name) {
            Some(new) => diff_values(&old.name, &old.state, &new.state, &mut out),
            None => out.push(StateChange { path: old.name.clone(), before: Some(old.state.clone()), after: None }),
        }
    }
    for new in &after.stores {
        if !before.stores.iter().any(|s| s.kind == new.kind && s.name == new.name) {
            out.push(StateChange { path: new.name.clone(), before: None, after: Some(new.state.clone()) });
        }
    }
    out
}

/// What Hermes uses of its heap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesHeap {
    pub used_bytes: u64,
    pub total_bytes: u64,
    /// The runtime's own description, when it gives one: the Hermes build and bytecode version.
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    /// `log`, `warn`, `error`, `info`…
    pub level: String,
    pub text: String,
}

/// A console message as a line.
pub fn parse_console(event: &Value) -> Option<ConsoleLine> {
    if event.get("method")?.as_str()? != "Runtime.consoleAPICalled" {
        return None;
    }
    let params = event.get("params")?;
    let level = params.get("type")?.as_str()?.to_string();
    let text: Vec<String> = params
        .get("args")?
        .as_array()?
        .iter()
        .map(|a| match a.get("value") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => a.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .collect();
    Some(ConsoleLine { level, text: strip_colors(&text.join(" ")) })
}

/// Removes the color codes (`ESC [ … m`) some messages carry, which a window would show as garbage.
fn strip_colors(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Connects to the runtime of `app` through Metro on `port`.
pub async fn connect_app(app: &str, port: u16) -> Result<ReactNative> {
    let targets = list_targets(port).await?;
    let Some(target) = pick(&targets, app) else {
        let known: Vec<String> = targets.iter().filter_map(|t| t.app_id.clone()).collect();
        return Err(Error::failed(format!(
            "Metro does not list a runtime for {app}. It lists: {}. Open the app on a device that reaches Metro.",
            if known.is_empty() { "nothing".to_string() } else { known.join(", ") }
        )));
    };
    ReactNative::connect(port, target).await
}

/// A connection to a React Native app's JavaScript runtime.
pub struct ReactNative {
    rpc: Rpc,
}

impl ReactNative {
    /// Connects to `target` through Metro on `port`.
    pub async fn connect(port: u16, target: &Target) -> Result<ReactNative> {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .map_err(|e| Error::failed(format!("Could not reach Metro on port {port}: {e}")))?;
        let url = format!("ws://127.0.0.1:{port}{}", target.ws_path);
        Ok(ReactNative { rpc: Rpc::connect_as(stream, &url, Some(&format!("http://localhost:{port}"))).await? })
    }

    async fn evaluate(&self, expression: &str) -> Result<Value> {
        let answer = self
            .rpc
            .call("Runtime.evaluate", json!({ "expression": expression, "returnByValue": true, "silent": true }))
            .await?;
        if let Some(exception) = answer.get("exceptionDetails") {
            let text = exception.get("exception").and_then(|e| e.get("description")).and_then(Value::as_str);
            return Err(Error::failed(text.unwrap_or("The page script failed.").to_string()));
        }
        Ok(answer.get("result").and_then(|r| r.get("value")).cloned().unwrap_or(Value::Null))
    }

    /// The app's state: Redux, React Query, React contexts, and stores in `globalThis.__EMULABZIP_STORES__`.
    pub async fn state(&self) -> Result<ReactState> {
        let text = self.evaluate(&state_script()).await?;
        parse_state(text.as_str().unwrap_or("{}"))
    }

    /// Starts keeping the Redux actions from now on. Returns how many stores it wrapped. A development build only.
    pub async fn watch_actions(&self) -> Result<usize> {
        let text = self.evaluate(&actions_install_script()).await?;
        let value: Value = serde_json::from_str(text.as_str().unwrap_or("{}")).unwrap_or(Value::Null);
        Ok(value.get("wrapped").and_then(Value::as_u64).unwrap_or(0) as usize)
    }

    /// The actions after `since` (a `seq`; 0 for all kept), and whether the log is installed.
    pub async fn actions_since(&self, since: u64) -> Result<(bool, Vec<DispatchedAction>)> {
        let text = self.evaluate(&actions_read_script(since)).await?;
        Ok(parse_actions(text.as_str().unwrap_or("{}")))
    }

    /// Puts each store's `dispatch` back and drops the log.
    pub async fn stop_actions(&self) -> Result<()> {
        self.evaluate(&actions_remove_script()).await.map(|_| ())
    }

    /// The components on screen, from the React DevTools hook of a development build.
    pub async fn tree(&self) -> Result<ReactNode> {
        let text = self.evaluate(TREE_SCRIPT).await?;
        let value: Value = serde_json::from_str(text.as_str().unwrap_or("{}")).unwrap_or(Value::Null);
        match value.get("error").and_then(Value::as_str) {
            Some("hook") => {
                return Err(Error::failed(
                    "This app has no React DevTools hook. Only development builds have one: is it a release build?",
                ));
            }
            Some(_) => return Err(Error::failed("React has not rendered anything yet.")),
            None => {}
        }
        let mut count = 0;
        parse_node(&value, &mut count, 0).ok_or_else(|| Error::failed("The component tree is empty."))
    }

    /// Hermes' heap usage.
    pub async fn heap(&self) -> Result<HermesHeap> {
        let usage = self.rpc.call("Runtime.getHeapUsage", json!({})).await?;
        let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        let runtime = self
            .evaluate(
                "JSON.stringify(globalThis.HermesInternal && HermesInternal.getRuntimeProperties ? HermesInternal.getRuntimeProperties() : null)",
            )
            .await
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .filter(|s| s != "null")
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .map(|props| {
                let get = |k: &str| props.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                match get("Bytecode Version").as_str() {
                    "" => get("OSS Release Version"),
                    bytecode => format!("{} (bytecode {bytecode})", get("OSS Release Version")),
                }
            });
        Ok(HermesHeap { used_bytes: n("usedSize"), total_bytes: n("totalSize"), runtime })
    }

    /// The console messages written for `seconds`.
    pub async fn console(&self, seconds: u64) -> Result<Vec<ConsoleLine>> {
        let mut events = self.rpc.events();
        self.rpc.call("Runtime.enable", json!({})).await?;
        let mut lines = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(seconds), async {
            while let Ok(event) = events.recv().await {
                if let Some(line) = parse_console(&event)
                    && lines.len() < MAX_CONSOLE
                {
                    lines.push(line);
                }
            }
        })
        .await;
        let _ = self.rpc.call("Runtime.disable", json!({})).await;
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Value {
        json!([
          { "id": "dev-1", "title": "Hermes React Native", "description": "com.example.shop", "appId": "com.example.shop",
            "deviceName": "sdk_gphone64", "vm": "Hermes",
            "webSocketDebuggerUrl": "ws://localhost:8081/inspector/debug?device=abc&page=1" },
          { "id": "dev-2", "title": "Reanimated UI runtime", "appId": "com.example.shop", "vm": "Hermes",
            "webSocketDebuggerUrl": "ws://localhost:8081/inspector/debug?device=abc&page=2" },
          { "id": "x", "title": "no socket" }
        ])
    }

    #[test]
    fn reads_the_targets_metro_lists() {
        let targets = parse_targets(&list());
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].ws_path, "/inspector/debug?device=abc&page=1");
        assert_eq!(targets[0].device_name.as_deref(), Some("sdk_gphone64"));
        assert!(parse_targets(&json!({})).is_empty());
    }

    #[test]
    fn picks_the_apps_own_runtime_and_not_the_helpers() {
        let targets = parse_targets(&list());
        assert_eq!(pick(&targets, "com.example.shop").unwrap().id, "dev-1");
        // Only one real runtime: it is taken even when the name differs.
        assert_eq!(pick(&targets, "com.other").unwrap().id, "dev-1");
        let mut two = targets.clone();
        two[1].title = "Hermes React Native".into();
        two[1].app_id = Some("com.second".into());
        assert!(pick(&two, "com.nobody").is_none());
        assert_eq!(pick(&two, "com.second").unwrap().id, "dev-2");
    }

    #[test]
    fn reads_chunked_and_plain_answers() {
        assert_eq!(dechunk(b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"), b"hello world");
        assert_eq!(dechunk(b"zz\r\nbroken"), b"");
    }

    #[test]
    fn reads_the_component_tree() {
        let value = json!({ "name": "Root", "host": false, "props": [], "children": [
            { "name": "App", "host": false, "key": null, "props": ["title=\"Hi\"", "onPress=ƒ"], "children": [
                { "name": "RCTView", "host": true, "props": [], "children": [] } ] } ] });
        let mut count = 0;
        let root = parse_node(&value, &mut count, 0).unwrap();
        assert_eq!(root.count(), 3);
        assert_eq!(root.children[0].props, ["title=\"Hi\"", "onPress=ƒ"]);
        assert!(root.children[0].children[0].host);
        assert!(parse_node(&json!({ "no": "name" }), &mut 0, 0).is_none());
    }

    #[test]
    fn reads_console_messages() {
        let event = json!({ "method": "Runtime.consoleAPICalled", "params": { "type": "warn", "args": [
            { "type": "string", "value": "slow" }, { "type": "number", "value": 3 }, { "type": "object", "description": "Object" } ] } });
        assert_eq!(parse_console(&event).unwrap(), ConsoleLine { level: "warn".into(), text: "slow 3 Object".into() });
        assert!(parse_console(&json!({ "method": "Other" })).is_none());
    }

    #[test]
    fn color_codes_are_removed_from_console_text() {
        assert_eq!(strip_colors("\u{1b}[48;2;253;247;231m\u{1b}[30m\u{1b}[1mNOTE: \u{1b}[22mhello"), "NOTE: hello");
        assert_eq!(strip_colors("plain"), "plain");
        assert_eq!(strip_colors("cut \u{1b}[3"), "cut ");
    }

    #[test]
    fn state_is_read_from_the_pages_answer() {
        let state = parse_state(
            r#"{"stores":[{"kind":"redux","name":"redux","state":{"n":1},"cut":["/a: depth"]},{"kind":"store","name":"cart","state":null,"cut":[],"error":"boom"}]}"#,
        )
        .unwrap();
        assert_eq!(state.stores.len(), 2);
        assert_eq!(state.stores[0].state, json!({ "n": 1 }));
        assert_eq!(state.stores[0].cut, ["/a: depth"]);
        assert_eq!(state.stores[1].error.as_deref(), Some("boom"));
        assert!(parse_state(r#"{"error":"hook"}"#).unwrap_err().to_string().contains("release build"));
        assert!(parse_state("nope").is_err());
    }

    #[test]
    fn actions_are_read_and_a_missing_log_is_said() {
        let (installed, actions) = parse_actions(
            r#"{"installed":true,"actions":[{"seq":3,"at":9,"store":"redux","type":"cart/add","action":{"type":"cart/add"}},{"no":"seq"}]}"#,
        );
        assert!(installed);
        assert_eq!(actions.len(), 1);
        assert_eq!((actions[0].seq, actions[0].kind.as_str()), (3, "cart/add"));
        assert!(!parse_actions("{}").0);
    }

    #[test]
    fn states_are_compared_down_to_the_value() {
        let before = ReactState {
            stores: vec![StateStore {
                kind: "redux".into(),
                name: "redux".into(),
                state: json!({ "cart": { "items": [{ "qty": 1 }], "open": false }, "gone": 1, "when": { "$": "date", "value": "a" } }),
                cut: vec![],
                error: None,
            }],
        };
        let mut after = before.clone();
        after.stores[0].state = json!({ "cart": { "items": [{ "qty": 2 }, { "qty": 1 }], "open": false }, "new": true, "when": { "$": "date", "value": "b" } });
        after.stores.push(StateStore {
            kind: "store".into(),
            name: "theme".into(),
            state: json!("dark"),
            cut: vec![],
            error: None,
        });
        let lines: Vec<String> = diff_states(&before, &after).iter().map(StateChange::line).collect();
        assert!(lines.contains(&"~ redux/cart/items/0/qty: 1 → 2".to_string()), "{lines:?}");
        assert!(lines.contains(&"+ redux/cart/items/1 = {\"qty\":1}".to_string()), "{lines:?}");
        assert!(lines.contains(&"- redux/gone (was 1)".to_string()), "{lines:?}");
        assert!(lines.contains(&"+ redux/new = true".to_string()), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("~ redux/when:")), "a date marker is compared whole: {lines:?}");
        assert!(lines.contains(&"+ theme = \"dark\"".to_string()), "{lines:?}");
        assert!(diff_states(&before, &before).is_empty());
    }

    #[test]
    fn the_state_scripts_are_whole_expressions() {
        for script in [state_script(), actions_install_script(), actions_read_script(4), actions_remove_script()] {
            assert!(script.starts_with("(function () {") && script.trim_end().ends_with("})()"));
            for forbidden in ["fetch(", "XMLHttpRequest", "eval("] {
                assert!(!script.contains(forbidden), "{forbidden}");
            }
        }
        assert!(actions_read_script(4).contains("})(4)"));
    }

    #[test]
    fn the_page_script_only_reads() {
        for forbidden in ["fetch(", "XMLHttpRequest", "eval(", "delete ", "= function(", ".push(null"] {
            assert!(!TREE_SCRIPT.contains(forbidden), "{forbidden}");
        }
    }
}
