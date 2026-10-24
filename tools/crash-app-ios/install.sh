#!/usr/bin/env bash
# Builds a tiny app that crashes on purpose and installs it on the booted iOS Simulator.
# Used to try and test the Crashes tool. Needs Xcode.
#
#   tools/crash-app-ios/install.sh            # the booted simulator
#   SIMULATOR=<udid> tools/crash-app-ios/install.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
device="${SIMULATOR:-booted}"
bundle=dev.emulabzip.crashy
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

arch="$(uname -m)"
app="$work/Crashy.app"
mkdir -p "$app"
echo "› compiling for the simulator ($arch)"
xcrun --sdk iphonesimulator swiftc -target "$arch-apple-ios17.0-simulator" -g -Onone \
  -o "$app/Crashy" "$here/main.swift"
cp "$here/Info.plist" "$app/Info.plist"
codesign --force --sign - "$app"

echo "› installing"
xcrun simctl install "$device" "$app"

cat <<MSG
✓ $bundle is installed. Trigger a failure with:

  xcrun simctl launch $device $bundle --crash        # fatalError, three frames deep
  xcrun simctl launch $device $bundle --abort        # abort()
  xcrun simctl launch $device $bundle --exception    # an uncaught NSException

Remove it with: xcrun simctl uninstall $device $bundle
MSG
