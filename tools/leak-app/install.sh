#!/usr/bin/env bash
# Builds a tiny debuggable app that leaks an Activity on purpose, installs it on the running emulator and
# prints how to trigger the leak. Used to try and test the leak finder.
#
#   tools/leak-app/install.sh            # uses the only running device
#   ANDROID_SERIAL=emulator-5556 tools/leak-app/install.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
build_tools="$(ls -d "$sdk"/build-tools/* | sort -V | tail -1)"
platform="$(ls -d "$sdk"/platforms/android-* | sort -V | tail -1)"
adb="$sdk/platform-tools/adb"
javac="${JAVAC:-javac}"
if ! command -v "$javac" >/dev/null 2>&1 && [[ -x "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/javac" ]]; then
  javac="/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/javac"
fi
package=dev.emulabzip.leaky
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "› compiling with $(basename "$build_tools") and $(basename "$platform")"
mkdir -p "$work/classes" "$work/dex"
"$javac" --release 8 -Xlint:-options -cp "$platform/android.jar" -d "$work/classes" "$here/src/MainActivity.java"
"$build_tools/d8" --min-api 24 --lib "$platform/android.jar" --output "$work/dex" "$work"/classes/dev/emulabzip/leaky/*.class
"$build_tools/aapt2" link -o "$work/unsigned.apk" --manifest "$here/AndroidManifest.xml" -I "$platform/android.jar"
(cd "$work/dex" && zip -q "$work/unsigned.apk" classes.dex)
"$build_tools/zipalign" -f 4 "$work/unsigned.apk" "$work/aligned.apk"

keystore="$HOME/.android/debug.keystore"
if [[ ! -f "$keystore" ]]; then
  keystore="$work/debug.keystore"
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android -alias androiddebugkey \
    -keyalg RSA -validity 10000 -dname "CN=Android Debug" >/dev/null 2>&1
fi
"$build_tools/apksigner" sign --ks "$keystore" --ks-pass pass:android --key-pass pass:android \
  --out "$work/leaky.apk" "$work/aligned.apk"

echo "› installing"
"$adb" install -r "$work/leaky.apk" >/dev/null

cat <<MSG
✓ $package is installed. Leak an activity, then dump the heap and look for it:

  adb shell am start -n $package/.MainActivity --es mode leak      # it finishes by itself after a moment
  emulabzip heapdump $package --analyze

Without a mode the app leaks nothing, which is what a clean dump looks like.
Remove it with: adb uninstall $package
MSG
