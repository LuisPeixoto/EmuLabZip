---
title: I built an open-source debugging lab for Android emulators and iOS simulators
description: EmuLabZip puts logs, network, performance, memory, storage and crashes of your emulators and simulators in one fast Rust app, with a CLI and an MCP server.
tags: android, ios, rust, opensource
series: EmuLabZip
cover: docs/images/overview.png
publish_after: 2026-11-15
pair: 01-introducing
lang: en
---

When I debug a mobile app I end up with six tools open: Logcat, the Instruments window, Android Studio's profilers, a proxy
like Charles, a terminal full of `adb shell` and `xcrun simctl`, and a text editor to read some database I pulled off the
device. Every one of them knows a slice of what the emulator knows, and none of them talk to each other.

So I built **EmuLabZip**: one open-source app that connects to your running Android emulators and iOS simulators and brings
all of that into one window. It is written in Rust, ships as a desktop app (Tauri + Svelte) and as a command line tool, and
nothing gets installed on the device.

![The overview of an Android emulator in EmuLabZip](docs/images/overview.png)

## What it does

You start an emulator or boot a simulator, and it shows up. There is no setup on the device and nothing to add to your app.
From there:

- **Logs** from logcat and the iOS unified log in one table that stays smooth at thousands of lines per second, with level,
  tag, text, regex and per-app filters.
- **Performance and memory**: CPU, memory, frame rate, janky frames and battery for the app you pick, memory snapshots you can
  diff by category, and Android heap dumps with a built-in leak finder.
- **Network**: a debugging proxy that decrypts HTTPS, with mocks, breakpoints, throttling, GraphQL and gRPC decoding.
- **Storage**: browse an app's files and open SQLite, SharedPreferences, property lists, MMKV, Realm and more in place.
- **Crashes**: crashes, native crashes and ANRs with their stacks, readable when you give it your R8 `mapping.txt`, source map
  or dSYM.
- **Sessions**: record a whole run (logs, requests, crashes, screen video, touches) into one file you can open later,
  compare, or collect from CI.

![Live requests of an Android emulator, with the protocol each one used](docs/images/network.png)

## Why Rust, and why no agent on the device

EmuLabZip does not inject anything into your app. It speaks the public interfaces of each platform: for Android it has its
own async client for the ADB host protocol (no need to shell out to `adb` for every call), and for iOS it drives `xcrun
simctl`. The Android and Apple code lives in separate crates behind one `Backend` trait, so the desktop app and the CLI use
exactly the same code.

That also means it keeps working when Xcode or Android Studio update, because it depends on interfaces they have to keep.

## The same data in a terminal

Everything the app shows is also a command. Start a device, then:

```bash
emulabzip devices
emulabzip logs
emulabzip mem com.example.app
emulabzip net --har capture.har
emulabzip crashes --app com.example.app
```

Each command prints tables for people and `--json` for scripts. The same engine runs in CI: `emulabzip ci` records a session
around your test command and exits with its exit code, so a pipeline behaves as before and you get a file to open when
something fails. I will write about that in a later post.

## And it talks to AI assistants

EmuLabZip includes an MCP server (`emulabzip mcp`). Connect Claude, Cursor or VS Code and the assistant can read your
logs and crashes, follow the traffic live, drive the screen and record a session. It cannot wipe or rewrite anything unless
you start the server with `--allow-writes`, and then each change asks you first. There is a post about that too.

## Try it

The desktop app has installers for macOS, Windows and Linux. The command line tool installs with Cargo:

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

Then start an emulator or simulator and run `emulabzip devices`.

- Website and documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Getting started guide: [emulabzip.luispeixoto.dev/docs/getting-started/](https://emulabzip.luispeixoto.dev/docs/getting-started/)
- Source code and releases (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)

If something breaks or is missing, tell me in an issue. It is a one-person project and the roadmap follows what people need.
