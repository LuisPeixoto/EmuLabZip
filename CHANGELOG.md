# Changelog

All notable changes to EmuLabZip are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] - 2026-11-07

Phase 10: timeline and replay.

### Added

- **A timeline.** `emulabzip session timeline` and the Timeline tab of the desktop app put touches, log warnings and
  errors, requests, slow frames, garbage collections, crashes, screenshots and video on one axis. Hover to read, click to
  put the video there, drag to move, ⌘ and scroll to zoom; **Open** goes to the request, log line or crash behind an
  event. A run of slow frames is one burst. `session_timeline` gives it to an assistant over MCP.
- **A common clock.** A recording keeps the device's clock, zone and uptime, so streams that stamp time differently line
  up. Session format version 2; version 1 files still open, with their log times estimated.
- **Touches** (`--taps`, Android): taps, long presses and swipes from `getevent`, placed on the screen. **Slow frames**
  from `dumpsys gfxinfo framestats`, and **garbage collections** from ART's log lines, are recorded on Android with no
  option.
- **Screen video** (`--video [seconds]`, and **Video** in the desktop app): `screenrecord` on Android and
  `simctl io recordVideo` on the iOS Simulator, in pieces (30 s by default) that are stored as they finish, so a killed
  recording keeps what it had. `emulabzip session video` writes the pieces out; the desktop app plays them in step with
  the timeline.
- **Video in bug reports.** A report made from a session includes the piece that holds the crash and a strip of the
  last minute, drawn without script. Up to 20 MB the clip is part of the HTML; a bigger one is written next to it as
  `emulabzip-report.mp4`. `--without video` leaves it out.
- **Network replay.** `emulabzip net --replay <file>`, **Network → Replay** and **Sessions → Replay network** answer the
  app's requests from a HAR file or a recorded session. A request that is not in the recording fails with a 504 and is
  listed (`--passthrough` lets it reach the server); a request recorded twice gets the answers in order. HTTP and HTTPS
  work; WebSockets, gRPC, connections the proxy cannot decode and resends are refused so nothing reaches the network.
  `session record` and `ci` take `--replay`, and `ci --fail-on-miss` fails a run that asked for something the recording
  does not hold.
- The Compose demo has a button that fetches over plain HTTP, for the replay and timeline demos.

### Documentation

- New guides: [Timeline and screen video](docs/timeline.md) and [Replaying a recording](docs/replay.md). Updated:
  sessions, network, mocks, CI, command line, MCP, bug reports, architecture, troubleshooting, security and the roadmap.

## [0.9.0] - 2026-11-07

Phase 9: readable stacks and one-click bug reports.

### Added

- **Readable crashes.** A new `emulabzip-symbols` crate restores what a release build hides:
  - Java and Kotlin names from the R8 or ProGuard `mapping.txt`, with inlined calls given back as frames of their own;
  - React Native JavaScript from source maps, including Hermes' composed maps (`1:608617` becomes `App.tsx 17:14`);
  - iOS addresses from the `.dSYM`, with `atos` on a Mac, after checking the UUID.
  Crashes now carry the app's version, the image and address of native frames, and a **JavaScript** thread read from
  React Native's exception text (Android and iOS).
- **Mappings kept per app and version**, and only used for their own version. `emulabzip mappings add|list|remove`,
  `emulabzip crash <id> --mapping <file>`, and `emulabzip symbolicate <stack> --mapping <file>` for a stack from a log
  or CI. In the desktop app, **Crashes** has **Readable** and **Original**, says what was used and what is missing, and
  offers **Add mapping or source map…** and **Add dSYM…**.
- **Bug reports in one click.** `emulabzip report`, **Report a bug** in the desktop app, **Bug report** in an open
  session, and the `create_bug_report` tool write one self-contained HTML file with a screenshot, the device, warnings
  and errors of the log, failed requests and the readable crashes (a crash that repeats is shown once). `--issue
  github:…|gitlab:…|jira:…` builds a pre-filled issue link, with no token stored and nothing sent.
- `get_crash` reads crashes with the stored files and says which it used.
- Test apps: a release build of the Compose demo with R8, the iOS crash app with `--stripped` and its dSYM, and a Crash
  button and release build in the React Native demo.

### Fixed

- Frameworks: the panels kept losing their results whenever the list of devices refreshed.
- Network: a GraphQL body can be edited when a request is sent again.
- Frameworks: the colours in the caption of the frames chart.

### Documentation

- New guides, [Readable stacks](docs/symbolication.md) and [Bug reports](docs/bug-reports.md).
- The roadmap lists Phases 9 to 16, up to 1.0: readable stacks and bug reports, timeline and replay, app state as data,
  quality beyond performance, automation and hybrid apps, AI with context, devices and platforms, and the team and
  ecosystem features that close 1.0.

## [0.8.0] - 2026-11-06

Phase 8: inside the frameworks.

### Added

- **Flutter**: a new `emulabzip-frameworks` crate reads the Dart VM service of debug and profile apps: the widget tree
  (with the file and line of the app's own widgets), the build and raster time of frames, and the Dart heap. The
  address is found in the log, and the port is reached through the device, with nothing forwarded. `emulabzip
  flutter`, **Frameworks** in the desktop app, and the `flutter_*` tools.
- **React Native**: the component tree, Hermes' heap and the console, read through Metro's inspector. `emulabzip rn`,
  **Frameworks**, and `react_tree`.
- **Framework detection** from the files of an app: Flutter, React Native, Hermes, Jetpack Compose (with its version)
  and SwiftUI. `emulabzip frameworks` and `detect_frameworks`.
- **Compose, Flutter and React Native views are marked in Layout** (and SwiftUI, with `layout --app` or an app picked in
  the desktop app).
- **GraphQL and gRPC** in Network: operation names and gRPC methods and status in the list, GraphQL queries with their
  variables, gRPC calls streamed through the proxy with their trailers, and protobuf read without a schema or with your
  `.proto` files.
- **Send again**: Resend and Edit and resend for a captured request.
- **Network conditions**: Fast and Slow 3G, slow Wi-Fi, high latency, packet loss, offline, or numbers of your own, applied
  while connections are open. `emulabzip net --conditions`.
- Demo apps in `tools/` for Compose, Flutter, React Native and SwiftUI.
- `Lab::recent_logs` and `Lab::connect_port` for tools that need the log of a device or one of its ports.

### Changed

- `te: trailers` is no longer removed from requests, which gRPC needs.

## [0.7.0] - 2026-11-05

Phase 7: performance and quality.

### Added

- **App start time**: cold, warm and hot starts, repeated N times with the median, fastest, slowest and 90th percentile.
  Android uses `am start -W`; the iOS Simulator reads when SpringBoard asks for the app and when its scene is active.
  `emulabzip startup`, Performance → Start time, and the `measure_startup` tool.
- **Energy per app**: Android's `dumpsys batterystats` for the app (estimate, parts, wake locks, alarms, jobs, network;
  the battery is always put back); processor time and the Mac's thermal state on the simulator. `emulabzip energy`,
  Performance → Energy, and `measure_energy`.
- **Leak finder** for `.hprof` heap dumps, in a new `emulabzip-heap` crate with no dependencies: retained sizes,
  dominators, a class histogram and destroyed Activities and removed Fragments that are still alive, with the path from a
  GC root. `emulabzip heap analyze`, `emulabzip heapdump --analyze`, Memory → Find leaks, and `analyze_heap`. A `.memgraph`
  is read with the Mac's `leaks` tool.
- **Budgets and baselines**: `emulabzip.toml` limits that `emulabzip ci` holds a run to, and `--baseline` with
  `--fail-on-regression` to say what got worse than the last good session. `emulabzip session compare` does the same for
  two sessions. Sessions record the start time in an optional `startup.json`.
- `tools/leak-app` (Android) and `--leak` in `tools/crash-app-ios`, to try the leak finder.

- A website at https://emulabzip.luispeixoto.dev, in English, Spanish, Portuguese and Chinese, built from `site/` and
  published with GitHub Pages. It scores 100 in Lighthouse (performance, accessibility, best practices and SEO).

### Documentation

- The roadmap lists Phases 7 to 10, up to 1.0: performance and quality, the inside of frameworks, more devices and
  platforms, and the team and ecosystem features that close 1.0.

## [0.6.0] - 2026-10-24

Phase 6: network power tools, layout and accessibility, sessions, comparing devices.

### Added

- **Mocks and breakpoints**: rules that answer a request yourself (status, headers, text or binary body, delay), slow it
  down, block it, or hold the request and/or the response so you can edit it and continue. A Rules dialog with presets
  and "Mock this response" from any request, a drawer for held messages, `emulabzip net --rules file.json`. Held
  messages continue by themselves after two minutes.
- **WebSocket messages**: a frame parser (RFC 6455, fragments, control frames, `permessage-deflate`) feeds a Messages
  tab with direction, text, JSON and hex, and goes into the HAR export as `_webSocketMessages`.
- **Packet capture import**: `.pcap` and `.pcapng` files (such as `emulator -tcpdump` writes) become requests, TLS
  connections named by their server name, and DNS lookups, with TCP streams put back together. In the app (**Import**)
  and as `emulabzip net import`.
- **HTTP/3 to servers**: with **Use HTTP/3 to servers** on, hosts that announce `Alt-Svc: h3` are reached over QUIC,
  with a fallback to TCP. A Proto column shows `h1`, `h2` or `h3`. **Hide HTTP/3 from apps** (on by default) keeps apps
  from trying QUIC around the proxy.
- **Layout and accessibility**: the views on screen with their names, sizes and flags (`uiautomator` on Android, the
  Accessibility API on the iOS Simulator), a Layout view with the screenshot, the tree and properties, and an audit of
  missing labels, touch targets, contrast and duplicate names. `emulabzip layout` and `emulabzip a11y --fail-on`, and the
  MCP tools `get_layout` and `audit_accessibility`.
- **Sessions**: a recorder that writes logs, samples, requests, crashes and screenshots into one `.ezsession` file
  (still readable if the run was killed), a Sessions view, `emulabzip session record` and `session show`, and the MCP
  tools `list_sessions` and `session_summary`.
- **CI mode**: `emulabzip ci -o run.ezsession -- <command>` records around a command, keeps its exit code, writes a
  Markdown report (also to `$GITHUB_STEP_SUMMARY`), and `--fail-on-crash` fails a run whose app crashed.
- **Compare**: two devices side by side with both screens, CPU and memory charts, and a table of what differs;
  `emulabzip compare`.
- Live tests for all of it in `tools/live-tests.sh`, and a capture written by a real emulator as a test fixture.

### Changed

- WebSocket connections are no longer opaque tunnels: the bytes still pass unchanged, and the messages are read from
  them.

## [0.5.0] - 2026-10-24

Phase 5: crashes and automation.

### Added

- **Crashes**: crashes, native crashes and ANRs with their stacks, grouped by thread, with the facts the device wrote
  down and hints for symbolication (`retrace`, `ndk-stack`, `llvm-symbolizer`, `atos`). Android reports come from
  DropBox (no root needed); iOS Simulator reports are the `.ips` files macOS writes. A toast and a sidebar badge tell
  you when a new one happens.
- **Controls**: location (emulators and simulators), light and dark mode, a clean status bar, battery level,
  runtime permissions, notifications (a simulated push on iOS) and links, each with a way back. Every value is checked
  before a device sees it; links that read local files or run script are refused.
- **MCP server** (`emulabzip mcp`): 25 tools that let an AI assistant list devices and apps, read logs, take
  screenshots, measure performance and memory, browse app files, read crashes and traces, record traces, and control
  the device. It speaks on standard input and output only.
- **Plugins**: your own collectors as programs that print JSON lines (metrics, tables, text, logs). A plugin runs only
  after the user enables it, tied to its `plugin.json`, with a timeout and an output limit. `emulabzip plugin new`,
  `install`, `enable`, `run`; a Plugins view in the app; two examples in `examples/plugins`.
- **Command line**: `emulabzip crashes`, `crash`, `control`, `plugin` and `mcp`.
- `tools/crash-app` and `tools/crash-app-ios`: tiny apps that crash, hang and segfault on purpose, and
  `tools/live-tests.sh` to run every test that needs a device.

### Fixed

- Starting the adb server no longer hangs on Windows. The server it leaves behind kept the output of EmuLabZip open, so
  anything reading that output (a script, a pipe) waited forever.
- A plugin run ends when its program exits, even if the program left a background process holding its output, and a
  failing program's error output is no longer lost.

## [0.4.0] - 2026-10-23

Phase 4: tracing.

### Added

- **Traces**: record what a device did, then find out why it was slow, in one library of recordings.
- **System traces** with Perfetto (Android 9+): presets for CPU and scheduling, frames and jank, and memory (with
  native allocations of one app), or your own text configuration. A summary of CPU by process and thread, frame
  percentiles and jank kinds, memory and the busiest slices, and a SQL console (PerfettoSQL).
- The official `trace_processor` is downloaded once, after asking, from the Perfetto release; its SHA-256 is pinned
  and checked. It only listens on `127.0.0.1`.
- **CPU profiles** with simpleperf, for an app or the whole system, drawn as a flame graph (click to zoom, search,
  process and thread pickers, hot functions, folded-stack export).
- **Instruments** recordings with `xctrace` for any template, opened in Instruments, with a flame graph for Time
  Profiler. Needs Developer Mode on the Mac, which EmuLabZip checks first.
- **Open in Perfetto UI** through a one-shot local server; the trace is never uploaded.
- **Command line**: `emulabzip trace record`, `cpu`, `instruments`, `summary`, `query`, `open` and `list`.

## [0.3.0] - 2026-10-23

Phase 3: network.

### Added

- **Network**: a debugging proxy that decrypts HTTP/1.1 and HTTP/2 over TLS, lists every request with its headers,
  decoded bodies (gzip, deflate, Brotli, zstd) and timing, and tunnels everything else. It listens on `127.0.0.1` only.
- **One-click routing**: Android devices are pointed at the proxy with `adb reverse` and the global proxy setting,
  and only the forward EmuLabZip made is removed afterwards. The iOS Simulator is captured by pointing the Mac's
  proxy at EmuLabZip; the settings are saved first and restored exactly, also at the next start if the app was killed.
- **Certificate**: added to the system store of rootable Android emulators (trusted by every app, without stacking
  mounts when repeated), to the keychain of iOS simulators, and walked through step by step for Android images from
  Google Play and physical devices, with the `network_security_config` for debug builds.
- Apps that reject the certificate are passed through after the first failure, and a banner names them.
- WebSocket upgrades and non-HTTP connections are tunnelled and listed.
- HAR 1.2 export.
- **Command line**: `emulabzip net` and `emulabzip ca install` / `path`.
- `EMULABZIP_HOME` moves the certificate authority and saved settings to another folder.

### Changed

- The ADB client restarts `adbd` as root and waits for the device to come back, and retries the brief "device
  offline" answer an emulator gives while it does.

## [0.2.0] - 2026-10-23

Phase 2: performance, memory and storage.

### Added

- **Performance**: live charts of app and device CPU, app and device memory, frame rate, janky frames and battery,
  sampled every 0.5, 1 or 2 seconds. On Android each sample is a single shell call over `/proc`, `dumpsys gfxinfo` and
  `dumpsys battery`; on the iOS Simulator the app's host process is sampled.
- **Memory**: snapshots of PSS by category (`dumpsys meminfo`) on Android and of the footprint by VM region
  (`footprint`) on iOS, with object counts, and a diff between any two snapshots.
- **Heap dumps**: `.hprof` files for debuggable Android apps and `.memgraph` files for simulator apps.
- **Storage**: a file browser for an app's private data (`run-as`, debuggable apps), shared storage, and the data,
  bundle and app group containers on iOS.
- **Viewers**: a read-only SQLite inspector with paging and SQL queries (write-ahead logs included),
  SharedPreferences, property lists, JSON, images, text and a hex view.
- **Command line**: `emulabzip perf`, `mem`, `heapdump`, `files`, `pull` and `db`.
- `tools/sample-app`, a debuggable Android app with demo data for trying the storage tools.

### Fixed

- The Overview no longer queries the device on every refresh of the device list.
- Monospaced numbers can be sized with utility classes again.
- Release builds no longer fail on mismatched Tauri crate and npm package versions, and CI now checks them.

## [0.1.0] - 2026-10-23

Phase 1: connect and observe.

### Added

- **Device discovery** for Android emulators and devices, through a pure-Rust client for the ADB host protocol, and
  for booted iOS simulators, through `xcrun simctl`. Devices appear and disappear on their own.
- **Overview** with OS and build, hardware, display, battery, storage, locale and host facts, a live screen preview
  and click-to-copy values.
- **App picker** with installed apps and the process id of the ones that are running.
- **Log viewer** for logcat and the iOS unified log: level, tag, text and regular-expression filters, a per-app
  filter that follows restarts, pause, line details, plain-text and JSON-lines export, and a virtualized list that
  stays smooth at thousands of lines per second.
- **Screenshots** saved as PNG.
- **Command line**: `emulabzip devices`, `info`, `apps`, `logs` and `screenshot`, with JSON output.
- Light and dark themes, settings and keyboard shortcuts.
- Documentation: getting started, Android, iOS Simulator, logs, CLI, architecture, troubleshooting, distribution, the
  roadmap and a write-up of how Instruments and the Android Studio profilers collect their data.

[Unreleased]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/LuisPeixoto/EmuLabZip/releases/tag/v0.1.0
