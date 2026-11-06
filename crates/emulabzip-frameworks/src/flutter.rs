//! Flutter debug and profile builds, through the Dart VM service.
//!
//! The VM service is JSON-RPC 2.0 over a WebSocket. A debug app prints where it listens in the log:
//! `The Dart VM service is listening on http://127.0.0.1:41233/Zk4ei7kv5OQ=/`. The widget tree comes from the
//! inspector's service extensions, frame timings from the `Flutter.Frame` events of the `Extension` stream, and
//! memory from `getMemoryUsage` and `getAllocationProfile`. Release builds have no VM service.

use std::time::Duration;

use emulabzip_core::{Error, Result};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::rpc::Rpc;

/// What the inspector calls the set of objects this tool asks about, so it can free them afterwards.
const GROUP: &str = "emulabzip";
/// A frame has 1/60 of a second at 60 Hz: 16.67 ms.
const FRAME_BUDGET_US: u64 = 16_667;
/// The most nodes read from a widget tree, and the deepest level.
const MAX_NODES: usize = 5_000;
const MAX_DEPTH: usize = 80;
/// The most frames kept for a chart.
const MAX_FRAMES: usize = 3_000;

/// Where a VM service listens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmUri {
    pub port: u16,
    /// The authentication token, such as `/Zk4ei7kv5OQ=/`.
    pub path: String,
}

impl VmUri {
    /// The WebSocket address of the service on this computer's loopback interface.
    pub fn ws_url(&self) -> String {
        let path = if self.path.ends_with('/') { self.path.clone() } else { format!("{}/", self.path) };
        let path = if path.starts_with('/') { path } else { format!("/{path}") };
        format!("ws://127.0.0.1:{}{path}ws", self.port)
    }

    /// Reads `http://127.0.0.1:41233/Zk4ei7kv5OQ=/` (or its `ws://` form).
    pub fn parse(text: &str) -> Option<VmUri> {
        let rest = text.strip_prefix("http://").or_else(|| text.strip_prefix("ws://"))?;
        let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
        let port: u16 = authority.rsplit_once(':')?.1.parse().ok()?;
        let path = path.trim_end_matches("ws").trim_end_matches('/');
        Some(VmUri { port, path: if path.is_empty() { "/".into() } else { format!("/{path}/") } })
    }
}

/// The address in the last log line that says where the VM service listens.
pub fn find_uri<'a>(messages: impl Iterator<Item = &'a str>) -> Option<VmUri> {
    messages
        .filter(|m| m.to_ascii_lowercase().contains("dart vm service") || m.contains("Observatory listening on"))
        .filter_map(|m| {
            let start = m.find("http://")?;
            let token: String = m[start..].chars().take_while(|c| !c.is_whitespace() && *c != ',').collect();
            VmUri::parse(token.trim_end_matches('.'))
        })
        .last()
}

/// One widget in the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetNode {
    /// What the inspector shows: `Text("hello")`, `Scaffold`.
    pub description: String,
    /// The class of the widget, when the inspector says it.
    pub widget_type: Option<String>,
    /// Where in the source it was created, when it was created by the app's own code.
    pub file: Option<String>,
    pub line: Option<u32>,
    /// Created by the app's own code, not by the framework or a package.
    pub local: bool,
    pub children: Vec<WidgetNode>,
}

impl WidgetNode {
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(WidgetNode::count).sum::<usize>()
    }
}

/// Reads a node of the inspector's JSON, with its children. Stops at the limits.
pub fn parse_widget(value: &Value, count: &mut usize, depth: usize) -> Option<WidgetNode> {
    if *count >= MAX_NODES {
        return None;
    }
    *count += 1;
    let description = value.get("description").and_then(Value::as_str).unwrap_or("").to_string();
    let widget_type = value.get("widgetRuntimeType").and_then(Value::as_str).map(str::to_string);
    let location = value.get("creationLocation");
    let file = location.and_then(|l| l.get("file")).and_then(Value::as_str).map(short_file);
    let line = location.and_then(|l| l.get("line")).and_then(Value::as_u64).map(|l| l as u32);
    let local = value.get("createdByLocalProject").and_then(Value::as_bool).unwrap_or(false);
    let children = if depth < MAX_DEPTH {
        value
            .get("children")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|c| parse_widget(c, count, depth + 1)).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Some(WidgetNode { description, widget_type, file, line, local, children })
}

/// `file:///Users/me/app/lib/main.dart` becomes `lib/main.dart` when it can, else the file name.
fn short_file(uri: &str) -> String {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    match path.find("/lib/") {
        Some(i) => path[i + 1..].to_string(),
        None => path.rsplit('/').next().unwrap_or(path).to_string(),
    }
}

/// One frame the engine drew.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub number: u64,
    /// Microseconds the UI thread spent building the frame.
    pub build_us: u64,
    /// Microseconds the raster thread spent drawing it.
    pub raster_us: u64,
    /// Microseconds from the start of the frame to its end.
    pub elapsed_us: u64,
}

impl FrameInfo {
    pub fn janky(&self) -> bool {
        self.build_us > FRAME_BUDGET_US || self.raster_us > FRAME_BUDGET_US
    }
}

/// The frames drawn over some seconds.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameStats {
    pub seconds: u64,
    pub frames: u64,
    pub janky: u64,
    pub jank_percent: f64,
    pub avg_build_us: u64,
    pub avg_raster_us: u64,
    pub worst_us: u64,
    /// The slowest frames, worst first.
    pub slowest: Vec<FrameInfo>,
    /// Every frame in order, up to a limit, for a chart.
    pub timeline: Vec<FrameInfo>,
}

/// Summarizes frames. `seconds` is how long they were collected for.
pub fn summarize_frames(frames: Vec<FrameInfo>, seconds: u64) -> FrameStats {
    let count = frames.len() as u64;
    let janky = frames.iter().filter(|f| f.janky()).count() as u64;
    let avg = |pick: fn(&FrameInfo) -> u64| frames.iter().map(pick).sum::<u64>().checked_div(count).unwrap_or(0);
    let mut slowest = frames.clone();
    slowest.sort_by_key(|f| std::cmp::Reverse(f.build_us.max(f.raster_us)));
    slowest.truncate(5);
    FrameStats {
        seconds,
        frames: count,
        janky,
        jank_percent: if count == 0 { 0.0 } else { janky as f64 * 100.0 / count as f64 },
        avg_build_us: avg(|f| f.build_us),
        avg_raster_us: avg(|f| f.raster_us),
        worst_us: frames.iter().map(|f| f.build_us.max(f.raster_us)).max().unwrap_or(0),
        slowest,
        timeline: frames.into_iter().take(MAX_FRAMES).collect(),
    }
}

/// A `Flutter.Frame` event as a frame.
pub fn parse_frame(event: &Value) -> Option<FrameInfo> {
    let event = event.get("params")?.get("event")?;
    if event.get("extensionKind")?.as_str()? != "Flutter.Frame" {
        return None;
    }
    let data = event.get("extensionData")?;
    let n = |key: &str| data.get(key).and_then(Value::as_u64);
    Some(FrameInfo { number: n("number")?, build_us: n("build")?, raster_us: n("raster")?, elapsed_us: n("elapsed")? })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassUse {
    pub name: String,
    pub library: Option<String>,
    pub instances: u64,
    pub bytes: u64,
}

/// What the Dart heap holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlutterMemory {
    pub heap_used: u64,
    pub heap_capacity: u64,
    /// Memory outside the heap that Dart objects keep alive: images, buffers.
    pub external: u64,
    /// The classes using the most memory.
    pub classes: Vec<ClassUse>,
}

/// Reads `getAllocationProfile` into the classes that use the most memory.
pub fn parse_profile(profile: &Value, top: usize) -> Vec<ClassUse> {
    let mut classes: Vec<ClassUse> = profile
        .get("members")
        .and_then(Value::as_array)
        .map(|members| {
            members
                .iter()
                .filter_map(|m| {
                    let class = m.get("class")?;
                    Some(ClassUse {
                        name: class.get("name")?.as_str()?.to_string(),
                        library: class
                            .get("library")
                            .and_then(|l| l.get("uri"))
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        instances: m.get("instancesCurrent").and_then(Value::as_u64).unwrap_or(0),
                        bytes: m.get("bytesCurrent").and_then(Value::as_u64).unwrap_or(0),
                    })
                })
                .filter(|c| c.bytes > 0)
                .collect()
        })
        .unwrap_or_default();
    classes.sort_by_key(|c| std::cmp::Reverse(c.bytes));
    classes.truncate(top);
    classes
}

/// Tells the inspector which folder is the app's own code, so it can say which widgets the app made, and reports
/// whether widgets know where they were created. Without that (an app built without `--track-widget-creation`;
/// `flutter run` adds it) there are no source locations.
async fn prepare_inspector(rpc: &Rpc, isolate: &str, details: &Value) -> bool {
    let tracked = rpc
        .call("ext.flutter.inspector.isWidgetCreationTracked", json!({ "isolateId": isolate }))
        .await
        .ok()
        .and_then(|v| v.get("result").and_then(Value::as_bool))
        .unwrap_or(false);
    if let Some(uri) = details.get("rootLib").and_then(|l| l.get("uri")).and_then(Value::as_str)
        && let Ok(resolved) =
            rpc.call("lookupResolvedPackageUris", json!({ "isolateId": isolate, "uris": [uri], "local": true })).await
        && let Some(file) = resolved.get("uris").and_then(|u| u.get(0)).and_then(Value::as_str)
        && let Some(root) = file.rfind("/lib/").map(|i| &file[..i])
    {
        // macOS reports the same folder as /private/var in one place and /var in another.
        let plain = root.replacen("/private/var", "/var", 1);
        let _ = rpc
            .call(
                "ext.flutter.inspector.addPubRootDirectories",
                json!({ "isolateId": isolate, "arg0": root, "arg1": plain }),
            )
            .await;
    }
    tracked
}

/// A connection to a Flutter app's VM service.
pub struct Flutter {
    rpc: Rpc,
    isolate: String,
    tracked: bool,
}

impl Flutter {
    /// Connects over `stream` (to the service's port) and finds the isolate that runs the app's UI.
    pub async fn connect<S>(stream: S, uri: &VmUri) -> Result<Flutter>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let rpc = Rpc::connect(stream, &uri.ws_url()).await?;
        let vm = rpc.call("getVM", json!({})).await?;
        let isolates = vm.get("isolates").and_then(Value::as_array).cloned().unwrap_or_default();
        // Prefer the isolate called `main`, then any that has the Flutter inspector.
        let mut ordered: Vec<&Value> = isolates.iter().collect();
        ordered.sort_by_key(|i| i.get("name").and_then(Value::as_str) != Some("main"));
        for isolate in ordered {
            let Some(id) = isolate.get("id").and_then(Value::as_str) else { continue };
            let details = rpc.call("getIsolate", json!({ "isolateId": id })).await?;
            let flutter = details
                .get("extensionRPCs")
                .and_then(Value::as_array)
                .is_some_and(|e| e.iter().any(|n| n.as_str().is_some_and(|n| n.starts_with("ext.flutter."))));
            if flutter {
                let tracked = prepare_inspector(&rpc, id, &details).await;
                return Ok(Flutter { rpc, isolate: id.to_string(), tracked });
            }
        }
        Err(Error::failed(
            "This VM service has no Flutter app yet. Is it a debug or profile build that has finished starting?",
        ))
    }

    /// Whether widgets know where in the source they were created. Without it the tree has no file names.
    pub fn creation_tracked(&self) -> bool {
        self.tracked
    }

    /// The widget tree of the app: the widgets its own code created, as the Flutter inspector shows them.
    pub async fn widget_tree(&self) -> Result<WidgetNode> {
        let isolate = &self.isolate;
        let first = self
            .rpc
            .call(
                "ext.flutter.inspector.getRootWidgetTree",
                json!({ "isolateId": isolate, "groupName": GROUP, "isSummaryTree": "true", "withPreviews": "false" }),
            )
            .await;
        let answer = match first {
            Ok(v) => v,
            // Older Flutter versions only have the summary tree.
            Err(_) => {
                self.rpc
                    .call(
                        "ext.flutter.inspector.getRootWidgetSummaryTree",
                        json!({ "isolateId": isolate, "objectGroup": GROUP, "groupName": GROUP }),
                    )
                    .await?
            }
        };
        let _ = self
            .rpc
            .call("ext.flutter.inspector.disposeGroup", json!({ "isolateId": isolate, "objectGroup": GROUP }))
            .await;
        let root = answer.get("result").unwrap_or(&answer);
        let mut count = 0;
        parse_widget(root, &mut count, 0).ok_or_else(|| Error::failed("The widget tree is empty."))
    }

    /// Records the frames the app draws for `seconds`. Use the app meanwhile: nothing is drawn while it is idle.
    pub async fn frames(&self, seconds: u64) -> Result<FrameStats> {
        let mut events = self.rpc.events();
        self.rpc.call("streamListen", json!({ "streamId": "Extension" })).await?;
        let mut frames = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(seconds), async {
            while let Ok(event) = events.recv().await {
                if let Some(frame) = parse_frame(&event) {
                    frames.push(frame);
                }
            }
        })
        .await;
        let _ = self.rpc.call("streamCancel", json!({ "streamId": "Extension" })).await;
        Ok(summarize_frames(frames, seconds))
    }

    /// The Dart heap and the classes that use the most of it.
    pub async fn memory(&self, top: usize) -> Result<FlutterMemory> {
        let usage = self.rpc.call("getMemoryUsage", json!({ "isolateId": self.isolate })).await?;
        let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        let profile = self.rpc.call("getAllocationProfile", json!({ "isolateId": self.isolate })).await?;
        Ok(FlutterMemory {
            heap_used: n("heapUsage"),
            heap_capacity: n("heapCapacity"),
            external: n("externalUsage"),
            classes: parse_profile(&profile, top),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_address_from_a_log_line() {
        let lines = [
            "flutter: something else",
            "The Dart VM service is listening on http://127.0.0.1:41233/Zk4ei7kv5OQ=/",
            "A Dart VM Service on sdk gphone64 is available at: http://127.0.0.1:52000/abc_DEF=/",
        ];
        let uri = find_uri(lines.into_iter()).unwrap();
        assert_eq!(uri, VmUri { port: 52000, path: "/abc_DEF=/".into() });
        assert_eq!(uri.ws_url(), "ws://127.0.0.1:52000/abc_DEF=/ws");
        let old = find_uri(["Observatory listening on http://127.0.0.1:8181/xyz=/"].into_iter()).unwrap();
        assert_eq!(old.port, 8181);
        assert!(find_uri(["nothing here"].into_iter()).is_none());
        assert!(find_uri(["The Dart VM service is listening on soon"].into_iter()).is_none());
    }

    #[test]
    fn parses_an_address_without_a_token() {
        assert_eq!(VmUri::parse("http://127.0.0.1:8181/").unwrap(), VmUri { port: 8181, path: "/".into() });
        assert_eq!(VmUri::parse("ws://127.0.0.1:8181/a=/ws").unwrap().path, "/a=/");
        assert!(VmUri::parse("127.0.0.1:1").is_none());
        assert_eq!(VmUri { port: 1, path: "/".into() }.ws_url(), "ws://127.0.0.1:1/ws");
    }

    fn tree() -> Value {
        json!({
          "description": "MyApp", "widgetRuntimeType": "MyApp", "createdByLocalProject": true,
          "creationLocation": { "file": "file:///Users/me/app/lib/main.dart", "line": 12, "column": 3, "name": "MyApp" },
          "children": [
            { "description": "Scaffold", "widgetRuntimeType": "Scaffold", "children": [
              { "description": "Text(\"hello\")", "createdByLocalProject": true,
                "creationLocation": { "file": "file:///Users/me/app/lib/home.dart", "line": 30 } } ] }
          ]
        })
    }

    #[test]
    fn reads_a_widget_tree() {
        let mut count = 0;
        let root = parse_widget(&tree(), &mut count, 0).unwrap();
        assert_eq!(count, 3);
        assert_eq!(root.count(), 3);
        assert_eq!((root.description.as_str(), root.widget_type.as_deref()), ("MyApp", Some("MyApp")));
        assert_eq!((root.file.as_deref(), root.line, root.local), (Some("lib/main.dart"), Some(12), true));
        let text = &root.children[0].children[0];
        assert_eq!(text.description, "Text(\"hello\")");
        assert_eq!(text.file.as_deref(), Some("lib/home.dart"));
        assert!(!root.children[0].local);
    }

    #[test]
    fn a_deep_or_huge_tree_is_cut() {
        let mut node = json!({ "description": "leaf" });
        for _ in 0..500 {
            node = json!({ "description": "w", "children": [node] });
        }
        let mut count = 0;
        let root = parse_widget(&node, &mut count, 0).unwrap();
        assert!(root.count() <= MAX_DEPTH + 1);
        let wide = json!({ "description": "row", "children": (0..9_000).map(|_| json!({"description":"x"})).collect::<Vec<_>>() });
        let mut count = 0;
        assert!(parse_widget(&wide, &mut count, 0).unwrap().count() <= MAX_NODES);
    }

    #[test]
    fn files_outside_lib_keep_only_their_name() {
        assert_eq!(short_file("file:///Users/me/.pub-cache/hosted/pkg/src/widget.dart"), "widget.dart");
        assert_eq!(short_file("package:flutter/src/widgets/text.dart"), "text.dart");
    }

    fn frame(n: u64, build: u64, raster: u64) -> FrameInfo {
        FrameInfo { number: n, build_us: build, raster_us: raster, elapsed_us: build + raster }
    }

    #[test]
    fn summarizes_frames_and_counts_the_janky_ones() {
        let stats = summarize_frames(vec![frame(1, 4_000, 3_000), frame(2, 50_000, 2_000), frame(3, 5_000, 20_000)], 3);
        assert_eq!((stats.frames, stats.janky), (3, 2));
        assert!((stats.jank_percent - 66.666).abs() < 0.01);
        assert_eq!(stats.worst_us, 50_000);
        assert_eq!(stats.slowest[0].number, 2);
        assert_eq!(stats.avg_build_us, 19_666);
        let none = summarize_frames(vec![], 5);
        assert_eq!((none.frames, none.jank_percent, none.worst_us), (0, 0.0, 0));
    }

    #[test]
    fn reads_a_frame_event_and_ignores_others() {
        let event = json!({ "method": "streamNotify", "params": { "streamId": "Extension", "event": {
            "kind": "Extension", "extensionKind": "Flutter.Frame",
            "extensionData": { "number": 7, "startTime": 100, "elapsed": 9000, "build": 4000, "raster": 5000, "vsyncOverhead": 10 } } } });
        assert_eq!(
            parse_frame(&event).unwrap(),
            FrameInfo { number: 7, build_us: 4000, raster_us: 5000, elapsed_us: 9000 }
        );
        let other =
            json!({ "method": "streamNotify", "params": { "event": { "extensionKind": "Flutter.Navigation" } } });
        assert!(parse_frame(&other).is_none());
        assert!(parse_frame(&json!({})).is_none());
    }

    #[test]
    fn lists_the_classes_that_use_the_most_memory() {
        let profile = json!({ "members": [
            { "class": { "name": "_Uint8List", "library": { "uri": "dart:typed_data" } }, "instancesCurrent": 10, "bytesCurrent": 5000 },
            { "class": { "name": "String" }, "instancesCurrent": 900, "bytesCurrent": 90000 },
            { "class": { "name": "Empty" }, "instancesCurrent": 0, "bytesCurrent": 0 },
            { "oops": true }
        ]});
        let top = parse_profile(&profile, 5);
        assert_eq!(top.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["String", "_Uint8List"]);
        assert_eq!(top[1].library.as_deref(), Some("dart:typed_data"));
        assert_eq!(parse_profile(&profile, 1).len(), 1);
        assert!(parse_profile(&json!({}), 5).is_empty());
    }
}
