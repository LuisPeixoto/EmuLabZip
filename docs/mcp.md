# MCP server

`emulabzip mcp` lets an AI assistant read and control your emulators and simulators. It speaks the
[Model Context Protocol](https://modelcontextprotocol.io) over standard input and output, and uses the same code as the
app and the command line, so a device behaves the same everywhere.

## Connect an assistant

Claude Code:

```bash
claude mcp add emulabzip -- emulabzip mcp
```

Claude Desktop, in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

Any other client that starts a stdio server works the same way. Give `command` the full path of `emulabzip` if it is not
on the client's PATH.

## The tools

Every tool takes an optional `device` (an id or a name). When only one device is running it can be left out.

**Read**

| Tool | Returns |
|------|---------|
| `list_devices` | The running emulators, simulators and attached devices |
| `device_info` | Build, hardware, display, battery, memory and locale |
| `list_apps` | Installed and running apps, with versions and process ids |
| `read_logs` | A snapshot of recent log lines, filtered by app, level, tag or text (at most 1,000 lines) |
| `screenshot` | The screen, as an image |
| `perf_snapshot` | A few samples of CPU, memory, frame rate and battery |
| `memory_report` | Where an app's memory goes |
| `list_files`, `read_file` | An app's storage roots, folders and files (text, or base64 for binary; cut at 256 KB) |
| `list_crashes`, `get_crash` | Crashes, native crashes and ANRs, and one with its stacks and hints |
| `list_traces`, `trace_summary`, `trace_query` | Recorded traces, their summary or hot functions, and PerfettoSQL |
| `get_layout` | The views on screen as a tree, with names, sizes and positions |
| `audit_accessibility` | Missing labels, small touch targets, low contrast and duplicate names on the screen |
| `list_sessions`, `session_summary` | Recorded [sessions](sessions.md), and the summary of one |
| `control_info` | What the device supports changing, and an app's permissions |
| `list_plugins` | The installed plugins and whether each is enabled |

**Act**

| Tool | Does |
|------|------|
| `record_trace` | Records a Perfetto trace, a CPU profile or an Instruments template (up to 120 seconds) and keeps it |
| `control_location`, `control_appearance`, `control_status_bar`, `control_battery` | Changes the device, or puts it back |
| `control_permission`, `control_notification`, `open_url` | Grants or revokes a permission, posts a notification or push, opens a link |
| `run_plugin` | Runs a plugin you enabled |

Mistakes come back as tool errors with the same plain messages the app shows, so the assistant can read them and try
again.

## What an assistant can and cannot do

The **Act** tools change the device for real, and they are always on: that is what the server is for. They are limited by
the same checks as the app (see [Controls](controls.md#safety)): a link to `file:` is refused, values are validated,
and nothing reaches a shell unquoted.

- It cannot run a plugin you have not enabled. `run_plugin` refuses it and says so.
- It cannot download anything. `trace_summary` and `trace_query` need the Perfetto trace processor, which is only
  downloaded after you agree in the app or run `emulabzip trace summary` once.
- It reads only what the device exposes to the tools above: the files of apps you can already browse, and only when
  they are debuggable on Android.
- It speaks only on standard input and output. There is no network port.

Treat an assistant like a colleague with your terminal: do not point it at a device that has data you cannot afford to
change.

## Trying it

```bash
# Ask the server what it offers, the way an assistant does
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"try","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | emulabzip mcp
```

Logs go to standard error (`EMULABZIP_LOG=debug`); standard output carries only the protocol.
