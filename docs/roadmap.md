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

## Phase 6: Ideas

Packet capture (`emulator -tcpdump`) import · WebSocket frames · request breakpoints and mocks · HTTP/3 · physical iOS devices · layout inspection · accessibility audits · session recording and sharing · comparing two
devices side by side · CI mode that records a session while UI tests run.
