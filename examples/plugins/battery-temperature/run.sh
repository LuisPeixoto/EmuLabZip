#!/bin/sh
# Prints the battery temperature in °C every two seconds, as a metric. Android only.
adb="${EMULABZIP_ADB:-adb}"
while true; do
  tenths=$("$adb" -s "$EMULABZIP_DEVICE" shell dumpsys battery | sed -n 's/^ *temperature: *//p' | head -1 | tr -d '\r')
  if [ -n "$tenths" ]; then
    printf '{"type":"metric","name":"battery temperature","value":%s,"unit":"°C"}\n' "$(awk "BEGIN { print $tenths / 10 }")"
  else
    printf '{"type":"error","message":"The device did not report a battery temperature"}\n'
    exit 1
  fi
  sleep 2
done
