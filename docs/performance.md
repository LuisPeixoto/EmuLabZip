# Performance

The Performance tool samples the selected app and the device while the tool is open, and draws the last two minutes
as live charts. Pick the app in the toolbar; without one, only device-wide values are shown.

## What is measured

| Value | Android | iOS Simulator |
|-------|---------|---------------|
| App CPU | `/proc/<pid>/stat` user + system time, as a share of all cores | Host process CPU time (`ps`), as a share of all of the Mac's cores |
| Device CPU | `/proc/stat`, busy time across all cores | — (the simulator shares the Mac's CPU) |
| App memory | Resident set size (`VmRSS`) | Resident set size of the host process |
| Device memory | `MemTotal − MemAvailable` from `/proc/meminfo` | — |
| Frame rate | Frames rendered per second, from `dumpsys gfxinfo` | Phase 4, with Instruments traces |
| Janky frames | Share of janky frames since the previous sample | Phase 4 |
| Battery | Level and temperature from `dumpsys battery` | — |

Each Android sample is **one** `adb shell` call that runs every command at once, so sampling every 500 ms stays cheap.
The app's process id is looked up on every sample: when the app restarts, the charts continue with the new process.

## Reading the charts

- CPU percentages are of the **whole device**, the way Android Studio shows them. On a 4-core emulator, one busy core
  is 25%.
- The frame rate only moves while the app draws. A static screen renders no frames, which is good.
- Janky frames are frames that missed their deadline, as counted by the platform's `gfxinfo`.
- Hover a chart to read exact values. Charts with a single series have no legend; the title names the value.

## Controls

| Control | Effect |
|---------|--------|
| **0.5 s · 1 s · 2 s** | Time between samples. |
| **Pause** | Stops sampling; the charts keep what they have. |
| **Clear** (trash) | Empties the charts. |

Sampling stops when you leave the tool, so it never runs in the background.

## From the command line

```bash
emulabzip perf --app com.example.app
emulabzip perf --app com.example.app --interval 500 --json | jq .appCpu
```
