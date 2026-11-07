# Roadmap

EmuLabZip grows in phases. Each phase ends with a release, and every item in it ships working on Android emulators
and, where the platform allows, iOS simulators. Items are roughly in priority order within each phase. Ideas and pull
requests are welcome; open an issue to discuss anything large first.

## ✅ Phase 1: Connect and observe (v0.1)

Automatic discovery of Android emulators and devices (pure-Rust ADB client) and booted iOS simulators (`simctl`) ·
device picker with live status · app picker with running process ids · **Overview** with build, hardware, display,
battery, storage, locale and host facts · live screen preview and screenshots · **streaming log viewer** for logcat and
the unified log: level, tag, text and regular-expression filters, per-app filter, pause, virtualized list that handles
thousands of lines per second, plain-text and JSON-lines export · `emulabzip` command line: `devices`, `info`, `apps`,
`logs`, `screenshot`.

## ✅ Phase 2: Performance, memory and storage (v0.2)

Live CPU, memory, frame rate, janky frames and battery charts for the selected app · memory breakdown (Java, native,
graphics, code, stack on Android; VM regions on iOS) with snapshots and a diff · heap dumps (`.hprof`) on Android and
memory graphs (`.memgraph`) on iOS · file browser for the app sandbox (`run-as` for debuggable Android apps, shared
storage, and the data, bundle and app group containers on iOS) · SQLite inspector with paged rows and read-only
queries · SharedPreferences, property list, JSON, image, text and hex viewers · `emulabzip perf`, `mem`, `heapdump`,
`files`, `pull` and `db`. See [Performance](performance.md), [Memory](memory.md) and [Storage](storage.md).

## ✅ Phase 3: Network (v0.3)

A standalone HTTP(S) proxy written in Rust (`emulabzip-net`) that decrypts HTTP/1.1 and HTTP/2 · one-click routing of
an Android device (`adb reverse` and the global proxy setting) or of the iOS Simulator (the Mac's proxy, saved and
restored exactly) · certificate installation: the system store on rootable emulators, a guided user certificate
elsewhere, `simctl keychain` on simulators · request list with filters, headers, decoded bodies and timing · hosts that
reject the certificate are passed through · WebSocket and other connections tunnelled and listed · HAR export ·
`emulabzip net` and `emulabzip ca`. See [Network](network.md).

## ✅ Phase 4: Tracing (v0.4)

Perfetto system traces with presets (CPU and scheduling, frames and jank, memory, custom) · a summary and a SQL console
backed by the official `trace_processor`, downloaded once, pinned and verified · CPU flame graphs from `simpleperf`
(an app or the whole system) · Instruments recordings with `xctrace`, with a flame graph for Time Profiler · open
traces in the Perfetto UI or in Instruments · a library of recordings · `emulabzip trace`. See [Traces](traces.md).

## ✅ Phase 5: Crashes and automation (v0.5)

Crash, native crash and ANR collection (DropBox on Android, `.ips` reports on the iOS Simulator) with stacks, the facts
the device wrote down, hints for symbolication, and a notification when a new one happens · controls for location,
appearance, the status bar, battery, permissions, notifications and links · an **MCP server** so AI assistants can read
devices, logs, crashes and traces and act on them · plugins: your own collectors as programs that print JSON lines,
enabled by the user. See [Crashes](crashes.md), [Controls](controls.md), [MCP server](mcp.md) and
[Plugins](plugins.md).

## ✅ Phase 6: Network power tools, layout and sessions (v0.6)

**Mocks, delays, blocks and breakpoints** on requests and responses · **WebSocket messages**, including
`permessage-deflate` · **packet capture import** (`emulator -tcpdump`, `.pcap` and `.pcapng`) with HTTP, TLS server names
and DNS · **HTTP/3** from the proxy to servers · **layout inspection** and an **accessibility audit** (labels, touch
targets, contrast) on Android and the iOS Simulator · **sessions** recorded to a single file, and **CI mode** that
records around a test run · **comparing two devices**. See [Mocks](mocks.md), [WebSockets](websockets.md),
[Capture import](pcap.md), [Layout](layout.md), [Sessions](sessions.md), [CI mode](ci.md) and [Compare](compare.md).

## Phase 7: Performance and quality (v0.7) ✅

**App start time**, cold, warm and hot: `am start -W` and the `Displayed` lines on Android, `simctl launch` with the
unified log's launch signposts on iOS, repeated N times with the median shown · **energy** per app: `dumpsys
batterystats` (CPU, wakelocks, radio, GPS) on Android, the thermal state and CPU time on the simulator · a **leak
finder** for `.hprof` heap dumps: retained sizes, dominators, and destroyed Activities and Fragments that are still
reachable, with the path to the GC root · **performance budgets** in an `emulabzip.toml` (start time, memory peak,
janky frames, request count) that `emulabzip ci` checks, failing the run when one is broken · **baselines**: compare a
run with the last good one and say what got slower.

## Phase 8: Inside the frameworks (v0.8) ✅

**Flutter**: the Dart VM service of debug builds (widget tree, frame timings, memory) · **React Native**: Hermes and the
React tree through the debugger protocol · **Jetpack Compose and SwiftUI** in Layout through their semantics trees, with
recomposition counts where Android exposes them · network decoding for **gRPC and protobuf** (with `.proto` files, or
raw) and **GraphQL**, with operations named in the list · **edit and resend** a request · **network conditions** in the
proxy: presets for 3G, slow Wi-Fi, high latency, offline and packet loss.

Left for later from this phase: **recomposition counts** (Android Studio gets them from an agent loaded into the app,
which uses interfaces that are not public), SwiftUI's own view type names (accessibility does not expose them), and
recording the network conditions in a session.

## Phase 9: Readable stacks and one-click bug reports (v0.9) ✅

**Symbolication** of crashes and stacks: R8 and ProGuard retrace from `mapping.txt`, dSYMs with `atos` on a Mac, and
Hermes and Metro **source maps**, so React Native JavaScript stacks point at the right file and line · mapping files
kept per app and version, and `--mapping` in the command line · a **bug report** in one click: the session, a
screenshot, the device's details, the filtered logs and the failed requests, packed into one HTML file or opened as an
issue on GitHub, GitLab or Jira.

## Phase 10: Timeline and replay (v0.10) ✅

A **timeline** that lines up touches, log problems, requests, slow frames, garbage collections and crashes on one time
axis, on a clock measured against the device · **screen video** recorded into a session in short pieces (`screenrecord`
on Android, `simctl io recordVideo` on iOS), in step with the timeline in the desktop app, and the clip around the crash
added to bug reports · **network replay**: a session or a HAR file answers the app's requests, so it runs offline against
recorded responses, for bugs whose backend has since changed and for deterministic CI runs (`--replay`,
`--fail-on-miss`).

Left for later from this phase: touches on iOS (the simulator gives them to nothing outside it), per-frame jank on iOS,
replaying WebSockets and gRPC calls (a HAR file does not hold their messages and trailers), and matching a request
that carries a timestamp or a random id.

## ✅ Phase 11: App state as data (v0.11)

**[App state snapshots](app-state.md)**: save the whole sandbox to one `.ezstate` file and restore it (`run-as` and tar for
debuggable Android apps, the data container on the simulator), with an automatic backup of what a restore replaces ·
**[emulator snapshots and the device lifecycle](devices.md)**: create, start, stop, wipe and delete AVDs and simulators,
and `avd snapshot save|load|delete` · a **[write mode](storage-editing.md)** in Storage for SQLite rows and
SharedPreferences (property lists from the command line), behind a confirmation, with Undo, and read-only staying the
default · viewers for **DataStore, MMKV, AsyncStorage, Core Data and Realm** (with a reader of Realm's file format) · a
**schema diff** between two states, for testing migrations · **[React Native state](react-native-state.md)**: Redux, React
Query, context and registered stores, with a Redux action log.

Limits: Realm sets, dictionaries, mixed, decimal and UUID columns are not read; MMKV types are guessed; Core Data
attribute names lose their case; AsyncStorage and Core Data from an iOS device were tested on fixture files, not on a
simulator; Flutter state is not read.

## ✅ Phase 12: Quality beyond performance (v0.12)

A **[security and privacy audit](security-audit.md)** while the app runs: cleartext traffic, tokens and personal data in logs and
preferences, exported components (`dumpsys package`), the `debuggable` and `allowBackup` flags, App Transport Security
exceptions in `Info.plist` and domains missing from the privacy manifest · an **[APK, AAB and IPA analyzer](package-size.md)**: size by
folder, dex and assets, a diff between two builds, and a size budget in `emulabzip.toml` · **[localization checks](localization.md)**:
pseudo-locales (`en-XA`, `ar-XB`), font scale and display size controls, a sweep across languages and sizes, and text that
does not fit found through Layout · **more accessibility checks**: focus order and colour blindness simulation (text
scaling through the sweep) · **[visual regression](visual-regression.md)**: screenshots compared with a baseline, with a
difference picture and a check in CI · an **[analytics inspector](analytics.md)**: Segment, Amplitude, Mixpanel and Firebase
events decoded by the proxy, and Firebase's debug log.

Limits: the audit cannot see the `exported` attribute (`dumpsys` does not print it) and judges text by pattern; the size
analyzer reads sizes, folders and dex headers, not bytecode, and not an installed app yet; cut-off text is a heuristic;
Android changes the language per app, not per device; the Firebase decoder was tested on payloads made to Google's layout,
not on live traffic; there is no analytics tool over MCP.

## Phase 13: Automation and hybrid apps (v0.13)

**WebViews** on Android through the Chrome DevTools Protocol (`webview_devtools_remote` sockets forwarded with `adb`),
with their console and network inside EmuLabZip, and a shortcut to Safari's Web Inspector on iOS · **input** on Android
(tap, swipe and text through `adb input`) and **recording a flow** from `getevent`, exported as a Maestro flow or a
command-line script; iOS has no public interface for touches, so this stays Android only · a **deep link tester** with
history, and checks for `assetlinks.json` and `apple-app-site-association` (`pm get-app-links` on Android) ·
**programmable network rules**: map local and map remote, responses checked against an OpenAPI file, and rule scripts
in sandboxed WebAssembly.

## Phase 14: AI with context (v0.14)

The timeline, network replay, state snapshots and input as **MCP tools** · **MCP prompts** for the common questions:
explain this crash, why this screen got slow, summarize this session · an assistant that reproduces a bug through
input, records it as a session and opens the issue, with every action shown to the user before it runs.

## Phase 15: Devices and platforms (v0.15)

**Physical iPhones and iPads** through the tools Xcode ships (`devicectl`, `usbmuxd`): logs, apps, screenshots, crashes
and files where iOS allows it, and a plain note where it does not · **Android over Wi-Fi**, with wireless debugging
pairing from the app · **Wear OS and Android TV** emulators, and **watchOS, tvOS and visionOS** simulators, with the
views that apply to each · **several devices at once**: logs and requests from two devices in one list, tagged by device.

## Phase 16: Team and ecosystem, 1.0 (v1.0)

**Comparing sessions** side by side, and a **shareable HTML report** of a session (one file, no server) · **shared rule
libraries**: a rules file kept in the app's repository, loaded by the app and the command line · a **VS Code extension**
with devices, logs and the MCP server in the editor · a **plugin catalog**: community plugins listed with checksums, still
enabled one by one by the user · an **interface in Portuguese** and other languages · **updates**: signed auto-update in
the app, and Homebrew, winget and Scoop packages · the **1.0 promise**: the command line's options and JSON, the MCP
tools and prompts, and the `.ezsession` format are stable, and change only with a major version.

## Ideas

What is not in a phase yet: layouts for apps that draw their own UI (games, canvases) · HTTP/3 streaming for very large
responses · plugins that read sessions · an optional **companion SDK** (npm, Gradle, SwiftPM) for what only code inside
the app can see, such as Redux, Zustand or React Query state, recomposition counts and feature flags, kept apart from
the default, which installs nothing on the device.
