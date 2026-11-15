---
source: d9e8d27bf968
description: 介绍会话：把应用使用期间设备上发生的一切录成一个文件，说明其中包含的内容、如何在应用和命令行中录制与读取，以及分享会话文件时需要注意的事项。
title: 会话：把设备上发生的一切录成一个文件
---
# 会话

**会话**是对应用使用期间设备上所发生之事的录制，保存为一个文件，你可以稍后打开、附加到缺陷报告，或从 CI 中收集。

| 内部文件 | 内容 |
|--------|------|
| `session.json` | 设备、应用，以及开始和结束的时间 |
| `logs.jsonl` | 每一行日志（最多 500,000 行） |
| `perf.jsonl` | CPU、内存、帧率和电池的采样，每秒一次 |
| `network.har` | 请求（录制了网络时），包含 WebSocket 消息 |
| `crashes/` | 运行*期间*发生的每次崩溃、原生崩溃和 ANR，附带其报告 |
| `screens/` | 每隔几秒拍摄的截图（按需），由 `index.jsonl` 记录时间 |
| `input.jsonl` | 触摸（点按、长按、滑动）（按需），仅限 Android |
| `frames.jsonl` | 应用绘制过慢的帧，仅限 Android |
| `video/` | 分段的屏幕录像（按需），由 `index.jsonl` 说明每段覆盖的内容 |

`session.json` 还保存设备的**时钟**，因此各个数据流可以放在同一条轴上：参见[时间线与屏幕录像](timeline.md)。

`.ezsession` 文件是由纯 JSON、一个 HAR 文件、PNG 和 MP4 组成的 zip，因此其他工具也能读取它。其布局为版本 2；来自 EmuLabZip 0.9 或更早版本的版本 1 文件仍然可以打开。

## 录制

在应用中，打开 **Sessions**，在工具栏中选择应用，然后点击 **Record**。还可以勾选 **Network**（必须先在 [Network](network.md) 视图中连接设备）、**Video** 和 **Touches**，并选择截图的频率。点击 **Stop and save**；会话会立即打开。在这里录制的会话会列在左侧。

在命令行中：

```bash
emulabzip session record -o run.ezsession --app com.example.app --screens 5 --network
# ... use the app, then Ctrl+C. Or add --duration 60 to stop by itself.
emulabzip session record -o run.ezsession --app com.example.app --video --taps   # the screen, and touches (Android)
```

使用 `--network` 时，设备在运行期间会指向代理，并在之后恢复，即使出了问题也是如此。

如果录制被终止（崩溃、合上了笔记本电脑），已写入的内容仍保存在名为 `run.ezsession.partial` 的文件夹中。像打开文件一样打开它；其报告会写明“cut short”。

## 读取

```bash
emulabzip session show run.ezsession            # for people
emulabzip session show run.ezsession --json     # for scripts
emulabzip session show run.ezsession --markdown # for a pull request comment
```

报告包含时长、日志错误及重复最多的错误（数字会被忽略，因此“retry 3 of 5”和“retry 4 of 5”算作一个）、崩溃、请求及其中最慢的，以及 CPU 和内存的峰值。

在应用中，打开的会话有这些标签页：**Overview**、**Logs**（按级别和文本过滤）、**Requests**（每个请求的请求头和正文）、**Timeline**（触摸、请求、日志问题、慢帧、垃圾回收和崩溃都在同一条轴上，并配有随之播放的视频，下方是 CPU 和内存图表）以及 **Screens**。**Replay network** 会用录制内容应答应用的请求：参见[回放录制](replay.md)。

会话也是日后报告缺陷最简便的方式：在打开的会话中点击 **Bug report**，或使用 `emulabzip report --session run.ezsession`，即可把它打包成一个 HTML 文件。参见[缺陷报告](bug-reports.md)。

```bash
emulabzip session timeline run.ezsession        # everything on one axis
emulabzip session video run.ezsession -o clips/ # the pieces of the screen recording
```

AI 助手可以使用 [MCP 服务器](mcp.md)的 `list_sessions`、`session_summary` 和 `session_timeline` 工具读取会话。

## 须知

- 会话可能包含你的应用记录和发送的内容：令牌、地址、个人数据。屏幕录像可能显示屏幕上出现过的任何内容。请像对待日志一样对待它们，并在把会话附加到公开 issue 之前先检查。
- 由较新版本 EmuLabZip 录制的会话（较新的格式）会被拒绝，并提示你更新。
- 会话文件只会写入你选择的位置或 EmuLabZip 自己的 `sessions` 文件夹。

参见 [CI 模式](ci.md)，了解如何围绕测试运行进行录制。
