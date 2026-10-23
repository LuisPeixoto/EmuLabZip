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

## Environment

| Variable | Effect |
|----------|--------|
| `EMULABZIP_DEVICE` | Default for `--device`. |
| `EMULABZIP_LOG=debug` | Verbose diagnostics on stderr. |
| `ADB_SERVER_SOCKET`, `ANDROID_ADB_SERVER_PORT`, `EMULABZIP_ADB` | See [Android](android.md). |
