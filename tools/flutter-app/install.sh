#!/usr/bin/env bash
# Builds the Flutter demo app in debug mode and installs it. Needs the Flutter SDK ($FLUTTER_HOME or on the PATH).
#
#   tools/flutter-app/install.sh android        # the running emulator (ANDROID_SERIAL picks one)
#   tools/flutter-app/install.sh ios            # the booted simulator (SIMULATOR=<udid> picks one)
set -euo pipefail

platform="${1:-}"
case "$platform" in android | ios) ;; *) echo "usage: $0 android|ios" >&2; exit 2 ;; esac

here="$(cd "$(dirname "$0")" && pwd)"
flutter="${FLUTTER_HOME:+$FLUTTER_HOME/bin/}flutter"
command -v "$flutter" >/dev/null || { echo "Flutter was not found. Set FLUTTER_HOME or put flutter on the PATH." >&2; exit 1; }

# The app is built in a copy, so the repository does not collect generated files.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -R "$here/lib" "$here/pubspec.yaml" "$work/"
cd "$work"
org=dev.emulabzip
"$flutter" create --project-name flutter_demo --org "$org" --platforms "$platform" --no-pub . >/dev/null
# `flutter create` names the application id after the project: dev.emulabzip.flutter_demo. Use a plainer one.
if [ "$platform" = android ]; then
  sed -i.bak 's/dev.emulabzip.flutter_demo/dev.emulabzip.flutterdemo/g' android/app/build.gradle* && rm -f android/app/build.gradle*.bak
  find android -name '*.kt' -o -name AndroidManifest.xml | xargs sed -i.bak 's/dev.emulabzip.flutter_demo/dev.emulabzip.flutterdemo/g'
  find android -name '*.bak' -delete
  echo "› building the debug APK"
  "$flutter" build apk --debug --extra-front-end-options=--track-widget-creation
  adb ${ANDROID_SERIAL:+-s "$ANDROID_SERIAL"} install -r build/app/outputs/flutter-apk/app-debug.apk
  id=dev.emulabzip.flutterdemo
else
  sed -i.bak 's/dev.emulabzip.flutterDemo/dev.emulabzip.flutterdemo/g' ios/Runner.xcodeproj/project.pbxproj && rm -f ios/Runner.xcodeproj/project.pbxproj.bak
  echo "› building for the simulator"
  "$flutter" build ios --debug --simulator --extra-front-end-options=--track-widget-creation
  xcrun simctl install "${SIMULATOR:-booted}" build/ios/iphonesimulator/Runner.app
  id=dev.emulabzip.flutterdemo
fi

cat <<MSG
✓ $id is installed. Open it, then try:

  emulabzip frameworks $id
  emulabzip flutter tree $id
MSG
