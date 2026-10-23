#!/usr/bin/env bash
# Builds a tiny debuggable APK with no code, installs it on the running emulator and fills its
# sandbox with a SQLite database, SharedPreferences, JSON and an image. Used to try and test the
# Storage tools, which need a debuggable app for `run-as`.
#
#   tools/sample-app/install.sh            # uses the only running device
#   ANDROID_SERIAL=emulator-5556 tools/sample-app/install.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
build_tools="$(ls -d "$sdk"/build-tools/* | sort -V | tail -1)"
platform="$(ls -d "$sdk"/platforms/android-* | sort -V | tail -1)"
adb="$sdk/platform-tools/adb"
package=dev.emulabzip.sample
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "› building $package with $(basename "$build_tools") and $(basename "$platform")"
"$build_tools/aapt2" link -o "$work/unsigned.apk" --manifest "$here/AndroidManifest.xml" -I "$platform/android.jar"
"$build_tools/zipalign" -f 4 "$work/unsigned.apk" "$work/aligned.apk"

keystore="$HOME/.android/debug.keystore"
if [[ ! -f "$keystore" ]]; then
  keystore="$work/debug.keystore"
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android -alias androiddebugkey \
    -keyalg RSA -validity 10000 -dname "CN=Android Debug" >/dev/null 2>&1
fi
"$build_tools/apksigner" sign --ks "$keystore" --ks-pass pass:android --key-pass pass:android \
  --out "$work/sample.apk" "$work/aligned.apk"

echo "› installing"
"$adb" install -r "$work/sample.apk" >/dev/null

echo "› seeding the sandbox"
python3 "$here/seed.py" "$work/seed"
cp "$repo/apps/desktop/src-tauri/icons/128x128.png" "$work/seed/logo.png"
for f in notes.db settings.xml config.json logo.png; do
  "$adb" push -q "$work/seed/$f" "/data/local/tmp/emulabzip-$f" >/dev/null
  "$adb" shell chmod 644 "/data/local/tmp/emulabzip-$f"
done
"$adb" shell "run-as $package sh -c 'mkdir -p databases shared_prefs files cache \
  && cp /data/local/tmp/emulabzip-notes.db databases/notes.db \
  && cp /data/local/tmp/emulabzip-settings.xml shared_prefs/${package}_preferences.xml \
  && cp /data/local/tmp/emulabzip-config.json files/config.json \
  && cp /data/local/tmp/emulabzip-logo.png files/logo.png \
  && echo started > cache/session.log'"
"$adb" shell rm -f /data/local/tmp/emulabzip-notes.db /data/local/tmp/emulabzip-settings.xml \
  /data/local/tmp/emulabzip-config.json /data/local/tmp/emulabzip-logo.png

echo "✓ $package is ready. Pick it in EmuLabZip and open Storage."
