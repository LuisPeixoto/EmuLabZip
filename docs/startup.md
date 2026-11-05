# App start time

How long an app takes to open, measured the same way every time, repeated a few times so the median means something.

```bash
emulabzip startup com.example.app                 # cold, 5 runs
emulabzip startup com.example.app --kind hot --runs 10
emulabzip startup com.example.app --json
```

| Kind | Android | iOS Simulator |
|------|---------|---------------|
| **cold** | The process is stopped first (`am force-stop`), so the system starts it from nothing | The app is terminated first |
| **warm** | The screen is closed with the back button and the process is kept | Not available: the simulator has no back button, so there is no separate warm start |
| **hot** | The app is sent to the background with the home button | The Settings app is put in front |

## What is measured

- **Android** uses `am start -W`, which reports `TotalTime` (the time until the first frame is drawn), `WaitTime` and the
  `LaunchState`. The launcher activity is found with `cmd package resolve-activity`.
- **iOS Simulator** reads the unified log. SpringBoard logs when it is asked to open an app and again when the app's scene
  is active. The time between the two is what a person waits, animation included. Compare runs with each other, not with
  a phone.

The result lists every run and the median, fastest, slowest and 90th percentile. The median is the number to follow; one
slow run is usually noise.

## Where to find it

- **Desktop**: Performance → **Start time** (kind, number of runs, a bar per run).
- **CLI**: `emulabzip startup`.
- **MCP**: `measure_startup`.
- **CI**: with a `[startup]` table in [`emulabzip.toml`](budgets.md), `emulabzip ci --app …` measures before the command
  and holds the median to `start_time_ms`.

The app is stopped and opened on the device while this runs, so do not run it on a device where that costs you work.
