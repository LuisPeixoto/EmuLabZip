# Changelog

All notable changes to EmuLabZip are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/LuisPeixoto/EmuLabZip/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LuisPeixoto/EmuLabZip/releases/tag/v0.1.0
