---
source: 3928a9800ad4
description: 记录我们对 Xcode Instruments 和 Android Studio 性能分析器数据来源的静态研究，以及 EmuLabZip 为何只使用公开、有文档的接口，不在设备上安装任何东西。
title: 官方工具如何工作：Instruments 与 Android Studio 的数据来源研究
---
# 官方工具如何工作

在编写 EmuLabZip 之前，我们研究了 Xcode Instruments 和 Android Studio 性能分析器是如何收集数据的。这项研究是静态的：我们查看了已发布的应用包、它们链接的库、JAR 文件中的类和包，以及设备端二进制文件中的字符串。没有任何内容被修补或再分发。目标是弄清楚**存在哪些数据来源**，然后选出开源工具可以通过公开、有文档的接口使用的那些。

## Xcode Instruments

| 方面 | 我们的发现 |
|------|---------------|
| 语言和界面 | Swift 和 Objective-C，AppKit 搭配 SwiftUI，以及私有的图表和调用树框架 |
| 插件 | `.xrplugin` 包和 `.instrdst` 工具包（Networking、HTTP Tracing、SwiftUI、Hangs、Hitches、System Trace……） |
| 传输 | **DTX**，一种私有的 RPC 协议，运行在套接字、Mach 端口或共享内存之上，带压缩 |
| 设备代理 | `DTServiceHub` 提供 `sysmontap`（进程和系统统计）、`coreprofilesessiontap`（内核追踪）、`objectalloc`、`vmtracking` 和 `activitytracetap`（os_log 和 signpost）等通道 |
| 数据来源 | 内核追踪（`ktrace`、`kperf`）、统一日志系统和 signpost、符号化 |
| 网络 | 连接统计（`netstats`）以及通过统一日志发出的 CFNetwork HTTP 追踪，而不是代理 |
| 命令行 | `xctrace record` 和 `xctrace export` |

## Xcode 与模拟器

`simctl` 通过 XPC 服务驱动 **CoreSimulator**。它可以列出并启动设备，在模拟器内运行命令（`simctl spawn … log stream`），捕获屏幕（`simctl io screenshot`、`recordVideo`），列出应用及其容器，覆盖状态栏，设置位置，发送推送通知并添加根证书。模拟器中的应用是普通的 macOS 进程，因此主机可以直接读取它的 CPU 和内存使用情况。

## Android Studio

| 方面 | 我们的发现 |
|------|---------------|
| 平台 | JetBrains Runtime 上的 IntelliJ Platform，Kotlin 和 Java，Swing 搭配 Compose for Desktop 用于较新的性能分析器界面 |
| 设备访问 | ddmlib 和 adblib（ADB），以及模拟器的 gRPC API，通过 `avd/running/pid_*.ini` 文件发现 |
| 性能分析器 | 一条*传输管道*：Studio 与设备端守护进程之间使用 grpc-java 和 protobuf |
| 设备守护进程 | `transport`（原名 `perfd`），被推送到 `/data/local/tmp/perfd/`。它运行 `dumpsys meminfo --checkin`、`dumpsys SurfaceFlinger --latency`、`perfetto`、`simpleperf`、`am dumpheap`，并把 JVMTI 代理加载到可调试的应用中 |
| 追踪 | Perfetto（作为 gRPC 服务的 `trace_processor`）、simpleperf 报告、ART 方法追踪 |
| 内存 | 用 Shark 解析 `.hprof` 堆转储，通过 Perfetto 中的 heapprofd 获取原生分配 |
| 网络 | 以前是用 ASM 改写 OkHttp 和 `HttpURLConnection` 的字节码（`profilers-transform.jar`）；现在是 App Inspection 网络检查器，它挂钩可调试应用内部的 HTTP 栈 |

## EmuLabZip 使用什么

我们只使用**公开、有文档的接口**。Apple 的 DTX 协议和 Google 的设备端代理都是私有的，会随版本变化，也不是我们可以再分发的。

| 数据 | Android | iOS 模拟器 |
|------|---------|---------------|
| 设备 | ADB host 协议（`host:devices-l`），直接用 Rust 通信 | `xcrun simctl list devices -j` |
| 设备信息 | `getprop`、`wm`、`dumpsys battery`、`/proc` | `simctl` JSON、运行时列表、模拟器内的 `defaults` |
| 应用 | `pm list packages`、`ps` | `simctl listapps`、模拟器内的 `launchctl list` |
| 日志 | `logcat -v threadtime` | 模拟器内的 `log stream --style ndjson` |
| 截图 | `screencap -p` | `simctl io screenshot` |
| CPU 和内存（阶段 2） | `/proc/<pid>/stat`、`dumpsys meminfo` | 主机进程统计 |
| 帧（阶段 2） | `dumpsys gfxinfo framestats` | Signpost 和 `xctrace`（阶段 4） |
| 追踪（阶段 4） | Perfetto、simpleperf | `xctrace record` 和 `export` |
| 网络（阶段 3） | 带用户 CA 的 HTTP 代理、`emulator -tcpdump` | 主机代理、`simctl keychain add-root-cert` |

这让 EmuLabZip 保持小巧，可以安全地用于任何模拟器镜像，并且在 Xcode 和 Android Studio 更新时保持稳定。这也意味着设备上不会安装任何东西：每一次查询都是你自己也可以输入到 `adb shell` 或 `xcrun simctl` 中的内容。
