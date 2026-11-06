# SwiftUI demo app (iOS Simulator)

A small SwiftUI app (a title, a counter button, a toggle and a list, each with an accessibility identifier), used to
try EmuLabZip's framework detection and Layout.

```bash
tools/swiftui-app/install.sh
xcrun simctl launch booted dev.emulabzip.swiftdemo
emulabzip frameworks dev.emulabzip.swiftdemo
```

It is built with `swiftc` alone, like `tools/crash-app-ios`: no Xcode project is needed.
