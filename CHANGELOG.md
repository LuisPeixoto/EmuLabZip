# Changelog

All notable changes to EmuLabZip are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/LuisPeixoto/EmuLabZip/releases/tag/v0.1.0
