#!/usr/bin/env bash
# Builds a tiny debuggable app that crashes on purpose, installs it on the running emulator and
# prints how to trigger each kind of failure. Used to try and test the Crashes tool.
#
#   tools/crash-app/install.sh            # uses the only running device
#   ANDROID_SERIAL=emulator-5556 tools/crash-app/install.sh
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
package=dev.emulabzip.crashy
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "› compiling with $(basename "$build_tools") and $(basename "$platform")"
mkdir -p "$work/classes" "$work/dex"
"$javac" --release 8 -Xlint:-options -cp "$platform/android.jar" -d "$work/classes" "$here/src/MainActivity.java"
"$build_tools/d8" --min-api 24 --lib "$platform/android.jar" --output "$work/dex" "$work"/classes/dev/emulabzip/crashy/*.class
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
  --out "$work/crashy.apk" "$work/aligned.apk"

echo "› installing"
"$adb" install -r "$work/crashy.apk" >/dev/null

cat <<MSG
✓ $package is installed. Trigger a failure with:

  adb shell am start -n $package/.MainActivity --es mode crash     # an uncaught exception
  adb shell am start -n $package/.MainActivity --es mode native    # a native crash (SIGSEGV)
  adb shell am start -n $package/.MainActivity --es mode anr       # then: adb shell input tap 300 300

Remove it with: adb uninstall $package
MSG
