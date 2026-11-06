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
