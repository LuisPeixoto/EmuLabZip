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

## Phase 2: Performance, memory and storage (v0.2)

Live CPU, memory, frame rate, janky frames and battery charts for the selected app · memory breakdown (Java, native,
graphics, code, stack) with snapshots and a diff · heap dumps (`.hprof`) for Android · file browser for the app sandbox
(`run-as` for debuggable Android apps, the data container on iOS) · SQLite inspector with paged rows and read-only
queries · SharedPreferences and property list viewers · `emulabzip perf`, `emulabzip mem`, `emulabzip ls`,
`emulabzip pull` and `emulabzip db`.

## Phase 3: Network

A standalone HTTP(S) proxy written in Rust (`emulabzip-net`) · one-click wiring of an emulator
(`settings put global http_proxy`) or simulator (host proxy) and certificate installation
(`simctl keychain add-root-cert`, user CA on Android) · request list, headers, bodies and timings · HAR export ·
`emulator -tcpdump` pcap import.

## Phase 4: Tracing

Perfetto recording with presets (CPU scheduling, frames, memory, battery) · SQL over traces with `trace_processor` ·
CPU flame graphs from `simpleperf` · Instruments traces recorded and exported with `xctrace` · open traces in
ui.perfetto.dev.

## Phase 5: Crashes and automation

ANR, tombstone and crash report collection with symbolication hints · emulator and simulator controls (location, push
notifications, status bar, appearance, permissions) · **MCP server** so AI assistants can read devices, logs and
traces · plugin API for custom collectors and views.

## Phase 6: Ideas

Physical iOS devices · layout inspection · accessibility audits · session recording and sharing · comparing two
devices side by side · CI mode that records a session while UI tests run.
