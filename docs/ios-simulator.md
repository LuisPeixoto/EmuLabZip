# iOS Simulator

The iOS Simulator is supported on macOS with Xcode installed. On Windows and Linux the iOS backend reports itself as
unavailable and the rest of EmuLabZip works as usual.

## How it connects

EmuLabZip uses the documented `xcrun simctl` tool. Arguments are passed directly to the process, never through a
shell. Instruments uses private frameworks to talk to simulators; EmuLabZip deliberately does not, so it keeps working
across Xcode releases. See [How the official tools work](research/how-the-official-tools-work.md).

## What is collected

| Feature | Source |
|---------|--------|
| Device list | `simctl list devices -j`, booted iOS simulators only |
| Overview | Simulator JSON, `simctl list runtimes -j`, `defaults read -g AppleLocale` / `AppleLanguages` inside the simulator, `sw_vers` and `xcodebuild -version` on the host |
| Apps | `simctl listapps` (converted with `plutil`), process ids from `launchctl list` inside the simulator |
| Logs | `simctl spawn <udid> log stream --style ndjson --level debug` |
| Screenshots | `simctl io <udid> screenshot` |

## The unified log is busy

A simulator easily writes several thousand lines per second at the debug level. EmuLabZip batches lines every 100 ms
and keeps a bounded buffer (50,000 lines by default), so the interface stays responsive. To see less:

- Pick your app and turn on the app filter in Logs.
- Raise the minimum level to **I** or **W**.
- Turn off *Verbose and debug lines* in Settings to ask `log stream` for info and above only.

iOS log lines carry the **process name** as their tag and the **subsystem and category** (for example
`com.apple.network:connection`), which you can see by clicking a line.

## Limitations

- `log stream` has no history, so the log starts when the device is selected.
- Physical iOS devices are not supported yet (see the [roadmap](roadmap.md)).
