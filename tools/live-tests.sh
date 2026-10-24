#!/usr/bin/env bash
# Runs the opt-in tests that need a real emulator or simulator. They change settings on the device
# and put them back, so use a throwaway emulator, not one with work on it.
#
#   ANDROID=emulator-5554 SIMULATOR=<udid> tools/live-tests.sh
#
# Set ANDROID, SIMULATOR or both. Install the crash apps first and let them fail once
# (see tools/crash-app/README.md and tools/crash-app-ios/README.md).
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -z "${ANDROID:-}" && -z "${SIMULATOR:-}" ]]; then
  echo "Set ANDROID=<adb serial> and/or SIMULATOR=<udid>" >&2
  exit 2
fi

if [[ -n "${ANDROID:-}" ]]; then
  echo "› Android: $ANDROID"
  export EMULABZIP_DEVICE="$ANDROID"
  cargo test -p emulabzip-android --test live -- --ignored --test-threads=1
  cargo test -p emulabzip-cli --test mcp_live -- --ignored --test-threads=1
  cargo test -p emulabzip-cli --test live_session -- --ignored --test-threads=1
  # Chrome has to be past its first run: write its command line, and dismiss the notification prompt once.
  # They also need the internet (example.com, cloudflare-quic.com).
  cargo test -p emulabzip-cli --test live_net -- --ignored --test-threads=1
fi

# HTTP/3 from the proxy to a public server. Needs the internet, not a device.
cargo test -p emulabzip-net --test e2e -- --ignored live_http3

if [[ -n "${SIMULATOR:-}" ]]; then
  echo "› iOS Simulator: $SIMULATOR"
  export EMULABZIP_DEVICE="$SIMULATOR"
  # Layout needs Simulator.app open on the device and the Accessibility permission for this terminal.
  cargo test -p emulabzip-apple --test live -- --ignored --test-threads=1
fi
