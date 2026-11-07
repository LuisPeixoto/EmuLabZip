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

## A recording to replay

Two buttons fetch a page over plain HTTP, so no certificate has to be installed on the device. **Fetch the time** asks
`http://time.akamai.com/`, which answers with the time in seconds: every answer is different, so a recording and a replay
of it are easy to tell apart. **Fetch another page** asks `http://example.com/`.

```bash
# Record a session with the network, the screen and touches, and tap the buttons while it runs.
emulabzip session record -o run.ezsession --app dev.emulabzip.composedemo --network --video --taps --duration 30
emulabzip session timeline run.ezsession

# Run the app against the recording: the time never changes, and the other page fails with a 504.
emulabzip net --replay run.ezsession
```

The app allows cleartext traffic for this. It is a demo; do not copy that setting into an app you ship.
