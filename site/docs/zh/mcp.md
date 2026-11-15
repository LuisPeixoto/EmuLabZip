---
source: 8715ca46d87a
description: 了解如何通过 emulabzip mcp 让 AI 助手查看并操作你的模拟器：连接各类客户端、可用的工具、提示词与资源，以及默认关闭的破坏性操作和安全限制。
---
# MCP 服务器

`emulabzip mcp` 让 AI 助手查看你的模拟器，并与它们协作：读取日志、崩溃和请求，操作屏幕，录制会话，检查构建。它通过标准输入和输出使用 [Model Context Protocol](https://modelcontextprotocol.io)，并与应用和命令行使用相同的代码，因此设备在任何地方的行为都一样。

它提供 **118 个工具**（开启更改后为 130 个）、**8 个提示词**和 **13 个资源**。完整列表由服务器本身生成，因此不会出现偏差，见 [MCP 参考](mcp-tools.md)。

## 连接 AI 助手

桌面应用会在 **Settings, AI assistants** 下显示适合你电脑的确切文本，并带有 Copy 按钮。手动操作：

**Claude Code**

```bash
claude mcp add emulabzip -- emulabzip mcp
```

**Claude Desktop**，在 `claude_desktop_config.json` 中（Settings, Developer, Edit Config）：

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

**Cursor**，在 `~/.cursor/mcp.json` 中（或项目中的 `.cursor/mcp.json`）：与 Claude Desktop 相同的 `mcpServers` 块。

**VS Code**，在 `.vscode/mcp.json` 中：

```json
{
  "servers": {
    "emulabzip": { "type": "stdio", "command": "emulabzip", "args": ["mcp"] }
  }
}
```

任何其他启动 stdio 服务器的客户端都以同样方式工作。如果它不在客户端的 PATH 中，请给 `command` 填写 `emulabzip` 的完整路径。

## AI 助手能做什么

每个工具都接受可选的 `device`（id 或名称）。只有一台设备在运行时可以省略它。

| 请求 | 使用 |
|---------|--------------|
| “屏幕上有什么？它无障碍吗？” | `screenshot`, `get_layout`, `audit_accessibility`, `check_text`, `simulate_color_blindness` |
| “打开应用并登录” | `launch_app`, `tap`, `swipe`, `input_text`, `press_key`, `open_url`, `play_flow`, `flow_create` |
| “为什么崩溃了？” | `list_crashes`, `get_crash`, `symbolicate_stack`, `read_logs`, `watch_start` |
| “为什么这么慢？” | `perf_snapshot`, `record_trace`, `trace_summary`, `measure_startup`, `memory_diff`, `analyze_heap` |
| “它向服务器发送了什么？” | `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_export_har`, `openapi_check` |
| “让后端出错” | `net_rules_set`（模拟、延迟、拦截、映射）, `net_conditions`, `net_replay`, `net_breakpoints` |
| “我使用应用时跟踪日志” | `watch_start` 配合 `logs`, `perf`, `touches`, `webview_console`, `webview_network`, `react_console` 或 `react_actions`，然后 `watch_read` |
| “录制这个，并与上周的对比” | `session_start`, `session_stop`, `session_summary`, `session_timeline`, `session_compare` |
| “数据库里有什么？” | `storage_roots`, `read_database`（只读 SQL）, `read_preferences`, `list_files`, `read_file` |
| “这个构建准备好了吗？” | `audit_security`, `analyze_package`, `package_budget`, `i18n_sweep`, `visual_compare`, `check_link` |
| “把这些写成报告” | `create_bug_report`（用 `without` 排除视频、请求或日志） |

### 实时监视

数据流（日志、性能、触摸、WebView 和 React Native）是*监视*：`watch_start` 返回一个 id，`watch_read` 返回自上次读取以来到达的内容（如果 AI 助手太慢，还会返回错过的内容），`watch_stop` 结束监视。一个监视保留最近 5,000 项，十分钟没有读取会自动结束，同时最多运行八个。

### 网络、会话和会保留的设备状态

服务器在运行期间保持状态：代理、捕获的请求、规则、一次录制和各个监视。AI 助手断开连接时，它对设备所做的一切更改都会恢复：设备的代理设置被清除，正在进行的录制被结束。如果会话异常结束，`net_status` 会显示仍然开启的内容，`net_disconnect_device` 会把它恢复。

网络规则与桌面应用使用的相同（共用一个文件），因此 AI 助手设置的规则会显示在应用中。

## 提示词

提示词是 AI 助手可以作为起点的计划。在客户端中选择一个（在 Claude Code 中，输入 `/` 和提示词的名称）。

| 提示词 | 用途 |
|--------|-----|
| `explain_crash` | 找到最近的崩溃，结合映射读取它，说明什么出了问题以及如何修复 |
| `why_is_it_slow` | 测量、追踪缓慢的部分，并指出原因 |
| `summarize_session` | 讲述一次已录制会话的经过 |
| `reproduce_bug` | 在设备上按照描述操作，录制并写出报告 |
| `debug_request` | 捕获流量并说明某个请求为什么失败 |
| `release_check` | 一次完成安全、无障碍、大小、文字和语言检查 |
| `check_deep_link` | 验证链接并查看它落在哪里 |
| `mock_backend` | 以固定的响应、延迟或错误应答某个主机 |

## 资源

AI 助手也可以不调用工具而直接*读取*这些：`emulabzip://devices`、`sessions`、`flows`、`states`、`links/history`、`rules`、`network/flows` 和 `guide`（按任务说明如何使用服务器），以及 `emulabzip://sessions/{id}`、`emulabzip://flows/{id}`（以及 `/maestro`、`/script`）和 `emulabzip://network/flows/{id}`。地址永远不能指定路径：id 的检查方式与工具对其 id 的检查方式相同。

## AI 助手能做什么，不能做什么

查看类的工具始终开启。**操作**设备的工具（点击、录制、连接代理……）会真正改变设备：这正是服务器的用途。它们受到与应用相同的检查限制（见 [Controls](controls.md#safety)）：指向 `file:` 的链接会被拒绝，值会经过验证，任何内容都不会不加引号地传给 shell。

**破坏或改写数据默认是关闭的。**清除或删除设备、恢复状态、加载或删除模拟器快照、在应用的存储中写入文件、在网页中运行脚本，以及删除会话、流程、映射和基线，完全不会提供。使用 `--allow-writes` 启动服务器后它们才会出现（标记为破坏性），并且**每次调用都会先询问你**：客户端会显示将要发生什么（“Wipe Pixel_Test? All its apps and data are deleted.”），在你接受之前不会运行任何内容。恢复状态会先保存当前的数据。

```bash
claude mcp add emulabzip -- emulabzip mcp --allow-writes
```

- 无法询问你的客户端（没有 MCP *elicitation*）会被拒绝，并说明原因。对于在可丢弃设备上的无人值守运行，`--allow-writes --no-confirm` 会跳过询问；不要在任何有你在意的数据的地方使用它。
- **批准脚本和插件，以及运行 `ci`，永远不会提供**，无论是否使用 `--allow-writes`：它们会运行代码，因此由你自己掌控。`run_plugin` 会拒绝你尚未启用的插件，而运行脚本的网络规则只有在脚本已在应用中获得你的批准时才会被接受。
- 文件只会写入 EmuLabZip 自己的文件夹（`exports`、`reports`、`sessions`、`states`、`flows`、`mappings`）；AI 助手永远不会给出要写入的路径。
- 它不能下载任何内容。`trace_summary` 和 `trace_query` 需要 Perfetto 追踪处理器，只有在你于应用中同意，或运行一次 `emulabzip trace summary` 之后才会下载。
- 它只读取设备公开的内容：你本来就可以浏览的应用文件，并且在 Android 上只限可调试的应用。Storage 工具是只读的（会写入的 SQL 会被拒绝）。
- 它只通过标准输入和输出通信。协议本身没有网络端口。

把 AI 助手当作能使用你终端的同事：不要把它指向有你承受不起被更改的数据的设备。

## 试一试

```bash
# Ask the server what it offers, the way an assistant does
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"try","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | emulabzip mcp
```

日志输出到标准错误（`EMULABZIP_LOG=debug`）；标准输出只承载协议。
