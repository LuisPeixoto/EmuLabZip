//! The arguments of every tool. The doc comments become the descriptions an assistant reads.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Device {
    /// The device: an id (an adb serial or a simulator UDID) or its name. Optional when only one device is running.
    pub device: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Apps {
    pub device: Option<String>,
    /// Include system apps. By default only installed apps and running ones are listed.
    pub all: Option<bool>,
    /// Only apps that are running.
    pub running: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Logs {
    pub device: Option<String>,
    /// Only lines from this app (a package name or bundle id).
    pub app: Option<String>,
    /// Minimum level: verbose, debug, info, warn, error or fatal.
    pub level: Option<String>,
    /// Only lines whose tag contains this text.
    pub tag: Option<String>,
    /// Only lines whose tag or message contains this text (case-insensitive).
    pub grep: Option<String>,
    /// How many lines to return at most (default 200, up to 1000). The newest lines are kept.
    pub lines: Option<usize>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Perf {
    pub device: Option<String>,
    /// The app to measure (a package name or bundle id). Without it only device-wide values are read.
    pub app: Option<String>,
    /// How many samples to take, one second apart (default 3, up to 10).
    pub samples: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct App {
    pub device: Option<String>,
    /// The app: a package name or bundle id.
    pub app: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Startup {
    pub device: Option<String>,
    /// The app: a package name or bundle id.
    pub app: String,
    /// "cold" (default: the process is stopped first), "warm" (screen closed with back, process kept; Android only) or "hot" (app sent to the background).
    pub kind: Option<String>,
    /// How many times to open the app, 1 to 20 (default 3).
    pub runs: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Energy {
    pub device: Option<String>,
    /// The app: a package name or bundle id.
    pub app: String,
    /// How long to measure, 1 to 120 seconds (default 20). The app should be in use meanwhile.
    pub seconds: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Flutter {
    pub device: Option<String>,
    /// Where the Dart VM service listens, such as `http://127.0.0.1:41233/token=/`. When left out it is found in the log.
    pub uri: Option<String>,
    /// For `flutter_frames`: how long to record, 1 to 30 seconds (default 8). The app should be used meanwhile.
    pub seconds: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReactNative {
    /// The app's package name or bundle id, to pick its runtime among those Metro lists.
    pub app: String,
    /// Metro's port on this computer (default 8081).
    pub metro_port: Option<u16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Heap {
    pub device: Option<String>,
    /// The app: a package name. It must be debuggable (Android).
    pub app: String,
    /// Look in every heap, not only the app's (default false).
    pub all_heaps: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Files {
    pub device: Option<String>,
    pub app: String,
    /// The storage root: `data` or `external` on Android; `data`, `bundle` or `group:<id>` on iOS. Default `data`.
    pub root: Option<String>,
    /// A folder or file, relative to the root. Empty for the root itself.
    pub path: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Crashes {
    pub device: Option<String>,
    /// Only crashes of this app (a package name or bundle id).
    pub app: Option<String>,
    /// Only this kind: crash, native or anr.
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Crash {
    pub device: Option<String>,
    /// An id from `list_crashes`.
    pub id: String,
    /// Include the report as the device wrote it (long). Default false.
    pub raw: Option<bool>,
    /// Keep the original names, without using the mapping.txt, source maps and dSYMs stored for the app. Default false.
    pub original: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct BugReport {
    pub device: Option<String>,
    /// The app the report is about (a package name or bundle id).
    pub app: Option<String>,
    /// Make it from a recorded session (a name from `list_sessions`) instead of the device as it is now.
    pub session: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Also return a link that opens a new issue: `github:owner/repo`, `gitlab:[host/]group/project` or
    /// `jira:site/PROJECTID/ISSUETYPEID`.
    pub issue: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Layout {
    pub device: Option<String>,
    /// How many levels of the view tree to return (default 10).
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Session {
    /// A name from `list_sessions`.
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Recording {
    /// An id from `list_traces`.
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Query {
    /// An id of a Perfetto recording from `list_traces`.
    pub id: String,
    /// PerfettoSQL, for example `select name, count(*) n from slice group by name order by n desc limit 10`.
    pub sql: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordTrace {
    pub device: Option<String>,
    /// `perfetto` (Android system trace), `cpu` (simpleperf, Android) or `instruments` (iOS Simulator).
    pub kind: String,
    /// For perfetto: `cpu`, `frames` or `memory`.
    pub preset: Option<String>,
    /// For cpu and instruments: the app to look at. Required for cpu unless `system_wide`.
    pub app: Option<String>,
    /// For cpu: sample every process (needs an emulator image that allows root).
    pub system_wide: Option<bool>,
    /// For instruments: the template, default `Time Profiler`.
    pub template: Option<String>,
    /// How long to record, in seconds (default 10, up to 120).
    pub duration_s: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Location {
    pub device: Option<String>,
    /// Latitude in degrees. Leave out latitude and longitude to go back to the default location.
    pub lat: Option<String>,
    /// Longitude in degrees.
    pub lon: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Appearance {
    pub device: Option<String>,
    /// `light` or `dark`.
    pub mode: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct StatusBar {
    pub device: Option<String>,
    /// Go back to the real status bar. When set, the other fields are ignored.
    pub clear: Option<bool>,
    /// For example `9:41`.
    pub time: Option<String>,
    /// Battery level shown, 0 to 100.
    pub battery: Option<u8>,
    /// Show full Wi-Fi signal (true) or hide it (false).
    pub wifi: Option<bool>,
    /// Show full cellular signal (true) or hide it (false).
    pub cellular: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Permission {
    pub device: Option<String>,
    pub app: String,
    /// An Android permission (`android.permission.CAMERA`) or an iOS service (`photos`, `location`, `microphone`, …).
    pub permission: String,
    /// `grant`, `revoke` or `reset`.
    pub action: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Notification {
    pub device: Option<String>,
    /// iOS: the app that receives the push. Android posts from the shell and ignores it.
    pub app: Option<String>,
    pub title: String,
    pub body: String,
    /// iOS: the whole push payload as JSON, instead of a plain alert.
    pub payload: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenUrl {
    pub device: Option<String>,
    /// A web address or a link of an app (`myapp://item/3`). `file:`, `javascript:` and similar are refused.
    pub url: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Battery {
    pub device: Option<String>,
    /// Go back to the real battery. When set, the other fields are ignored.
    pub reset: Option<bool>,
    /// Level, 0 to 100 (Android only).
    pub level: Option<u8>,
    pub charging: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunPlugin {
    pub device: Option<String>,
    /// The plugin's name, from `list_plugins`. It must be enabled.
    pub name: String,
    /// The app to hand to the plugin (a package name or bundle id).
    pub app: Option<String>,
    /// For a streaming plugin: how long to listen, in seconds (default 5, up to 60). A one-shot plugin runs to its end.
    pub seconds: Option<u64>,
}
