# Crash app (iOS Simulator)

A tiny app that fails on purpose, used to try and test EmuLabZip's Crashes tool on the iOS Simulator.

```bash
tools/crash-app-ios/install.sh
```

One Swift file, compiled with `swiftc` for the simulator and ad-hoc signed. No Xcode project is needed. Launch it with
an argument:

| Argument | What happens | Shows up as |
|----------|--------------|-------------|
| `--crash` | `fatalError` three calls deep | A crash with a symbolicated stack |
| `--abort` | `abort()` | `SIGABRT` |
| `--exception` | An uncaught `NSException` | An exception with its reason |

Remove it with `xcrun simctl uninstall booted dev.emulabzip.crashy`.
