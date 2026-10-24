# Crash app (Android)

A tiny **debuggable** Android app that fails on purpose, used to try and test EmuLabZip's Crashes tool.

```bash
tools/crash-app/install.sh
```

The script compiles one Java class with `javac` and `d8`, packages it with `aapt2`, signs it with your debug keystore
and installs it. No Gradle is needed. Then start it with a `mode`:

| Mode | What happens | Shows up as |
|------|--------------|-------------|
| `crash` | Throws an `IllegalStateException` caused by a `NumberFormatException` | A crash with a `Caused by:` chain |
| `native` | Sends itself `SIGSEGV` | A native crash with a tombstone |
| `anr` | Blocks the main thread for a minute; tap the screen afterwards | An ANR |

Remove it with `adb uninstall dev.emulabzip.crashy`.
