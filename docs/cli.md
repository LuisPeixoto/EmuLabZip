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

Saves an `.hprof` heap dump (Android, debuggable apps) or a `.memgraph` memory graph (iOS). `-o` picks the file. With
`--analyze` it also looks for leaks, see [Finding leaks](leaks.md).

### `emulabzip heap analyze <file>`

Looks for leaks in an `.hprof` (any platform) or a `.memgraph` (on a Mac). `--json`; `--top <n>` sets how many objects and
classes to list; `--all-heaps` looks beyond the app's own heap. See [Finding leaks](leaks.md).

### `emulabzip startup <app>`

Times how long the app takes to open. `-k, --kind cold|warm|hot` (default `cold`), `-r, --runs <n>` (1 to 50, default 5),
`--json`. See [App start time](startup.md).

### `emulabzip energy <app>`

Measures the energy the app uses while you use it. `-s, --seconds <n>` (default 30; <kbd>Ctrl+C</kbd> reads what there is),
`--json`. See [Energy](energy.md).

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
| `--rules <file>` | Mock, delay or block requests with the rules in a JSON file. See [Mocks and breakpoints](mocks.md). |
| `--http3` | Talk HTTP/3 to servers that offer it. `--show-h3` also shows apps the offers. |
| `--conditions <preset>` | Slow the network: `fast-3g`, `slow-3g`, `slow-wifi`, `high-latency`, `packet-loss` or `offline`. See [Network conditions](network-conditions.md). |
| `--down <kbps>`, `--up <kbps>`, `--latency <ms>`, `--loss <percent>` | Numbers of your own, or changes to a preset. |

GraphQL operations and gRPC methods and statuses are printed after each request.

`emulabzip net import <file> [--har <file>] [--json] [--dns]` reads a `.pcap` or `.pcapng` capture and prints the
requests it holds, with the DNS lookups when asked. See [Importing a capture](pcap.md).

### `emulabzip layout` and `emulabzip a11y`

`layout` prints the views on the screen as a tree (`--json` for all of it, `--png <file>` to save the screenshot taken
with it, `--depth <n>`, `--app <id>` to mark an iOS screen with its app's framework). `a11y` audits the screen and prints the findings; `--fail-on note|warning|error` exits with 1 when
something at least that serious is found. See [Layout and accessibility](layout.md).

### `emulabzip frameworks <app>`

Lists the UI frameworks the app is built with (Flutter, React Native, Hermes, Jetpack Compose, SwiftUI) and what showed
it. `--json`. See [Inside the frameworks](frameworks.md).

### `emulabzip flutter tree|frames|memory <app>`

Looks inside a Flutter debug or profile app through its Dart VM service. `--uri <address>` gives where it listens when it
is not in the log; `tree` has `--depth <n>`, `frames` has `-s, --seconds <n>` (default 10), `memory` has `--top <n>`.
All accept `--json`.

### `emulabzip rn tree|heap|console|state <app>`

Looks inside a React Native development app through Metro (`--metro <port>`, default 8081). `tree` has `--depth <n>`,
`console` has `-s, --seconds <n>`. `state` reads Redux, React Query, contexts and registered stores; `--watch` follows Redux
actions and prints what changed. See [React Native state](react-native-state.md). All accept `--json`. Metro lets one
debugger connect at a time.

### `emulabzip state save|restore|show|diff`

`save <app> -o file.ezstate [--external]` writes an app's data to one file. `restore <file> --yes` replaces the app's data
with it, after saving what it replaces. `show <file>` says what a file holds. `diff <a> [b]` lists the tables, columns,
indexes and keys that changed between two files, or between one and the app now. See [App state](app-state.md).

### `emulabzip edit`

Changes data on the device: `cell` (`-t <table>`, `--rowid <n>` or `--pk col=value`, `--set`, `--text`, `--null`), `insert`,
`delete`, `pref-set <key> <value> [--as type]`, `pref-remove`, `plist-set <key/path> <value> [--as type]`, `plist-remove`
and `undo <folder>`. Each takes the app and the file, and **every one needs `--yes`**. See
[Changing an app's data](storage-editing.md).

### `emulabzip avd`, `sim` and `snapshot`

`avd` and `sim` have `list`, `options`, `create <name> --image <id> --model <id>`, `boot <device>` (`--headless`, `--cold`,
`--wipe`), `shutdown`, `wipe` (`erase` on `sim`) and `delete`. `snapshot` has `list`, `save`, `load` and `delete` for the
running emulator picked with `-d`. `wipe`, `delete`, `boot --wipe`, `snapshot load` and `snapshot delete` need `--yes`.
`emulabzip devices --all` also lists devices that are not running. See [Devices](devices.md).

### `emulabzip audit security <app>`

Looks for secrets and personal data in the log, preferences and databases, plain HTTP and secrets in requests (`--har`),
the `debuggable` and `allowBackup` flags, components other apps can reach, App Transport Security exceptions and privacy
manifest gaps. `--only manifest,logs,storage,traffic`, `--json`, `--fail-on <note|warning|error>`. Evidence is cut down so a
secret is never repeated. See [Security audit](security-audit.md).

### `emulabzip size show|diff`

`show <file>` breaks an `.apk`, `.aab` or `.ipa` down by kind, native ABI and dex file (`--top`, `--json`); with
`--budget` it holds the build to the `[size]` section of `emulabzip.toml` (`--config`, `--baseline`) and exits 1 over a limit.
`diff <before> <after>` says what changed (`--fail-on-growth <percent>`). See [Build size](package-size.md).

### `emulabzip i18n check|sweep`

`check` looks for text that is cut off, off screen, clipped or not mirrored on the screen as it is (`--rtl`, `--fail-on`).
`sweep` tries it in other languages and text sizes (`--locales en-XA,ar-XB`, `--scales 100,130,200`, `--app`,
`--fail-on-worse`) and puts the device back. See [Language, text size and text that does not fit](localization.md).

### `emulabzip control locale|reset-locale|font-scale|display|reset-display`

Change the language of an app (Android) or of the simulator (iOS), the text size, and the Android display size.
`a11y --simulate <kind> [-o file]` shows the screen as someone with colour blindness sees it.

### `emulabzip visual capture|update|compare|list|diff|session`

Baselines of the screen, and what changed since. `capture` and `compare` take `--status-bar` (a clean 9:41 bar), `--dir`,
`--threshold`; `update` needs `--yes`. `compare` exits 1 over the limit and draws the difference. See
[Visual regression](visual-regression.md).

### `emulabzip analytics`

Runs the proxy and prints the events an app sends to Segment, Amplitude, Mixpanel and Firebase (`--service`, `--port`,
`--no-connect`, `--json`), or with `--firebase-log` Firebase's debug lines from the device log. See
[Analytics inspector](analytics.md).

### `emulabzip session` and `emulabzip ci`

`session record -o run.ezsession [--app <id>] [--network] [--screens <s>] [--duration <s>] [--video [<s>]] [--taps]
[--replay <file> [--passthrough] [--timing]]` records until <kbd>Ctrl+C</kbd>; `--video` records the screen in pieces
(30 s by default, 1 to 180), `--taps` records touches, and `--replay` answers the app's requests from a recording
meanwhile (it needs `--network`). `session show <file> [--json | --markdown]` summarizes a session or the `.partial`
folder of one that was cut short. `session timeline <file> [--lane <name>…] [--json]` prints touches, requests, log
problems, slow frames, garbage collections, crashes, screenshots and video on one time axis, and `session video <file> [-o
<dir>]` writes the pieces of the screen recording out. `ci -o run.ezsession [--app <id>] [--network] [--screens <s>]
[--fail-on-crash] [--summary <file>] [--video [<s>]] [--taps] [--replay <file> [--fail-on-miss]] -- <command…>` records
around a command and exits with its code. See [Sessions](sessions.md), [Timeline and screen video](timeline.md),
[Replaying a recording](replay.md) and [CI mode](ci.md).

### `emulabzip compare <a> <b>`

Lists what differs between two devices (`--all` for what matches too, `--json`). See [Comparing devices](compare.md).

### `emulabzip crashes` and `emulabzip crash <id>`

`crashes` lists what the device recorded, newest first, with an id for each; `-a, --app` and `-k, --kind
crash|native|anr` narrow it down. `crash` shows one with its stack and hints: `--all-threads` prints every thread,
`--raw` prints the report as the device wrote it, and `-m, --mapping <file>` reads it with a `mapping.txt`, a source map
or a `.dSYM` you name, besides the ones kept for its app and version. Both accept `--json`. See
[Crashes](crashes.md) and [Readable stacks](symbolication.md).

### `emulabzip mappings` and `emulabzip symbolicate`

`mappings add <app> <version> <file>` keeps a `mapping.txt`, a source map or a `.dSYM` for a build (`-k, --kind
mapping|sourcemap|dsym` when it cannot be told from the file), `mappings list [app] [--json]` shows what is kept, and
`mappings remove <app> <version> [-k <kind>]` forgets it. `symbolicate <stack.txt|-> -m <file>` reads a stack you
pasted, from a file or standard input. See [Readable stacks](symbolication.md).

### `emulabzip report`

Writes what a bug report needs into one HTML file: `[--session <file>] [-a, --app <id>] [-o <file>] [--title <text>]
[--description <text>] [--without screenshot|logs|requests|crashes|video] [--issue github:owner/repo | gitlab:[host/]group/project |
jira:site/PROJECTID/ISSUETYPEID] [--open]`. With `--issue` it prints a link that opens the issue with a summary filled
in, and `--open` opens it in the browser. See [Bug reports](bug-reports.md).

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
