# Finding leaks in heap dumps

EmuLabZip reads an Android `.hprof` itself and lists what keeps memory alive, with no other tool needed.

```bash
emulabzip heapdump com.example.app --analyze         # take a dump and analyze it
emulabzip heap analyze before-login.hprof            # a dump you already have
emulabzip heap analyze dump.hprof --json --top 20
```

The app has to be **debuggable** (or the device an `eng`/`userdebug` image) to be dumped.

## What it reports

**Suspected leaks**, the cases that are leaks beyond doubt, with the path that keeps each one alive:

- an `Activity` that was **destroyed** (`mDestroyed`), or was **finished** (`mFinished`, reported as only *possible*,
  because the system may not have destroyed it yet) and is still in memory;
- a `Fragment` (`androidx.fragment.app` or `android.app`) with no `mFragmentManager`, which means it was removed.

For each one: its class and address, **how much memory it keeps alive**, and the shortest path from a GC root, for
example:

```text
static field: class com.example.LeakyActivity.kept → com.example.LeakyActivity
```

Read the path from the right: the last line is the leaked object, and the entry before it is what holds it. Weak, soft
and phantom references (`Reference.referent`) are not followed, because they do not keep an object alive.

**Objects that keep the most alive**, by retained size. Retained size is what would be freed if the object were
collected; it is computed from the dominator tree, so shared objects are not counted twice.

**Classes by size**: instance count and shallow size.

By default only the app's own heap is looked at; `--all-heaps` adds the zygote and the system image.

## Try it

`tools/leak-app` is a small debuggable app that leaks an Activity on purpose (see its README):

```bash
tools/leak-app/install.sh
adb shell am start -n dev.emulabzip.leaky/.MainActivity --es mode leak
emulabzip heapdump dev.emulabzip.leaky --analyze
```

## iOS Simulator

A `.memgraph` is read with the Mac's `leaks` tool, so it needs a Mac. The report lists the **retain cycles and leaked
objects** by type, with their sizes (there are no retained sizes). `tools/crash-app-ios --leak` makes a retain cycle to
try it on.

## Where to find it

- **Desktop**: Memory → **Heap dump…** then **Find leaks**, or **Open dump…** for a file on disk. Three tabs: Suspected
  leaks, Dominators and Classes.
- **CLI**: `emulabzip heap analyze`, `emulabzip heapdump --analyze`.
- **MCP**: `analyze_heap` (Android).

Analyzing a 40 MB dump takes a few seconds. A heap dump contains the app's data (see [Security](../SECURITY.md)), so keep
it as private as the app.
