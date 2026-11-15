---
title: Give Claude, Cursor or Copilot eyes on your emulator with MCP
description: EmuLabZip's MCP server lets an AI assistant read crashes, follow logs and traffic live and drive your emulator's screen. It cannot change data unless you allow it.
tags: ai, mcp, android, devtools
series: EmuLabZip
cover: docs/images/crashes.png
publish_after: 2026-11-19
pair: 03-mcp
lang: en
---

Ask an AI assistant "why does the checkout screen crash?" and it can only guess, because it cannot see your device. The Model
Context Protocol (MCP) fixes that: a server exposes tools, and the assistant calls them. `emulabzip mcp` is such a server for
your Android emulators and iOS simulators.

It offers 118 tools, 8 prompts and 13 resources, and it uses the same code as the desktop app and the CLI, so a device
behaves the same everywhere. This post shows how to connect it, what an assistant can do with it, and the safety rules.

## Connect it

Claude Code:

```bash
claude mcp add emulabzip -- emulabzip mcp
```

Claude Desktop (`claude_desktop_config.json`) and Cursor (`~/.cursor/mcp.json`) use the same block:

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

VS Code (`.vscode/mcp.json`):

```json
{
  "servers": {
    "emulabzip": { "type": "stdio", "command": "emulabzip", "args": ["mcp"] }
  }
}
```

The desktop app shows these with your path filled in, under **Settings, AI assistants**, with a Copy button. The server
speaks only on standard input and output: there is no network port.

![Crashes view of EmuLabZip, the kind of data the assistant reads](docs/images/crashes.png)

## What an assistant can do

Every tool takes an optional `device`, which can be left out when one device is running. Some examples of what you can ask for
and what it uses:

| Ask for | Tools |
|---|---|
| "Why did it crash?" | `list_crashes`, `get_crash`, `symbolicate_stack`, `read_logs` |
| "What is it sending to the server?" | `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_export_har` |
| "Make the backend fail" | `net_rules_set` (mock, delay, block), `net_conditions` |
| "Why is it slow?" | `perf_snapshot`, `record_trace`, `trace_summary`, `measure_startup` |
| "What is in the database?" | `storage_roots`, `read_database` (read-only SQL), `read_preferences` |
| "Open the app and log in" | `launch_app`, `tap`, `swipe`, `input_text`, `press_key` |
| "Record this" | `session_start`, `session_stop`, `session_summary`, `session_timeline` |

Streams are **watches**: `watch_start` returns an id for logs, performance, touches, WebView or React Native output,
`watch_read` returns what arrived since the last read, and `watch_stop` ends it. So the assistant can follow the logs while you
use the app, instead of reading a single snapshot.

The server also keeps state while it runs: the proxy, the captured requests, a recording. When the assistant disconnects,
the device's proxy setting is put back and an open recording is finished.

## Prompts for the usual jobs

Prompts are plans an assistant can start from, and each one names the tools to call: `explain_crash`, `why_is_it_slow`,
`summarize_session`, `reproduce_bug`, `debug_request`, `release_check`, `check_deep_link` and `mock_backend`. In Claude Code you
pick them by typing `/`. The `reproduce_bug` prompt, for example, tells the assistant to start a session recording, say what
it is about to do before each action, drive the app with taps and typing, stop the recording and write a bug report.

## Safe by default

An assistant can look at everything, but it cannot destroy anything unless you say so:

- **Wiping or deleting a device, restoring a state, writing a file in an app's storage and running script in a web page are
  not offered at all.** Start the server with `--allow-writes` and they appear, marked destructive, and each call asks you to
  confirm first, saying exactly what will happen ("Wipe Pixel_Test? All its apps and data are deleted.").
- A client that cannot ask you (it has no MCP elicitation) is refused, with the reason. `--allow-writes --no-confirm` skips
  the question and is meant only for unattended use on a disposable device.
- **Approving scripts and plugins, and running CI commands, are never offered**, with or without `--allow-writes`.
- Storage tools are read-only, and files are only written to EmuLabZip's own folders.

```bash
claude mcp add emulabzip -- emulabzip mcp --allow-writes
```

Treat an assistant like a colleague with your terminal: do not point it at a device that holds data you cannot afford to
change.

## Try it

You need the CLI and a running emulator or simulator:

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- MCP guide: [emulabzip.luispeixoto.dev/docs/mcp/](https://emulabzip.luispeixoto.dev/docs/mcp/)
- Every tool, prompt and resource: [emulabzip.luispeixoto.dev/docs/mcp-tools/](https://emulabzip.luispeixoto.dev/docs/mcp-tools/)
- Website and documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
