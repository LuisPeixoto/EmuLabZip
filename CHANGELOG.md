# Changelog

All notable changes to EmuLabZip are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/LuisPeixoto/EmuLabZip/releases/tag/v0.1.0
