# Command line

The `emulabzip` command gives you the same data as the desktop app, in a terminal or a script. It is a single static
binary with no runtime dependencies besides `adb` and Xcode.

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

Pre-built archives for macOS, Linux and Windows are attached to every
[release](https://github.com/LuisPeixoto/EmuLabZip/releases).

## Choosing a device

Every command except `devices` works on one device. When only one is running it is used automatically. Otherwise pass
`--device` (or set `EMULABZIP_DEVICE`) with an adb serial, a simulator UDID, a name, or part of a name:

```bash
emulabzip --device pixel info
emulabzip -d "iPhone 17" logs
```

## Commands

### `emulabzip devices`

```text
PLATFORM  NAME       OS                   STATE   ID
Android   Pixel 9    Android 16 (API 36)  online  emulator-5554
iOS       iPhone 17  iOS 26.2             online  F8CE84C1-04D1-4E33-B52B-02B59E8A09C5
```

`--json` prints the list as JSON. The alias `emulabzip ls` does the same.

### `emulabzip info`

Prints the same groups as the Overview (device, software, hardware, display, battery, locale). `--json` prints an
object of objects.

### `emulabzip apps`

Lists user apps with their version and process id. `--all` adds system apps, `--running` keeps only running ones.

### `emulabzip logs`

Follows the device log, coloured by level, until you press <kbd>Ctrl+C</kbd>.

| Option | Meaning |
|--------|---------|
| `-a, --app <id>` | Only lines from this app. The process id is refreshed every few seconds, so restarts are followed. |
| `-l, --level <level>` | Minimum level: `verbose`, `debug`, `info`, `warn`, `error`, `fatal`, or a letter. |
| `-t, --tag <text>` | Tag contains the text. |
| `-g, --grep <text>` | Message or tag contains the text. |
| `-E, --regex` | Treat `--grep` as a regular expression. |
| `--tail <n>` | Recent lines to print first (Android, default 100). |
| `--json` | One JSON object per line, ready for `jq`. |

```bash
emulabzip logs --app com.example.app --level warn
emulabzip logs -g 'skipped \d+ frames' -E
emulabzip logs --json | jq -r 'select(.level == "error") | .message'
```

### `emulabzip screenshot`

Saves a PNG of the screen. `-o, --output <file>` picks the path; by default the file is named after the device and the
time.

### `emulabzip perf`

Prints one line per sample: app and device CPU, app and device memory, frame rate, janky frames and battery.

| Option | Meaning |
|--------|---------|
| `-a, --app <id>` | App to sample. Without it, only device-wide values. |
| `-i, --interval <ms>` | Time between samples (default 1000, minimum 250). |
| `--json` | One JSON object per sample. |

### `emulabzip mem <app>`

A memory breakdown: PSS by category on Android, footprint by VM region on iOS. See [Memory](memory.md).

### `emulabzip heapdump <app>`

Saves an `.hprof` heap dump (Android, debuggable apps) or a `.memgraph` memory graph (iOS). `-o` picks the file.

### `emulabzip files <app> [path]`

Lists a folder of the app. `--root` picks the storage root (`data` or `external` on Android; `data`, `bundle` or
`group:<id>` on iOS); by default the first readable one is used. See [Storage](storage.md).

### `emulabzip pull <app> <path>`

Copies one file to this computer. `-o` picks the destination.

### `emulabzip db <app> <path>`

Opens a SQLite database read-only. Without options it lists tables and views; `--table` prints rows (`--limit`,
`--offset`); `-q, --query` runs a read-only statement. All of them accept `--json`.

### `emulabzip net`

Runs the proxy, points the device at it and prints each request when it finishes, until <kbd>Ctrl+C</kbd>. The
device is put back when it stops, also after an error. See [Network](network.md).

| Option | Meaning |
|--------|---------|
| `-p, --port <n>` | Port on `127.0.0.1` (default 9090, or `EMULABZIP_PORT`). |
| `--no-connect` | Only run the proxy; do not change any device. |
| `--har <file>` | Save the requests as a HAR file when stopping. |
| `--json` | One JSON object per request. |

### `emulabzip crashes` and `emulabzip crash <id>`

`crashes` lists what the device recorded, newest first, with an id for each; `-a, --app` and `-k, --kind
crash|native|anr` narrow it down. `crash` shows one with its stack and hints: `--all-threads` prints every thread,
`--raw` prints the report as the device wrote it. Both accept `--json`. See [Crashes](crashes.md).

### `emulabzip control`

Changes the device. See [Controls](controls.md).

| Command | What it does |
|---------|--------------|
| `control info [-a <app>]` | What the device supports, its appearance and the permissions of the app. |
| `control location set <lat> <lon>` / `location clear` | Sets or resets the position. |
| `control appearance light\|dark` | Light or dark mode. |
| `control statusbar set [--time 9:41] [--battery 100] [--wifi true] [--cellular true]` / `statusbar clear` | A clean status bar, or the real one back. |
| `control battery set <level> [--charging]` / `battery reset` | Pretends the battery is at a level (Android). |
| `control permission <app> <permission> grant\|revoke\|reset` | An Android permission name, or an iOS service such as `photos`. |
| `control notify [<app>] [--title …] [--body …] [--payload <file>]` | A notification (Android) or a simulated push (iOS). |
| `control open <url>` | Opens a link on the device. |

### `emulabzip plugin`

Runs your own collectors. See [Plugins](plugins.md).

| Command | What it does |
|---------|--------------|
| `plugin list [--json]` | The installed plugins and whether each is enabled. |
| `plugin new <name>` | Starts a plugin from a template. |
| `plugin install <folder> [--force]` | Copies a plugin folder in. It starts disabled. |
| `plugin enable <name>` / `plugin disable <name>` | Allows or stops allowing a plugin to run. |
| `plugin run <name> [-a <app>] [--seconds <n>] [--json]` | Runs a plugin on the device and prints what it reports. |

### `emulabzip mcp`

Serves the devices to an AI assistant over MCP on standard input and output. See [MCP server](mcp.md).

### `emulabzip trace`

Records and analyzes traces. See [Traces](traces.md). <kbd>Ctrl+C</kbd> during a recording stops it early and keeps
what was recorded.

| Command | What it does |
|---------|--------------|
| `trace record` | Records a Perfetto system trace (Android 9+). `-p, --preset cpu\|frames\|memory\|custom`, `--config <file>` for `custom`, `-D, --duration <s>` (default 10), `-a, --app <pkg>`, `-o, --output <file>`, `--open` to show it in the Perfetto UI. |
| `trace cpu` | Samples an app (`-a, --app`) or the whole system (`--system`) with simpleperf and prints the hot functions. `-o` saves the report, `--folded <file>` the folded stacks, `--top <n>` the list length. |
| `trace instruments` | Records an Instruments template from a simulator (`-t, --template`, default Time Profiler) into a `.trace` document. Needs Developer Mode. |
| `trace summary <file>` | CPU, frames, memory and busiest slices of a Perfetto trace. `--json` for scripts. |
| `trace query <file> "<sql>"` | Runs PerfettoSQL on a Perfetto trace. `--json` for scripts. |
| `trace open <file>` | Opens a Perfetto trace in the Perfetto UI. |
| `trace list` | Lists the recordings made in the desktop app. |

`summary` and `query` download the trace processor the first time (see [Traces](traces.md#the-trace-processor)).

### `emulabzip ca install`

Makes the device trust the certificate: the system store on a rootable Android emulator, the keychain on an iOS
simulator, and for other Android devices it copies the file and prints the steps. `emulabzip ca path` prints where
the certificate file is.

## Environment

| Variable | Effect |
|----------|--------|
| `EMULABZIP_DEVICE` | Default for `--device`. |
| `EMULABZIP_LOG=debug` | Verbose diagnostics on stderr. |
| `EMULABZIP_HOME` | Folder for the certificate authority, saved proxy settings, recordings and the trace processor. |
| `EMULABZIP_PORT` | Default for `net --port`. |
| `ADB_SERVER_SOCKET`, `ANDROID_ADB_SERVER_PORT`, `EMULABZIP_ADB` | See [Android](android.md). |
