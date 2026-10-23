# How the official tools work

Before writing EmuLabZip we studied how Xcode Instruments and the Android Studio profilers collect their data. The
study was static: we looked at the shipped application bundles, the libraries they link, the classes and packages
inside the JAR files, and the strings in the device-side binaries. Nothing was patched or redistributed. The goal was
to understand **which data sources exist**, and then to pick the ones an open-source tool can use through public,
documented interfaces.

## Xcode Instruments

| Area | What we found |
|------|---------------|
| Languages and UI | Swift and Objective-C, AppKit with SwiftUI, plus private chart and call-tree frameworks |
| Plugins | `.xrplugin` bundles and `.instrdst` instrument packages (Networking, HTTP Tracing, SwiftUI, Hangs, Hitches, System Trace…) |
| Transport | **DTX**, a private RPC protocol, over sockets, Mach ports or shared memory, with compression |
| Device agent | `DTServiceHub` exposes channels such as `sysmontap` (process and system statistics), `coreprofilesessiontap` (kernel trace), `objectalloc`, `vmtracking` and `activitytracetap` (os_log and signposts) |
| Data sources | Kernel tracing (`ktrace`, `kperf`), the unified logging system and signposts, symbolication |
| Network | Connection statistics (`netstats`) and CFNetwork HTTP tracing emitted through the unified log, not a proxy |
| Command line | `xctrace record` and `xctrace export` |

## Xcode and the Simulator

`simctl` drives **CoreSimulator** through an XPC service. It can list and boot devices, run commands inside a
simulator (`simctl spawn … log stream`), capture the screen (`simctl io screenshot`, `recordVideo`), list apps and their
containers, override the status bar, set the location, send push notifications and add root certificates. A simulator
app is an ordinary macOS process, so the host can read its CPU and memory usage directly.

## Android Studio

| Area | What we found |
|------|---------------|
| Platform | IntelliJ Platform on the JetBrains Runtime, Kotlin and Java, Swing with Compose for Desktop for the newer profiler UI |
| Device access | ddmlib and adblib (ADB), and the emulator's gRPC API, discovered from the `avd/running/pid_*.ini` files |
| Profilers | A *transport pipeline*: grpc-java and protobuf between Studio and an on-device daemon |
| Device daemon | `transport` (formerly `perfd`), pushed to `/data/local/tmp/perfd/`. It runs `dumpsys meminfo --checkin`, `dumpsys SurfaceFlinger --latency`, `perfetto`, `simpleperf`, `am dumpheap` and loads a JVMTI agent into debuggable apps |
| Traces | Perfetto (`trace_processor` as a gRPC service), simpleperf reports, ART method traces |
| Memory | `.hprof` heap dumps parsed with Shark, native allocations through heapprofd in Perfetto |
| Network | Formerly bytecode rewriting of OkHttp and `HttpURLConnection` with ASM (`profilers-transform.jar`); today the App Inspection network inspector, which hooks HTTP stacks inside debuggable apps |

## What EmuLabZip uses

We only use **public, documented interfaces**. Apple's DTX protocol and Google's on-device agents are private, change
between releases and are not ours to redistribute.

| Data | Android | iOS Simulator |
|------|---------|---------------|
| Devices | ADB host protocol (`host:devices-l`), spoken directly from Rust | `xcrun simctl list devices -j` |
| Device facts | `getprop`, `wm`, `dumpsys battery`, `/proc` | `simctl` JSON, runtime list, `defaults` inside the simulator |
| Apps | `pm list packages`, `ps` | `simctl listapps`, `launchctl list` inside the simulator |
| Logs | `logcat -v threadtime` | `log stream --style ndjson` inside the simulator |
| Screenshots | `screencap -p` | `simctl io screenshot` |
| CPU and memory (Phase 2) | `/proc/<pid>/stat`, `dumpsys meminfo` | Host process statistics |
| Frames (Phase 2) | `dumpsys gfxinfo framestats` | Signposts and `xctrace` (Phase 4) |
| Traces (Phase 4) | Perfetto, simpleperf | `xctrace record` and `export` |
| Network (Phase 3) | HTTP proxy with a user CA, `emulator -tcpdump` | Host proxy, `simctl keychain add-root-cert` |

This keeps EmuLabZip small, safe to run against any emulator image, and stable across Xcode and Android Studio
updates. It also means nothing is installed on the device: every query is something you could type into
`adb shell` or `xcrun simctl` yourself.
