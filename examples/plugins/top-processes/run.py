#!/usr/bin/env python3
"""Lists the processes with the largest resident memory on an Android device."""
import json
import os
import subprocess

adb = os.environ.get("EMULABZIP_ADB", "adb")
device = os.environ["EMULABZIP_DEVICE"]
app = os.environ.get("EMULABZIP_APP", "")

out = subprocess.run([adb, "-s", device, "shell", "ps", "-A", "-o", "PID,RSS,NAME"], capture_output=True, text=True, check=True).stdout
rows = []
for line in out.splitlines()[1:]:
    parts = line.split(None, 2)
    if len(parts) == 3 and parts[1].isdigit():
        rows.append((int(parts[0]), int(parts[1]), parts[2].strip()))
rows.sort(key=lambda r: r[1], reverse=True)

print(json.dumps({
    "type": "table",
    "title": "Largest processes by resident memory",
    "columns": ["PID", "Memory (MB)", "Process"],
    "rows": [[pid, round(rss / 1024, 1), name] for pid, rss, name in rows[:15]],
}), flush=True)

if app:
    mine = [r for r in rows if r[2] == app]
    body = f"{app} uses {mine[0][1] / 1024:.1f} MB (pid {mine[0][0]})" if mine else f"{app} is not running"
    print(json.dumps({"type": "text", "title": "Picked app", "body": body}), flush=True)
