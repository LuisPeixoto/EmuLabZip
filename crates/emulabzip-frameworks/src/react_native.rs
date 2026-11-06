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
    fn the_page_script_only_reads() {
        for forbidden in ["fetch(", "XMLHttpRequest", "eval(", "delete ", "= function(", ".push(null"] {
            assert!(!TREE_SCRIPT.contains(forbidden), "{forbidden}");
        }
    }
}
