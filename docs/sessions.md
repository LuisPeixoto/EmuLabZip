# Sessions

A **session** is a recording of what happened on a device while an app was used, kept as one file you can open
later, attach to a bug report, or collect from CI.

| Inside | What |
|--------|------|
| `session.json` | The device, the app, when it started and ended |
| `logs.jsonl` | Every log line (up to 500,000) |
| `perf.jsonl` | CPU, memory, frame rate and battery samples, one per second |
| `network.har` | The requests, when the network was recorded, with WebSocket messages |
| `crashes/` | Every crash, native crash and ANR that happened *during* the run, with its report |
| `screens/` | Screenshots taken every few seconds, when asked, with `index.jsonl` saying when |
| `input.jsonl` | Touches (taps, long presses, swipes), when asked, on Android |
| `frames.jsonl` | Frames the app drew too slowly, on Android |
| `video/` | The screen recording in pieces, when asked, with `index.jsonl` saying what each covers |

`session.json` also holds the device's **clock**, so the streams can be put on one axis: see
[Timeline and screen video](timeline.md).

A `.ezsession` file is a zip of plain JSON, a HAR file, PNGs and MP4s, so other tools can read it too. The layout is at
version 2; a version 1 file from EmuLabZip 0.9 or earlier still opens.

## Recording

In the app, open **Sessions**, pick the app in the toolbar, and press **Record**. Optionally tick **Network** (the device
has to be connected in the [Network](network.md) view first), **Video** and **Touches**, and choose how often to take a
screenshot. Press
**Stop and save**; the session opens right away. Sessions recorded here are listed on the left.

On the command line:

```bash
emulabzip session record -o run.ezsession --app com.example.app --screens 5 --network
# ... use the app, then Ctrl+C. Or add --duration 60 to stop by itself.
emulabzip session record -o run.ezsession --app com.example.app --video --taps   # the screen, and touches (Android)
```

With `--network`, the device is pointed at the proxy for the run and put back afterwards, even if something fails.

If a recording is killed (a crash, a closed laptop), what was written is still there in a folder called
`run.ezsession.partial`. Open it like a file; its report says "cut short".

## Reading

```bash
emulabzip session show run.ezsession            # for people
emulabzip session show run.ezsession --json     # for scripts
emulabzip session show run.ezsession --markdown # for a pull request comment
```

The report has the duration, log errors and the most repeated ones (numbers are ignored, so "retry 3 of 5" and "retry 4
of 5" count as one), crashes, requests with the slowest, and CPU and memory peaks.

In the app the open session has tabs: **Overview**, **Logs** (filter by level and text), **Requests** (headers and body
of each), **Timeline** (touches, requests, log problems, slow frames, garbage collections and crashes on one axis, with the video
that follows it, and the CPU and memory charts below) and **Screens**. **Replay network** answers an app's requests from
the recording: see [Replaying a recording](replay.md).

A session is also the easiest way to report a bug later: **Bug report** in the open session, or `emulabzip report
--session run.ezsession`, packs it into one HTML file. See [Bug reports](bug-reports.md).

```bash
emulabzip session timeline run.ezsession        # everything on one axis
emulabzip session video run.ezsession -o clips/ # the pieces of the screen recording
```

An assistant can read sessions with the `list_sessions`, `session_summary` and `session_timeline` tools of the
[MCP server](mcp.md).

## Good to know

- Sessions can contain what your app logs and sends: tokens, addresses, personal data. A screen recording can show
  anything that was on the screen. Treat them like logs, and look before you attach one to a public issue.
- A session recorded by a newer EmuLabZip (a newer format) is refused with a message asking you to update.
- Session files are only written to places you pick or to EmuLabZip's own `sessions` folder.

See [CI mode](ci.md) to record around a test run.
