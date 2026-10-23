# Troubleshooting

## My Android emulator does not show up

1. Run `adb devices`. If the emulator is not listed there, EmuLabZip cannot see it either. Restart it, or run
   `adb kill-server && adb start-server`.
2. The device picker footer shows **adb unavailable** when `adb` cannot be found. Install the Android SDK platform
   tools, or point `ANDROID_HOME` or `EMULABZIP_ADB` at your SDK.
3. A device listed as **unauthorized** is waiting for you to accept the USB debugging prompt.
4. A custom ADB server port must be set with `ANDROID_ADB_SERVER_PORT` or `ADB_SERVER_SOCKET` for EmuLabZip too.

## My simulator does not show up

- Only **booted iOS** simulators are listed. Check with `xcrun simctl list devices booted`.
- The footer shows **simctl unavailable** when Xcode's command line tools are missing. Run `xcode-select --install`,
  or `sudo xcode-select -s /Applications/Xcode.app` if several Xcode versions are installed.

## The log is empty

- On Android, check that `adb logcat` prints lines.
- Look at the minimum level and the filters; **Reset filters** appears when lines exist but none match.
- The app filter needs the app to be running. Start the app; the filter picks up the new process id automatically.

## The log is too busy

See [The unified log is busy](ios-simulator.md#the-unified-log-is-busy). On Android, raise the minimum level or filter
by tag.

## Screenshots fail

- On Android, `adb exec-out screencap -p > shot.png` must work. Some secure screens (DRM video, banking apps) are
  black by design.
- On iOS, `xcrun simctl io booted screenshot shot.png` must work.

## Getting diagnostics

Start the app or the CLI with `EMULABZIP_LOG=debug` and include the output when you
[open an issue](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=bug_report.yml).
