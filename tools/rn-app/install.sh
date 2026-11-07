#!/usr/bin/env bash
# Creates a React Native app in tools/rn-app/build/, puts the demo screen in it, builds the debug APK and installs
# it on the running emulator. Needs Node 22+, a JDK 17+, and the Android SDK (ANDROID_HOME). The first run downloads
# a lot (npm packages, Gradle, the React Native and Hermes libraries) and takes a while.
#
#   tools/rn-app/install.sh             # ANDROID_SERIAL picks a device when several run
#   tools/rn-app/install.sh --start     # also start Metro, in the foreground
#   tools/rn-app/install.sh --release   # a release build (Hermes, bundled JS) and its source map, to try crash symbolication
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$here/build"
app="$work/RnDemo"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [ -z "${JAVA_HOME:-}" ] && [ -x "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/java" ]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi
adb_cmd=(adb ${ANDROID_SERIAL:+-s "$ANDROID_SERIAL"})

if [ ! -d "$app" ]; then
  mkdir -p "$work"
  echo "› creating the project"
  (cd "$work" && npx --yes @react-native-community/cli@latest init RnDemo --package-name dev.emulabzip.rndemo \
    --skip-git-init --pm npm --skip-install >/dev/null)
  (cd "$app" && npm install --no-audit --no-fund)
fi
cp "$here/App.tsx" "$app/App.tsx"
echo "sdk.dir=$ANDROID_HOME" > "$app/android/local.properties"

if [ "${1:-}" = "--release" ]; then
  echo "› building the release APK"
  (cd "$app/android" && ./gradlew --console=plain :app:assembleRelease -PreactNativeArchitectures="$("${adb_cmd[@]}" shell getprop ro.product.cpu.abi | tr -d '\r')")
  "${adb_cmd[@]}" install -r "$app/android/app/build/outputs/apk/release/app-release.apk"
  map="$(find "$app/android/app/build" -name 'index.android.bundle.packager.map' -o -name 'index.android.bundle.compiler.map' | head -3)"
  cat <<MSG
✓ the release build is installed. Tap Crash, then:

  emulabzip crash <id> --raw
  emulabzip mappings add dev.emulabzip.rndemo 1.0 <the composed source map>
  emulabzip crash <id>

The source maps are here (the composed one is the one for Hermes):
$map
MSG
  exit 0
fi

echo "› building the debug APK"
(cd "$app/android" && ./gradlew --console=plain :app:assembleDebug -PreactNativeArchitectures="$("${adb_cmd[@]}" shell getprop ro.product.cpu.abi | tr -d '\r')")
"${adb_cmd[@]}" install -r "$app/android/app/build/outputs/apk/debug/app-debug.apk"
# The app finds Metro on the device's own port 8081.
"${adb_cmd[@]}" reverse tcp:8081 tcp:8081

cat <<MSG
✓ dev.emulabzip.rndemo is installed. Start Metro, open the app, then try:

  cd "$app" && npx react-native start
  emulabzip frameworks dev.emulabzip.rndemo
  emulabzip rn tree dev.emulabzip.rndemo
MSG
if [ "${1:-}" = "--start" ]; then
  cd "$app" && exec npx react-native start
fi
