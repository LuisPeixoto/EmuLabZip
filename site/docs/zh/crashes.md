---
source: fe2e62872ca2
description: 介绍 Crashes 工具如何列出设备记录的崩溃、原生崩溃和 ANR，展示堆栈、事实与建议，说明报告来源、阅读方法、提示和演示应用。
---
# 崩溃

Crashes 工具列出设备记录的崩溃、原生崩溃和 ANR，并打开每一条，显示其堆栈、
设备写下的事实以及接下来可以尝试什么。EmuLabZip 打开期间，新的崩溃会自行出现，并伴有通知和侧边栏
徽标。

| 类型 | Android | iOS Simulator |
|------|---------|---------------|
| **Crash** | 未捕获的异常，附带其 `Caused by:` 链 | Swift 运行时失败、`abort()` 或未捕获的 `NSException` |
| **Native crash** | 致命信号，来自 tombstone | 错误的内存访问（`EXC_BAD_ACCESS`）及类似情况 |
| **ANR** | 应用无响应（Application Not Responding），附带每个线程的堆栈 | 不适用 |

<p align="center">
  <img src="images/crashes.png" alt="一个 ANR，含被阻塞的主线程、提示和详情" />
</p>

## 快速开始

1. 打开 **Crashes**（<kbd>⌘8</kbd>）。最新的崩溃会自动打开。
2. 在左侧选择另一条，或按类型过滤，或按工具栏中所选的应用过滤。
3. **Report** 显示提示、详情和线程。**Raw** 显示设备写下的原始报告。**Save…**
   将其写入文件。
4. 发布构建会打乱名称。当崩溃提出要求时，添加其 `mapping.txt`、source map 或 dSYM，
   崩溃就会被重新读取，并可在 **Readable** 和 **Original** 之间切换。见[可读的堆栈](symbolication.md)。
5. 工具栏中的 **Report a bug** 会把崩溃、截图、日志和失败的请求打包成一个文件。见
   [缺陷报告](bug-reports.md)。

在终端中：

```bash
emulabzip crashes --app com.example.app          # newest first, with an id for each
emulabzip crash dropbox:data_app_crash:20261023205928
emulabzip crash <id> --all-threads               # every thread, not only the one that crashed
emulabzip crash <id> --raw                       # as the device wrote it
emulabzip crash <id> --mapping mapping.txt       # readable, with a file you name
```

## 报告从哪里来

**Android。** 系统会在 DropBox 中保留每个崩溃、ANR 和 tombstone 的副本，shell 可以通过
`dumpsys dropbox --print <tag>` 读取。这意味着它适用于任何设备和任何模拟器，无需 root，也无需
安装任何东西。EmuLabZip 读取 `data_app_crash`、`system_app_crash`、`data_app_native_crash`、`SYSTEM_TOMBSTONE`、
`data_app_anr` 和 `system_app_anr`。id 形如 `dropbox:data_app_crash:20261023205928`。系统会对
同一应用的重复崩溃限速，因此循环崩溃被列出的次数少于它实际发生的次数。

**iOS Simulator。** 进程崩溃时，macOS 会向 `~/Library/Logs/DiagnosticReports` 写入 `.ips` 报告，模拟的
应用也不例外。当应用的路径经过
`CoreSimulator/Devices/<udid>` 时，该报告属于对应的模拟器。EmuLabZip 读取此处和 `Retired` 中最新的 200 个文件。id 形如
`ips:MyApp-2026-10-23-210246.ips`。macOS 会在崩溃几秒后写入报告。

## 阅读报告

崩溃的线程处于展开状态；其他线程折叠起来。对于 ANR，被阻塞的主线程排在最前。每个帧
都有一个彩色方块，表示它是谁的代码，颜色与火焰图相同，共四种：

| 颜色 | 含义 |
|--------|---------|
| 蓝色 | 应用自己的代码 |
| 绿色 | 平台：Android 框架和运行时、UIKit、Foundation、Swift 运行时 |
| 橙色 | 原生库 |
| 灰色 | 内核 |

**Only the app's code** 会隐藏其他所有内容，这是阅读大部分是运行时内部实现的原生堆栈的方法。

Java 崩溃会为链中的每个异常显示一个部分，最后一个，即根本原因，被标记为
崩溃。崩溃的标题就是这个根本原因。

## 提示

EmuLabZip 会说明它能从报告中得出什么：

- 看起来被混淆的类名：添加该构建的 `mapping.txt`，它们就会自动被还原。见
  [可读的堆栈](symbolication.md)。
- React Native 应用的 JavaScript 位置：添加该 bundle 的 source map。
- 应用中没有符号的原生帧：`ndk-stack -sym <folder> -dump tombstone.txt`，或 `llvm-symbolizer`。
  BuildId 必须与崩溃的那个构建一致。
- 应用中没有符号的 iOS 帧：添加该构建的 dSYM，它们会通过 `atos` 读取，或运行所显示的 `atos` 命令，
  其中已填好架构和加载地址。
- 主线程处于睡眠或等待的 ANR：主线程被阻塞了。
- 设备 CPU 不足时的 ANR：问题可能不在应用。
- 信号来自 `kill()` 的原生崩溃：它是被发送的，而不是由故障引发的。

## 试一试

`tools/crash-app`（Android）和 `tools/crash-app-ios`（iOS Simulator）是故意失败的小应用，让你
无需自己编写就能看到每一种类型。每个都有 `install.sh` 和一份 README。
