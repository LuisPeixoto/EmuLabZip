# Comparing two devices

**Compare** puts two devices side by side while you use your app on both: an old Android next to a new one, a Pixel next
to an iPhone, a phone next to a tablet.

Start a second emulator or simulator and it shows up in the **against** picker. The view then has:

- **Both screens**, refreshed every few seconds.
- **CPU and memory charts** for both at once, for the app picked in the toolbar when it runs on both (device-wide
  otherwise), so you can see which one works harder.
- **Differences**: every fact the two devices report (OS, build, hardware, display, battery, locale) lined up, with what
  differs highlighted. Tick **Show what matches** for the full list.

From a terminal:

```bash
emulabzip compare emulator-5554 "iPhone 17"          # only what differs
emulabzip compare emulator-5554 emulator-5556 --all  # everything
emulabzip compare emulator-5554 emulator-5556 --json
```

Facts one device does not have show as `—`. Comparing an Android device with an iOS one is allowed, though most rows
will differ because the platforms name things differently.
