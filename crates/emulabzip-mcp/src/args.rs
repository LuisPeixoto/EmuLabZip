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
pub struct Webview {
    pub device: Option<String>,
    /// The app whose WebView to use (its package name). Optional when only one WebView is open.
    pub app: Option<String>,
    /// The DevTools socket, such as `webview_devtools_remote_1234`, as `webview_list` shows it.
    pub socket: Option<String>,
    /// The page: its id, or part of its address. Optional when the WebView has one page.
    pub page: Option<String>,
    /// For `webview_console`: how long to listen, 1 to 60 seconds (default 8).
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
pub struct SessionTimeline {
    /// A name from `list_sessions`.
    pub id: String,
    /// Only these lanes: taps, logs, requests, frames, gc, crashes, screens or video. All of them when left out.
    pub lanes: Option<Vec<String>>,
    /// At most this many events, oldest first (default 200, at most 2000).
    pub limit: Option<usize>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Locale {
    pub device: Option<String>,
    /// A language tag such as `de-DE`, or the pseudo-locales `en-XA` (long accented text) and `ar-XB` (right to left). Leave
    /// it out to put the language back.
    pub locale: Option<String>,
    /// The app. Android changes the language of one app at a time (Android 13 and newer); the iOS Simulator changes
    /// for every app, the next time each starts.
    pub app: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ColorBlindness {
    pub device: Option<String>,
    /// `protanopia` (no red), `deuteranopia` (no green), `tritanopia` (no blue) or `achromatopsia` (no colour).
    pub vision: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SecurityAudit {
    pub device: Option<String>,
    /// The app: a package name or bundle id.
    pub app: String,
    /// Only these areas: `manifest`, `logs`, `storage`. Default: all of them.
    pub only: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Package {
    /// The path of an `.apk`, `.aab` or `.ipa` file on this computer.
    pub file: String,
    /// How many of the biggest files to list (default 15, up to 100).
    pub top: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PackageDiff {
    /// The older build: an `.apk`, `.aab` or `.ipa` file.
    pub before: String,
    /// The newer build.
    pub after: String,
    /// How many of the biggest changes to list (default 15, up to 100).
    pub top: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckText {
    pub device: Option<String>,
    /// The screen is shown in a right-to-left language: also check that the layout was mirrored.
    pub rtl: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisualCompare {
    pub device: Option<String>,
    /// The name of a baseline made with `emulabzip visual capture` (letters, digits, `.`, `_` and `-`).
    pub baseline: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FontScale {
    pub device: Option<String>,
    /// The text size as a percentage of normal: 100 is normal, 130 and 200 are common accessibility settings (50 to 300).
    pub percent: u32,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct DisplaySize {
    pub device: Option<String>,
    /// Width in pixels (Android only). Give the height too.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Density in dpi (Android only).
    pub density: Option<u32>,
    /// Put the screen size and density back instead.
    pub reset: Option<bool>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckLink {
    pub device: Option<String>,
    /// The link to check, such as `https://shop.example.com/cart/3` or `myapp://open/3`.
    pub url: String,
    /// The app that should open it: a package name or bundle id. Without it the domain file is read but the app's own
    /// side is not checked.
    pub app: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LaunchApp {
    pub device: Option<String>,
    /// The package name or bundle id.
    pub app: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlayFlow {
    pub device: Option<String>,
    /// The id of a saved flow, as `list_flows` shows it.
    pub flow: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Tap {
    pub device: Option<String>,
    /// Pixels from the left edge of the screen, as in a screenshot.
    pub x: u32,
    /// Pixels from the top edge of the screen.
    pub y: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Swipe {
    pub device: Option<String>,
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
    /// How long the drag takes, in milliseconds (default 300, at most 10000). From a point to itself it is a long press.
    pub duration_ms: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InputText {
    pub device: Option<String>,
    /// Printable ASCII only, at most 1000 characters. It is typed into the field that has focus.
    pub text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PressKey {
    pub device: Option<String>,
    /// `back`, `home`, `enter`, `tab`, `delete`, `volume_up`, `app_switch`, `dpad_down`…
    pub key: String,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BootDevice {
    /// The emulator or simulator to start, by name or id. `list_all_devices` shows the ones that are not running.
    pub device: String,
    /// "android" or "ios", when both have a device by that name.
    pub platform: Option<String>,
    /// No window (Android only).
    pub headless: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShutdownDevice {
    /// The running emulator or simulator to stop, by name or id.
    pub device: String,
    /// "android" or "ios", when both have a device by that name.
    pub platform: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveState {
    pub device: Option<String>,
    /// The app: a package name or bundle id. Android needs a debuggable app.
    pub app: String,
    /// A name for the saved state (letters, digits, `.`, `_`, `-`). Default: the app and the time.
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiffState {
    /// A saved state, by the name `list_states` shows.
    pub before: String,
    /// Another saved state to compare it with. Without it, the app as it is on the device now.
    pub after: Option<String>,
    pub device: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmulatorSnapshot {
    pub device: Option<String>,
    /// The snapshot's name (letters, digits, `.`, `_`, `-`).
    pub name: String,
}

// ---------------------------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct NetStart {
    /// The port the proxy listens on, on this computer only (default 9090).
    pub port: Option<u16>,
    /// Talk HTTP/3 to servers that offer it.
    pub http3: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct NetFlows {
    /// Only requests to this host. `*` and `?` work: `*.example.com`.
    pub host: Option<String>,
    /// Only requests whose path (without the query) matches this pattern: `/v1/users/*`.
    pub path: Option<String>,
    /// Only this method, such as `POST`.
    pub method: Option<String>,
    /// `2xx`, `3xx`, `4xx`, `5xx`, `failed` (no answer) or `errors` (4xx, 5xx and failed).
    pub status: Option<String>,
    /// Only requests whose address or GraphQL/gRPC operation contains this text.
    pub text: Option<String>,
    /// Only requests after this id, oldest first. Use the `nextSinceId` of the last answer to read what is new.
    pub since_id: Option<u64>,
    /// At most this many (default 50, at most 200). Without `since_id` these are the newest.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NetFlow {
    /// The request's id, as `net_flows` shows it.
    pub id: u64,
    /// Include the request and response bodies, decoded (default true).
    pub bodies: Option<bool>,
    /// The most characters of each body (default 8000, at most 40000).
    pub max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NetExport {
    /// A name for the file, such as `checkout`. It is written to EmuLabZip's `exports` folder as `<name>.har`.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NetRules {
    /// The whole list of rules, replacing the ones in force. Each is
    /// `{"name", "enabled"?, "matcher": {"methods"?, "host"?, "path"?, "urlContains"?}, "action": {...}, "thenContinue"?}`.
    /// Actions: `{"type":"mock","status","headers":[[k,v]],"body","delayMs"?}`, `{"type":"delay","ms"}`, `{"type":"block"}`,
    /// `{"type":"breakpoint","request","response"}`, `{"type":"mapRemote","scheme"?,"host"?,"port"?,"pathPrefix"?}`,
    /// `{"type":"mapLocal","path"}` (a file or folder inside EmuLabZip's `mocks` folder), `{"type":"validate","spec"}`
    /// (an OpenAPI file) and `{"type":"script","path"}` (only a script the user approved).
    pub rules: Vec<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct NetConditions {
    /// `fast-3g`, `slow-3g`, `slow-wifi`, `high-latency`, `packet-loss` or `offline`.
    pub preset: Option<String>,
    /// Download speed in kilobits per second (changes the preset, or stands alone).
    pub down_kbps: Option<u32>,
    pub up_kbps: Option<u32>,
    /// Time added to every answer, in milliseconds.
    pub latency_ms: Option<u32>,
    /// How much of the traffic stalls like a lost packet, 0 to 100.
    pub loss_percent: Option<f32>,
    /// Back to the normal network.
    pub off: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct NetReplay {
    /// A recorded session, as `list_sessions` shows it. Its requests answer the app's.
    pub session: Option<String>,
    /// Or a HAR file in EmuLabZip's `exports` folder (`net_export_har` writes there), by name.
    pub har: Option<String>,
    /// Let requests that are not in the recording reach the real server. By default they fail with a 504.
    pub passthrough: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NetResend {
    /// The request's id, as `net_flows` shows it.
    pub id: u64,
    pub method: Option<String>,
    /// A whole address, `https://host/path?query`, to send it to instead.
    pub url: Option<String>,
    /// Replaces all the headers.
    pub headers: Option<Vec<(String, String)>>,
    /// Replaces the body.
    pub body_text: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NetResume {
    /// The held message's id, as `net_breakpoints` shows it.
    pub id: u64,
    /// Fail the request instead of letting it go on.
    pub abort: Option<bool>,
    /// Replaces the headers.
    pub headers: Option<Vec<(String, String)>>,
    /// Replaces the body.
    pub body_text: Option<String>,
    /// Replaces the status of a response.
    pub status: Option<u16>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AnalyticsEvents {
    /// Only these services: `segment`, `amplitude`, `mixpanel` or `firebase`. All when left out.
    pub services: Option<Vec<String>>,
    /// At most this many events (default 100).
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenapiCheck {
    /// The full path of an OpenAPI 3 file (`.yaml`, `.yml` or `.json`) on this computer. It is only read.
    pub spec: String,
    /// Only requests to this host.
    pub host: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Watches
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchStart {
    /// What to follow: `logs` (device log lines), `perf` (CPU, memory and frame samples), `touches` (what is touched on an
    /// Android screen), `webview_console` and `webview_network` (a WebView's console, and its requests, which also appear
    /// in `net_flows`), `react_console` and `react_actions` (a React Native app's console, and its Redux actions).
    pub kind: String,
    pub device: Option<String>,
    /// The app: a package name or bundle id. Needed for `react_*`, optional for `logs`, `perf` and the WebView kinds.
    pub app: Option<String>,
    /// For `logs`: the minimum level (verbose, debug, info, warn, error, fatal).
    pub level: Option<String>,
    /// For `logs`: only tags that contain this text.
    pub tag: Option<String>,
    /// For `logs`: only lines whose tag or message contains this text.
    pub grep: Option<String>,
    /// For `perf`: seconds between samples (default 1, at least 1).
    pub interval_seconds: Option<u32>,
    /// For the WebView kinds: the DevTools socket and page, as `webview_list` shows them (optional when there is one).
    pub socket: Option<String>,
    pub page: Option<String>,
    /// For the `react_*` kinds: Metro's port on this computer (default 8081).
    pub metro_port: Option<u16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchRead {
    /// The watch's id, from `watch_start`.
    pub id: u64,
    /// Only what came after this number: the `next` of the last read. Without it, everything kept.
    pub since: Option<u64>,
    /// At most this many items (default 200, at most 1000).
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchId {
    /// The watch's id, from `watch_start`.
    pub id: u64,
}
