# MCP server

`emulabzip mcp` lets an AI assistant look at your emulators and simulators, and work with them: read logs, crashes and
requests, drive the screen, record a session, check a build. It speaks the
[Model Context Protocol](https://modelcontextprotocol.io) over standard input and output, and uses the same code as the app
and the command line, so a device behaves the same everywhere.

It offers **118 tools** (130 with changes turned on), **8 prompts** and **13 resources**. The full list, written from the
server itself so it cannot drift, is the [MCP reference](mcp-tools.md).

## Connect an assistant

The desktop app shows the exact text for your computer under **Settings, AI assistants**, with a Copy button. By hand:

**Claude Code**

```bash
claude mcp add emulabzip -- emulabzip mcp
```

**Claude Desktop**, in `claude_desktop_config.json` (Settings, Developer, Edit Config):

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

**Cursor**, in `~/.cursor/mcp.json` (or `.cursor/mcp.json` in a project): the same `mcpServers` block as Claude Desktop.

**VS Code**, in `.vscode/mcp.json`:

```json
{
  "servers": {
    "emulabzip": { "type": "stdio", "command": "emulabzip", "args": ["mcp"] }
  }
}
```

Any other client that starts a stdio server works the same way. Give `command` the full path of `emulabzip` if it is not on
the client's PATH.

## What an assistant can do

Every tool takes an optional `device` (an id or a name). When only one device is running it can be left out.

| Ask for | It uses |
|---------|---------|
| "What is on the screen? Is it accessible?" | `screenshot`, `get_layout`, `audit_accessibility`, `check_text`, `simulate_color_blindness` |
| "Open the app and log in" | `launch_app`, `tap`, `swipe`, `input_text`, `press_key`, `open_url`, `play_flow`, `flow_create` |
| "Why did it crash?" | `list_crashes`, `get_crash`, `symbolicate_stack`, `read_logs`, `watch_start` |
| "Why is it slow?" | `perf_snapshot`, `record_trace`, `trace_summary`, `measure_startup`, `memory_diff`, `analyze_heap` |
| "What is it sending to the server?" | `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_export_har`, `openapi_check` |
| "Make the backend fail" | `net_rules_set` (mock, delay, block, map), `net_conditions`, `net_replay`, `net_breakpoints` |
| "Follow the logs while I use the app" | `watch_start` with `logs`, `perf`, `touches`, `webview_console`, `webview_network`, `react_console` or `react_actions`, then `watch_read` |
| "Record this and compare it with last week" | `session_start`, `session_stop`, `session_summary`, `session_timeline`, `session_compare` |
| "What is in the database?" | `storage_roots`, `read_database` (read-only SQL), `read_preferences`, `list_files`, `read_file` |
| "Is this build ready?" | `audit_security`, `analyze_package`, `package_budget`, `i18n_sweep`, `visual_compare`, `check_link` |
| "Write this up" | `create_bug_report` (leave out the video, requests or logs with `without`) |

### Live watches

Streams (logs, performance, touches, WebView and React Native) are *watches*: `watch_start` returns an id, `watch_read` returns
what arrived since the last read (and what was missed if the assistant was slow), and `watch_stop` ends it. A watch keeps the
last 5,000 items, ends by itself after ten minutes without a read, and at most eight run at once.

### Network, session and device state that stays

The server keeps state while it runs: the proxy, the captured requests, the rules, a recording and the watches. Everything it
changed on a device is put back when the assistant disconnects: the device's proxy setting is cleared and an open recording
is finished. If a session ends badly, `net_status` shows what is still on and `net_disconnect_device` puts it back.

The network rules are the ones the desktop app uses (one shared file), so a rule an assistant sets shows up in the app.

## Prompts

Prompts are plans an assistant can start from. Pick one in the client (in Claude Code, type `/` and the prompt's name).

| Prompt | For |
|--------|-----|
| `explain_crash` | Find the latest crash, read it with its mapping, say what failed and how to fix it |
| `why_is_it_slow` | Measure, trace the slow part, and name the cause |
| `summarize_session` | Tell the story of a recorded session |
| `reproduce_bug` | Follow a description on the device, record it and write the report |
| `debug_request` | Capture the traffic and say why a request failed |
| `release_check` | Security, accessibility, size, text and language checks in one pass |
| `check_deep_link` | Verify a link and see where it lands |
| `mock_backend` | Answer a host with a fixed response, delay or error |

## Resources

An assistant can also *read* these without calling a tool: `emulabzip://devices`, `sessions`, `flows`, `states`,
`links/history`, `rules`, `network/flows` and `guide` (how to use the server, by task), plus `emulabzip://sessions/{id}`,
`emulabzip://flows/{id}` (and `/maestro`, `/script`) and `emulabzip://network/flows/{id}`. An address can never name a
path: ids are checked the way the tools check theirs.

## What an assistant can and cannot do

The tools that look are always on. The tools that **act** on a device (tap, record, connect the proxy…) change it for real:
that is what the server is for. They are limited by the same checks as the app (see [Controls](controls.md#safety)): a link
to `file:` is refused, values are validated, and nothing reaches a shell unquoted.

**Destroying or rewriting data is off by default.** Wiping or deleting a device, restoring a state, loading or deleting an
emulator snapshot, writing a file in an app's storage, running script in a web page, and deleting sessions, flows, mappings
and baselines are not offered at all. Start the server with `--allow-writes` and they appear (marked destructive), and
**each call asks you first**: the client shows what will happen ("Wipe Pixel_Test? All its apps and data are deleted.") and
nothing runs until you accept. Restoring a state saves the data as it is now first.

```bash
claude mcp add emulabzip -- emulabzip mcp --allow-writes
```

- A client that cannot ask you (no MCP *elicitation*) is refused, with the reason. For an unattended run on a disposable
  device, `--allow-writes --no-confirm` skips the question; do not use it anywhere you have data you care about.
- **Approving scripts and plugins, and running `ci`, are never offered**, with or without `--allow-writes`: they run code,
  so they stay with you. `run_plugin` refuses a plugin you have not enabled, and a network rule that runs a script is
  accepted only for a script you approved in the app.
- Files are written only into EmuLabZip's own folders (`exports`, `reports`, `sessions`, `states`, `flows`, `mappings`); an assistant
  never gives a path to write to.
- It cannot download anything. `trace_summary` and `trace_query` need the Perfetto trace processor, which is only
  downloaded after you agree in the app or run `emulabzip trace summary` once.
- It reads only what the device exposes: the files of apps you can already browse, and only when they are debuggable on
  Android. Storage tools are read-only (SQL that writes is refused).
- It speaks only on standard input and output. There is no network port for the protocol itself.

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
