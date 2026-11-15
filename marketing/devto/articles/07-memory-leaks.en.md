---
title: Find Android memory leaks and slow frames without opening Android Studio
description: Take memory snapshots and diff them, analyze heap dumps for leaked Activities and record Perfetto traces with flame graphs, all from EmuLabZip's app or CLI.
tags: android, performance, memoryleaks, debugging
series: EmuLabZip
cover: docs/images/memory.png
publish_after: 2026-12-03
pair: 07-memory-leaks
lang: en
---

Memory problems are slow to find because they only show up over time: open a screen five times and something is still alive.
The usual path is to dump the heap, copy a file of tens of megabytes, open it in a profiler and work out what holds what.

EmuLabZip does the same in three steps that each fit in a command: see where an app's memory goes and what grew, analyze a heap
dump for leaks, and, when the problem is time rather than memory, record a trace and read the flame graph.

![The Memory view: a snapshot compared with the previous one](docs/images/memory.png)

## Step 1: see what grew

Take a **snapshot** of the app's memory, use the app (open and close a screen a few times), then take another one. The second
is compared with the previous snapshot automatically, and categories are listed by the size of their change, with an arrow and
a sign so the direction never depends on colour alone.

```bash
emulabzip mem com.example.app
emulabzip mem com.example.app --json
```

On Android a snapshot comes from `dumpsys meminfo`: Java heap, native heap, code, stack, graphics and more, plus object counts
(views, activities, binders, WebViews). On the iOS Simulator it uses `footprint`, the number iOS uses to decide when to
terminate an app. A category that keeps growing across repeated actions is a good place to look for a leak.

## Step 2: analyze a heap dump

```bash
emulabzip heapdump com.example.app --analyze
emulabzip heap analyze before-login.hprof
emulabzip heap analyze dump.hprof --json --top 20
```

The first command takes a heap dump of a running app and analyzes it. EmuLabZip reads the Android `.hprof` itself, so no other
tool is needed. The app has to be debuggable (or the device an `eng` or `userdebug` image).

The report has three parts:

- **Suspected leaks**: an `Activity` that was destroyed (or finished) and is still in memory, and a `Fragment` that was removed
  but is still alive, each with how much memory it keeps alive and the **shortest path from a GC root**, for example
  `static field: class com.example.LeakyActivity.kept → com.example.LeakyActivity`. Read the path from the right: the last line
  is the leaked object and the one before it is what holds it. Weak, soft and phantom references are not followed, because
  they do not keep an object alive.
- **Objects that keep the most alive**, by retained size, computed from the dominator tree so shared objects are not counted
  twice.
- **Classes by size**: instance count and shallow size.

By default only the app's own heap is examined, and `--all-heaps` adds the zygote and the system image. Analyzing a 40 MB dump
takes a few seconds. A heap dump contains the app's data, so keep it as private as the app.

## Step 3: when it is time, not memory

For jank and slow starts, record a trace:

```bash
emulabzip trace record --preset frames --duration 15 -o frames.pftrace
emulabzip trace summary frames.pftrace
emulabzip trace query frames.pftrace "select name, count(*) n from slice group by name order by n desc limit 10"
emulabzip trace cpu --app com.example.app --duration 10 --folded stacks.txt
```

`trace record` makes a Perfetto system trace with four presets: CPU and scheduling, frames and jank, memory, and custom. The
summary shows the trace length, CPU busyness, the number of frames and the frame time at the 50th, 90th and 99th percentile,
CPU time by process and thread, and the kinds of jank per layer. `trace query` runs PerfettoSQL on the trace. `trace cpu` uses
`simpleperf` to get a CPU profile and shows it as a **flame graph** with hot functions, or exports folded stacks for other
tools. On the iOS Simulator the equivalent uses `xctrace` with Time Profiler.

![The summary of a Perfetto frames trace](docs/images/traces-summary.png)

The Perfetto trace processor is downloaded only after you agree, the first time you use a summary or a query.

## Try it

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Memory: [emulabzip.luispeixoto.dev/docs/memory/](https://emulabzip.luispeixoto.dev/docs/memory/)
- Finding leaks in heap dumps: [emulabzip.luispeixoto.dev/docs/leaks/](https://emulabzip.luispeixoto.dev/docs/leaks/)
- Traces and flame graphs: [emulabzip.luispeixoto.dev/docs/traces/](https://emulabzip.luispeixoto.dev/docs/traces/)
- Website and all the documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
