"""Writes the demo files that install.sh copies into the sample app's sandbox."""

import json
import sqlite3
import sys
from pathlib import Path

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)

db = out / "notes.db"
db.unlink(missing_ok=True)
conn = sqlite3.connect(db)
conn.executescript(
    """
    CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL, body TEXT, pinned INTEGER DEFAULT 0, updated_at TEXT);
    CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL, color TEXT);
    CREATE TABLE note_tags (note_id INTEGER REFERENCES notes(id), tag_id INTEGER REFERENCES tags(id));
    CREATE VIEW pinned_notes AS SELECT id, title FROM notes WHERE pinned = 1;
    """
)
tags = ["work", "ideas", "travel", "recipes", "reading", "home"]
colors = ["#6a4dff", "#0891b2", "#059669", "#d97706", "#db2777", "#2563eb"]
conn.executemany("INSERT INTO tags (name, color) VALUES (?, ?)", zip(tags, colors))
titles = [
    "Plan the Q4 launch", "Grocery list", "Book: The Pragmatic Programmer", "Trip to Lisbon",
    "Sourdough starter", "Fix the flaky login test", "Ideas for the onboarding flow", "Renew passport",
    "Weekly review", "Pasta carbonara", "Refactor the cache layer", "Call the plumber",
]
for i in range(240):
    title = titles[i % len(titles)] + ("" if i < len(titles) else f" #{i // len(titles) + 1}")
    updated = f"2026-10-{i % 23 + 1:02d}T{i * 7 % 24:02d}:{i * 13 % 60:02d}:00Z"
    conn.execute(
        "INSERT INTO notes (title, body, pinned, updated_at) VALUES (?, ?, ?, ?)",
        (title, f"Note body for “{title}”.", int(i % 7 == 0), updated),
    )
    conn.execute("INSERT INTO note_tags VALUES (?, ?)", (i + 1, i % len(tags) + 1))
conn.commit()
conn.close()

(out / "settings.xml").write_text(
    """<?xml version='1.0' encoding='utf-8' standalone='yes' ?>
<map>
    <string name="theme">dark</string>
    <boolean name="onboarding_done" value="true" />
    <int name="launch_count" value="42" />
    <long name="last_sync_ms" value="1792773580000" />
    <float name="font_scale" value="1.1" />
    <string name="api_base_url">https://api.example.com/v2</string>
    <set name="enabled_features"><string>offline_mode</string><string>new_editor</string></set>
</map>
"""
)

(out / "config.json").write_text(
    json.dumps(
        {
            "version": 3,
            "syncIntervalMinutes": 15,
            "endpoints": {"api": "https://api.example.com/v2", "cdn": "https://cdn.example.com"},
            "features": ["offline_mode", "new_editor"],
        },
        indent=2,
    )
)
