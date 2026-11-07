# Compose demo app (Android)

A small Jetpack Compose screen (a title, a counter button that recomposes, a switch and a list, each with a test
tag) used to try EmuLabZip's framework detection and Layout. It needs a JDK 17 and the Android SDK, and uses the
Gradle wrapper in this folder.

```bash
tools/compose-app/install.sh
adb shell am start -n dev.emulabzip.composedemo/.MainActivity
emulabzip frameworks dev.emulabzip.composedemo
emulabzip layout
```

`testTagsAsResourceId` is switched on, so Layout shows the tags as ids.

## A release build with R8

```bash
tools/compose-app/install.sh --release
adb shell am start -n dev.emulabzip.composedemo/.MainActivity --es crash 1
```

The release build is minified, so a crash has scrambled names. The `mapping.txt` that R8 writes is in
`app/build/outputs/mapping/release/`. Add it with `emulabzip mappings add` to read the crash.
