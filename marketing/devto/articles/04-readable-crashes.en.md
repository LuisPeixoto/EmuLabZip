---
title: Make obfuscated crashes readable: R8 mappings, React Native source maps and dSYMs
description: Read Android, React Native and iOS crash stacks with the mapping, source map or dSYM of the exact build, and pack a bug report into one HTML file, with EmuLabZip.
tags: android, ios, reactnative, debugging
series: EmuLabZip
cover: docs/images/symbolication.png
publish_after: 2026-11-24
pair: 04-readable-crashes
lang: en
---

A release build hides its names. R8 and ProGuard rename your classes to `a.b.c`. React Native ships one JavaScript bundle
where every position is `1:608617`. An iOS app without symbols has bare addresses. The crash that lands in your inbox tells
you almost nothing, unless you have the file the build made for exactly this purpose and a tool that applies it.

This post shows how EmuLabZip lists the crashes your emulator or simulator recorded, reads them back into what you wrote, and
turns one into a bug report with a single command.

![The same Android crash, readable with its mapping.txt and as the device wrote it](docs/images/symbolication.png)

## Find the crash

Crashes, native crashes and ANRs are collected by the device itself. EmuLabZip lists them, newest first, with an id for each:

```bash
emulabzip crashes --app com.example.shop
emulabzip crash dropbox:data_app_crash:20261023205928
```

Each crash opens with its stack, the facts the device wrote down and hints for what to
try next. An ANR shows the stack of every thread, so you can see what blocked the main one. `--all-threads` does the same for
other crashes, and `--raw` shows the report exactly as the device wrote it. In the desktop app, open **Crashes**: new crashes
appear by themselves while it is open, with a notification.

## Keep the file for the build

A mapping is only right for the build that made it, and reads another build worse than no mapping at all. So EmuLabZip keeps
files per app and version and only applies them to a crash of the same version. You add the file once:

```bash
emulabzip mappings add com.example.shop "1.2.0 (42)" app/build/outputs/mapping/release/mapping.txt
emulabzip crash dropbox:data_app_crash:20261023205928
```

From then on every crash of that build is readable. The version is the one the crash shows: the version name and code on
Android (`1.2.0 (42)`), the short version and build number on iOS. When no file fits, the crash says so and names the version
it needs. The files are copied into EmuLabZip's data folder, so a clean build does not take them away.

What you get depends on the stack:

| What crashed | The file | What you get |
|---|---|---|
| Java or Kotlin, minified with R8 or ProGuard | `mapping.txt` | `Cart.average` in `MainActivity.kt:39`, with inlined calls given back as frames of their own |
| React Native JavaScript (Hermes or JavaScriptCore) | the source map of the bundle | `App.tsx 17:14` instead of `1:608617` |
| An iOS app | its `.dSYM` | function, file and line for each address (on a Mac, using `atos`) |

R8 writes `mapping.txt` to `app/build/outputs/mapping/<variant>/` when `isMinifyEnabled` is on. Keep it for every release you
ship: it is the only way back from the names in a crash.

## Read a stack you were sent

Users and CI logs give you pasted stacks, not live crashes. Pipe them in:

```bash
emulabzip symbolicate stack.txt --mapping mapping.txt
adb logcat -d | emulabzip symbolicate - --mapping mapping.txt
```

Or name the file for a single crash without keeping it: `emulabzip crash <id> --mapping app/build/outputs/mapping/release/mapping.txt`.

## A bug report that someone else can read

A bug report needs the same things every time: what the screen looked like, which device and app, what the log said, which
requests failed, and the readable crash. EmuLabZip packs them into one HTML file:

```bash
emulabzip report --app com.example.shop -o report.html
emulabzip report --app com.example.shop --issue github:owner/repo --open
emulabzip report --session run.ezsession -o report.html
```

The second command also opens a new issue with a summary already filled in, and the third builds the report from a recorded
session, with the clip around the crash. Nothing is sent anywhere: you see the file and you decide where it goes. In the app
it is the **Report a bug** button in the toolbar.

![The Report a bug dialog](docs/images/bug-report-dialog.png)

## Try it

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Crashes guide: [emulabzip.luispeixoto.dev/docs/crashes/](https://emulabzip.luispeixoto.dev/docs/crashes/)
- Readable stacks: [emulabzip.luispeixoto.dev/docs/symbolication/](https://emulabzip.luispeixoto.dev/docs/symbolication/)
- Bug reports: [emulabzip.luispeixoto.dev/docs/bug-reports/](https://emulabzip.luispeixoto.dev/docs/bug-reports/)
- Website and all the documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
