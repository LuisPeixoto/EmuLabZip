# Memory

The Memory tool shows where the selected app's memory goes and lets you compare two moments to see what grew.

## Snapshots

Click **Take snapshot** to record a breakdown:

| | Android | iOS Simulator |
|---|---|---|
| Source | `dumpsys meminfo <package>` | `footprint --json <pid>` on the Mac |
| Total | PSS (proportional set size) | Footprint, the number iOS uses to decide when to terminate an app |
| Categories | Java heap, native heap, code, stack, graphics, private other, system | VM regions such as `MALLOC_SMALL`, `__DATA`, `CoreAnimation` |
| Details | Total RSS, swap, and object counts (views, activities, binders, WebViews…) | Clean and reclaimable memory, region count |

Take a snapshot, use the app (open and close a screen a few times), then take another one. The second snapshot is
compared with the previous one automatically; choose any other baseline from the **Compared with** menu. Categories are
listed by the size of their change, with an arrow and a sign so the direction never depends on colour alone.

A category that keeps growing across repeated actions is a good place to look for a leak.

## Heap dumps

| | Android | iOS Simulator |
|---|---|---|
| Button | **Heap dump…** | **Memory graph…** |
| File | `.hprof`, written with `am dumpheap` | `.memgraph`, written with `leaks --outputGraph` |
| Open it with | Android Studio, Eclipse MAT, or [Perfetto](https://ui.perfetto.dev) | Xcode (File › Open), `leaks`, `heap`, `vmmap` |
| Requirement | A **debuggable** app (or an `eng`/`userdebug` image) | None |

On Android the dump is written to `/data/local/tmp`, copied to your computer and removed from the device.

## From the command line

```bash
emulabzip mem com.example.app
emulabzip mem com.example.app --json
emulabzip heapdump com.example.app -o before-login.hprof
```
