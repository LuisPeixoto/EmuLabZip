#!/usr/bin/env bash
# Builds the Compose demo app and installs it on the running emulator. Needs a JDK 17 and the Android SDK
# (ANDROID_HOME). The first build downloads Gradle and the libraries.
#
#   tools/compose-app/install.sh            # ANDROID_SERIAL picks a device when several run
#   tools/compose-app/install.sh --release  # an R8-minified build, and its mapping.txt, to try crash symbolication
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
cd "$here"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [ -z "${JAVA_HOME:-}" ] && [ -x "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/java" ]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi
echo "sdk.dir=$ANDROID_HOME" > local.properties

if [ "${1:-}" = "--release" ]; then
  echo "› building the release APK (R8)"
  ./gradlew --console=plain :app:assembleRelease
  adb ${ANDROID_SERIAL:+-s "$ANDROID_SERIAL"} install -r app/build/outputs/apk/release/app-release.apk
  cat <<MSG
✓ the release build is installed. Its mapping is app/build/outputs/mapping/release/mapping.txt. Try:

  adb shell am start -n dev.emulabzip.composedemo/.MainActivity --es crash 1
  emulabzip crash <id> --raw                          # scrambled
  emulabzip mappings add dev.emulabzip.composedemo 1.0 app/build/outputs/mapping/release/mapping.txt
  emulabzip crash <id>                                # readable
MSG
  exit 0
fi

echo "› building the debug APK"
./gradlew --console=plain :app:assembleDebug
adb ${ANDROID_SERIAL:+-s "$ANDROID_SERIAL"} install -r app/build/outputs/apk/debug/app-debug.apk

cat <<MSG
✓ dev.emulabzip.composedemo is installed. Open it, then try:

  adb shell am start -n dev.emulabzip.composedemo/.MainActivity
  emulabzip frameworks dev.emulabzip.composedemo      # Jetpack Compose
  emulabzip layout                                    # the composables, with their test tags as ids
MSG
