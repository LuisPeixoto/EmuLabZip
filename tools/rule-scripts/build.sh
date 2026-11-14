#!/usr/bin/env bash
# Builds the example scripts and copies them to dist/. Needs the wasm32-unknown-unknown target:
#   rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
mkdir -p dist
for name in add_header redact_json flaky; do
  cp "target/wasm32-unknown-unknown/release/$name.wasm" "dist/$(echo "$name" | tr _ -).wasm"
done
ls -l dist
