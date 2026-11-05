# Leak app (Android)

A tiny **debuggable** Android app that leaks an `Activity` on purpose, used to try and test EmuLabZip's leak finder.

```bash
tools/leak-app/install.sh
```

The script compiles one Java class with `javac` and `d8`, packages it with `aapt2`, signs it with your debug keystore and
installs it. No Gradle is needed. Then:

```bash
adb shell am start -n dev.emulabzip.leaky/.MainActivity --es mode leak   # leaks itself, then finishes
emulabzip heapdump dev.emulabzip.leaky --analyze
```

| Mode | What happens | What the leak finder says |
|------|--------------|---------------------------|
| `leak` | Stores the activity in a static field and finishes a moment later | A destroyed `MainActivity` is still alive, held by `MainActivity.kept` |
| none | Shows a screen | Nothing |

Remove it with `adb uninstall dev.emulabzip.leaky`.
