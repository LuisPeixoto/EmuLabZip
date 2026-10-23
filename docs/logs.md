# Logs

The log viewer follows **logcat** on Android and the **unified log** on iOS in one table with the same columns:
time, process id, level, tag and message.

## Filters

| Control | What it does |
|---------|--------------|
| **V D I W E F** | Minimum level. Clicking **W** shows warnings, errors and fatal lines. |
| **Tag** | Lines whose tag contains the text (case-insensitive). |
| **Search** | Lines whose message or tag contains the text. Press <kbd>⌘F</kbd> to jump here. |
| **.\*** | Treat the search as a regular expression, for example `skipped \d+ frames`. An invalid expression turns the field red. |
| **App** | Only lines written by the app picked in the toolbar. |

On iOS, `Default` messages are shown as **I** and `Fault` messages as **F**. On Android, `A` (assert) is shown as
**F**.

## Reading

- The list follows new lines while you are at the bottom. Scroll up to stop following; **Jump to latest** brings you
  back.
- **Pause** freezes the view without stopping the stream. Lines that arrive meanwhile appear when you resume.
- Click a line to see the whole message, its thread id and (on iOS) its subsystem. Double-click a line to copy it.

## Export

The download button exports **the lines you currently see**, after filtering, as plain text in logcat's `threadtime`
layout or as JSON lines (one object per line). The CLI can do the same with `emulabzip logs --json`.

## Limits

The viewer keeps the newest 50,000 lines by default; change it in Settings. When lines arrive faster than the
interface can show them, the status bar counts the lines that were dropped instead of hiding the fact.
