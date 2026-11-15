---
source: 3fbc535a9ed8
description: 介绍 EmuLabZip 如何通过 xcrun simctl 连接 iOS 模拟器、收集哪些数据、如何应对繁忙的统一日志，以及目前的使用限制。
---
# iOS Simulator

在安装了 Xcode 的 macOS 上支持 iOS 模拟器。在 Windows 和 Linux 上，iOS 后端会报告自己不可用，
EmuLabZip 的其余部分照常工作。

## 连接方式

EmuLabZip 使用有文档的 `xcrun simctl` 工具。参数直接传给进程，绝不经过
shell。Instruments 使用私有框架与模拟器通信；EmuLabZip 有意不这样做，因此在各个 Xcode 版本之间都能持续工作。
见 [How the official tools work](research/how-the-official-tools-work.md)。

## 收集的内容

| 功能 | 来源 |
|---------|--------|
| 设备列表 | `simctl list devices -j`，仅限已启动的 iOS 模拟器 |
| Overview | 模拟器 JSON、`simctl list runtimes -j`、模拟器内的 `defaults read -g AppleLocale` / `AppleLanguages`，以及主机上的 `sw_vers` 和 `xcodebuild -version` |
| 应用 | `simctl listapps`（用 `plutil` 转换），进程 id 来自模拟器内的 `launchctl list` |
| 日志 | `simctl spawn <udid> log stream --style ndjson --level debug` |
| 截图 | `simctl io <udid> screenshot` |

## 统一日志很繁忙

在 debug 级别下，模拟器很容易每秒写入几千行。EmuLabZip 每 100 ms 批量处理一次日志行，
并保留一个有上限的缓冲区（默认 50,000 行），因此界面保持响应。要少看一些内容：

- 选择你的应用，并在 Logs 中打开应用过滤器。
- 将最低级别提高到 **I** 或 **W**。
- 在 Settings 中关闭 *Verbose and debug lines*，让 `log stream` 只返回 info 及以上级别。

iOS 日志行以**进程名**作为标签，并带有**子系统和类别**（例如
`com.apple.network:connection`），点击某一行即可看到。

## 限制

- `log stream` 没有历史记录，因此日志从选中设备时才开始。
- 尚不支持 iOS 真机（见[路线图](roadmap.md)）。
