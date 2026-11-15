---
source: bde0d784ece0
description: 介绍如何把屏幕截图、设备与应用信息、日志、失败的请求和可读的崩溃打包成一个 HTML 文件，并预填缺陷到 GitHub、GitLab 或 Jira，以及分享前的检查事项。
---
# 缺陷报告

每份缺陷报告每次都需要同样的内容：屏幕当时的样子、什么设备和应用、日志说了什么、哪些请求失败了，以及应用是如何崩溃的，并且要可读。EmuLabZip 会把它们打包成**一个 HTML 文件**，并且如果你愿意，还会打开一个已预填摘要的新 issue。

<p align="center">
  <img src="images/bug-report-dialog.png" alt="Report a bug 对话框" />
</p>

不会向任何地方发送任何内容。你能看到这个文件，并由你决定它的去向。

## 快速开始

在桌面应用中，点击工具栏中的 **Report a bug**。写下发生了什么，选择要包含的内容，然后点击 **Save HTML…**。如果你指定了跟踪器，预填好的 issue 会在浏览器中打开，文件会在其所在文件夹中显示，随时可以附加到 issue 上。

在终端中：

```bash
emulabzip report --app com.example.shop -o report.html
emulabzip report --app com.example.shop --issue github:owner/repo --open
emulabzip report --session run.ezsession -o report.html       # from a recording
```

## 文件中有什么

<p align="center">
  <img src="images/bug-report-html.png" alt="在浏览器中打开的报告：截图、设备和可读的崩溃" />
</p>

| 部分 | 包含的内容 |
|------|---------------|
| Screenshot | 屏幕当前的样子（或会话的最后一张），位于文件内部 |
| Screen recording | 来自带视频的会话：包含崩溃的那一段，或最后一段。不超过 20 MB 时它是文件的一部分；更大的片段会以 `emulabzip-report.mp4` 写在它旁边，文件会引用它 |
| Timeline strip | 来自会话：崩溃前（或运行结束前）的最后一分钟，每种事物一行：触摸、请求、日志问题、慢帧、垃圾回收、崩溃。由数字绘制，没有脚本 |
| Device | 名称、操作系统、型号、ABI、内存、屏幕、语言和时区 |
| Crashes | 最近 30 分钟内的崩溃，使用为该应用保存的映射文件、source map 和 dSYM 读取：参见[可读的堆栈](symbolication.md) |
| Failed requests | 没有得到应答或状态为 400 及以上的请求，附带错误和应答的开头。它们来自 [Network](network.md) 中的代理，因此请先启动它并连接设备 |
| Log | 警告和错误（数量多时错误优先），以及最后几行 |

<p align="center">
  <img src="images/bug-report-video.png" alt="由会话生成的报告：崩溃前后的片段，以及条带上的最后一分钟" />
</p>

该文件没有脚本，也不会从服务器加载任何内容，因此在任何浏览器中、离线、多年之后打开都是一样的。其中的一切都经过转义，因此日志或响应中的文本绝不会变成标记。列表有上限，过大的截图会被省略并附带说明，因此报告始终是人能够阅读的。

当某部分不应该离开你的电脑时，可以把它省略：`--without screenshot`、`logs`、`requests`、`crashes` 或 `video`（多个时重复使用），也可以使用对话框中的复选框。

## 打开 issue

`--issue` 接受一个跟踪器：

| 跟踪器 | 写法 |
|---------|-------------|
| GitHub | `github:owner/repo` |
| GitLab | `gitlab:group/project`，或使用自己的服务器时写 `gitlab:git.example.com/group/project` |
| Jira | `jira:yoursite.atlassian.net/PROJECTID/ISSUETYPEID`，两个 id 都是数字 |

该链接会打开跟踪器的新建 issue 页面，并填好标题和摘要：描述、应用和设备、崩溃及其前几个栈帧、失败的请求以及日志中最先出现的错误。摘要会被截短，以符合跟踪器接受的长度，并以一行说明完整报告已附上作为结尾。HTML 文件需要你自己附加。不会要求或保存任何令牌，因为 EmuLabZip 从不与跟踪器通信：是你的浏览器在通信，使用的是你自己的登录。

## 来自录制

[会话](sessions.md)已经包含一次运行的日志、请求、崩溃和截图，因此可以稍后根据它生成报告，也可以根据 CI 运行生成：在 **Sessions** 中打开它并点击 **Bug report**，或者

```bash
emulabzip report --session run.ezsession -o report.html
```

## 面向 AI 助手

`create_bug_report` 会把文件写入 EmuLabZip 数据文件夹中的 `reports/`，并返回其路径、一份 Markdown 摘要，以及（使用 `issue` 时）链接。片段过大而无法放进文件的报告，会得到自己的文件夹，片段就在其中。参见 [MCP](mcp.md)。

## 分享之前要检查什么

报告包含你的应用屏幕上和日志中的内容，以及失败请求的地址。屏幕片段可能显示其上出现过的任何内容。在把它放入他人可见的 issue 之前，请先阅读并观看它，并对应该排除在外的内容使用 `--without`。参见[安全](../SECURITY.md)。
