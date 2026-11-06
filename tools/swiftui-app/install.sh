#!/usr/bin/env bash
# Builds a small SwiftUI app and installs it on the booted iOS Simulator. Used to try framework detection and Layout.
# Needs Xcode.
#
#   tools/swiftui-app/install.sh            # the booted simulator
#   SIMULATOR=<udid> tools/swiftui-app/install.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
device="${SIMULATOR:-booted}"
bundle=dev.emulabzip.swiftdemo
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

arch="$(uname -m)"
app="$work/SwiftDemo.app"
mkdir -p "$app"
echo "› compiling for the simulator ($arch)"
xcrun --sdk iphonesimulator swiftc -target "$arch-apple-ios17.0-simulator" -parse-as-library -g -Onone \
  -o "$app/SwiftDemo" "$here/main.swift"
cp "$here/Info.plist" "$app/Info.plist"
codesign --force --sign - "$app"

echo "› installing"
xcrun simctl install "$device" "$app"

cat <<MSG
✓ $bundle is installed. Open it and look at it:

  xcrun simctl launch $device $bundle
  emulabzip frameworks $bundle        # SwiftUI
  emulabzip layout                    # the screen's views, marked as SwiftUI

Remove it with: xcrun simctl uninstall $device $bundle
MSG
