# Sample app

A tiny **debuggable** Android app with no code, used to try and test EmuLabZip's Storage tools. `run-as` only works
for debuggable apps, and most apps on an emulator are not.

```bash
tools/sample-app/install.sh
```

The script builds the APK with the SDK build tools (`aapt2`, `zipalign`, `apksigner`), installs it on the running
emulator and fills its sandbox with:

| File | Shows |
|------|-------|
| `databases/notes.db` | The SQLite inspector: three tables, a view, 240 rows |
| `shared_prefs/dev.emulabzip.sample_preferences.xml` | The SharedPreferences viewer, with every value type |
| `files/config.json` | The JSON viewer |
| `files/logo.png` | The image preview |
| `cache/session.log` | The text viewer |

Remove it with `adb uninstall dev.emulabzip.sample`.
