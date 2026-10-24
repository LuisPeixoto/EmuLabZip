# Getting started

This guide takes you from a fresh install to reading the logs of your app in a couple of minutes.

## 1. Install EmuLabZip

Download the installer for your system from the [latest release](https://github.com/LuisPeixoto/EmuLabZip/releases),
or [build from source](../README.md#build-from-source).

## 2. Have the platform tools ready

| To inspect | You need |
|------------|----------|
| Android emulators and devices | The Android SDK **platform tools** (`adb`). Android Studio installs them. |
| iOS simulators | A Mac with **Xcode** and its command line tools (`xcode-select --install`). |

EmuLabZip finds `adb` on your `PATH`, in `$ANDROID_HOME` / `$ANDROID_SDK_ROOT`, or in the default SDK location
(`~/Library/Android/sdk`, `%LOCALAPPDATA%\Android\Sdk`, `~/Android/Sdk`). It starts the ADB server if it is not
running. See [Android](android.md) and [iOS Simulator](ios-simulator.md) for details.

## 3. Start a device

```bash
# Android
emulator -list-avds
emulator -avd Pixel_9

# iOS
xcrun simctl boot "iPhone 17"
open -a Simulator
```

Open EmuLabZip. Running devices appear in the **device picker** in the top-left corner within a few seconds, and the
first one that is ready is selected automatically.

## 4. Look around

- **Overview** shows what the device is: OS and build, hardware, display, battery, storage and locale, with a live
  preview of the screen. Click any value to copy it.
- **Logs** follows the device log from the moment the device is selected, so history is already there when you open
  it. See [Logs](logs.md).
- **Performance** charts CPU, memory, frame rate and battery live. See [Performance](performance.md).
- **Memory** takes snapshots, compares them and saves heap dumps. See [Memory](memory.md).
- **Storage** browses the app's files and opens databases, preferences and property lists. See [Storage](storage.md).
- **Network** captures the device's HTTP and HTTPS traffic. See [Network](network.md).
- **Traces** records what the system and an app did and shows where the time went. See [Traces](traces.md).
- **Crashes** lists crashes, native crashes and ANRs with their stacks, and tells you when a new one happens. See
  [Crashes](crashes.md).
- **Controls** changes the device's location, appearance, status bar, battery and permissions. See
  [Controls](controls.md).
- **Plugins** runs your own collectors. See [Plugins](plugins.md). To let an AI assistant use your devices, see the
  [MCP server](mcp.md).
- **Screenshot** in the toolbar captures the screen and lets you save it as a PNG.

## 5. Focus on your app

Pick your app in the **app picker** next to the device. Then, in Logs, turn on the app filter to see only the lines
its process wrote. EmuLabZip keeps track of the process id, so the filter keeps working when the app restarts.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| <kbd>⌘1</kbd> … <kbd>⌘6</kbd> | Overview, Logs, Performance, Memory, Storage, Network |
| <kbd>⌘F</kbd> | Search the log |
| <kbd>⌘K</kbd> | Clear the log |
| <kbd>⌘R</kbd> | Look for devices now |
| <kbd>⌘,</kbd> | Settings |

On Windows and Linux use <kbd>Ctrl</kbd> instead of <kbd>⌘</kbd>.
