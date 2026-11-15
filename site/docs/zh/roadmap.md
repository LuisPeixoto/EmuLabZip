---
source: e1df661a2578
description: EmuLabZip 按阶段推进，每个阶段以一次发布结束：列出从第 1 阶段到 1.0 的已完成与计划中的功能、已知限制，以及尚未排入阶段的想法。
title: EmuLabZip 路线图：各阶段已发布功能、限制与后续计划
---
# 路线图

EmuLabZip 分阶段成长。每个阶段以一次发布结束，其中的每一项都能在 Android 模拟器上正常使用，并在平台允许时同样支持 iOS 模拟器。每个阶段内的条目大致按优先级排序。欢迎提出想法和 pull request；较大的改动请先开一个 issue 讨论。

## ✅ 阶段 1：连接与观察（v0.1）

自动发现 Android 模拟器和设备（纯 Rust 的 ADB 客户端）以及已启动的 iOS 模拟器（`simctl`） · 带实时状态的设备选择器 · 带运行中进程 id 的应用选择器 · **Overview**，显示构建、硬件、显示、电池、存储、区域设置和主机信息 · 实时屏幕预览和截图 · 用于 logcat 和统一日志的**流式日志查看器**：级别、标签、文本和正则表达式过滤器，按应用过滤，暂停，可处理每秒数千行的虚拟化列表，纯文本和 JSON 行导出 · `emulabzip` 命令行：`devices`、`info`、`apps`、`logs`、`screenshot`。

## ✅ 阶段 2：性能、内存与存储（v0.2）

所选应用的实时 CPU、内存、帧率、卡顿帧和电池图表 · 内存明细（Android 上为 Java、原生、图形、代码、堆栈；iOS 上为 VM 区域），带快照和差异对比 · Android 上的堆转储（`.hprof`）和 iOS 上的内存图（`.memgraph`） · 应用沙盒的文件浏览器（可调试 Android 应用用 `run-as`，还有共享存储，以及 iOS 上的 data、bundle 和 app group 容器） · 带分页行和只读查询的 SQLite 检查器 · SharedPreferences、属性列表、JSON、图像、文本和十六进制查看器 · `emulabzip perf`、`mem`、`heapdump`、`files`、`pull` 和 `db`。参见[性能](performance.md)、[内存](memory.md)和[存储](storage.md)。

## ✅ 阶段 3：网络（v0.3）

用 Rust 编写、可解密 HTTP/1.1 和 HTTP/2 的独立 HTTP(S) 代理（`emulabzip-net`） · 一键把 Android 设备（`adb reverse` 和全局代理设置）或 iOS 模拟器（Mac 的代理，精确保存并恢复）接入代理 · 证书安装：可 root 的模拟器用系统存储，其他情况用引导式用户证书，模拟器用 `simctl keychain` · 带过滤器、标头、解码后正文和耗时的请求列表 · 拒绝该证书的主机会被直通 · WebSocket 和其他连接被隧道转发并列出 · HAR 导出 · `emulabzip net` 和 `emulabzip ca`。参见[网络](network.md)。

## ✅ 阶段 4：追踪（v0.4）

带预设的 Perfetto 系统追踪（CPU 与调度、帧与卡顿、内存、自定义） · 由官方 `trace_processor` 支持的摘要和 SQL 控制台，该程序只下载一次，并固定版本和校验 · 来自 `simpleperf` 的 CPU 火焰图（单个应用或整个系统） · 用 `xctrace` 录制 Instruments，Time Profiler 带火焰图 · 在 Perfetto UI 或 Instruments 中打开追踪 · 录制库 · `emulabzip trace`。参见[追踪](traces.md)。

## ✅ 阶段 5：崩溃与自动化（v0.5）

崩溃、原生崩溃和 ANR 的收集（Android 上用 DropBox，iOS 模拟器上用 `.ips` 报告），包含堆栈、设备记录下的信息、符号化提示，以及新崩溃发生时的通知 · 位置、外观、状态栏、电池、权限、通知和链接的控制 · 一个 **MCP 服务器**，让 AI 助手可以读取设备、日志、崩溃和追踪并对其进行操作 · 插件：你自己的收集器，以打印 JSON 行的程序形式存在，由用户启用。参见[崩溃](crashes.md)、[控制](controls.md)、[MCP 服务器](mcp.md)和[插件](plugins.md)。

## ✅ 阶段 6：网络高级工具、布局与会话（v0.6）

对请求和响应的**模拟响应、延迟、阻止和断点** · **WebSocket 消息**，包括 `permessage-deflate` · **抓包导入**（`emulator -tcpdump`、`.pcap` 和 `.pcapng`），含 HTTP、TLS 服务器名称和 DNS · 从代理到服务器的 **HTTP/3** · Android 和 iOS 模拟器上的**布局检查**和**无障碍审计**（标签、触摸目标、对比度） · 录制到单个文件的**会话**，以及围绕测试运行进行录制的 **CI 模式** · **比较两台设备**。参见[模拟响应](mocks.md)、[WebSocket](websockets.md)、[抓包导入](pcap.md)、[布局](layout.md)、[会话](sessions.md)、[CI 模式](ci.md)和[比较](compare.md)。

## 阶段 7：性能与质量（v0.7） ✅

**应用启动耗时**，冷启动、温启动和热启动：Android 上用 `am start -W` 和 `Displayed` 行，iOS 上用 `simctl launch` 加统一日志的启动 signpost，重复 N 次并显示中位数 · 每个应用的**能耗**：Android 上用 `dumpsys
batterystats`（CPU、wakelock、无线电、GPS），模拟器上用热状态和 CPU 时间 · 针对 `.hprof` 堆转储的**泄漏查找器**：保留大小、支配树，以及仍可访问的已销毁 Activity 和 Fragment，并给出到 GC root 的路径 · `emulabzip.toml` 中的**性能预算**（启动耗时、内存峰值、卡顿帧、请求数），由 `emulabzip ci` 检查，任何一项被突破时运行失败 · **基线**：把一次运行与上一次良好的运行比较，并指出哪里变慢了。

## 阶段 8：框架内部（v0.8） ✅

**Flutter**：调试构建的 Dart VM service（组件树、帧耗时、内存） · **React Native**：通过调试器协议查看 Hermes 和 React 树 · **Jetpack Compose 和 SwiftUI** 在 Layout 中通过它们的语义树呈现，在 Android 提供的情况下带重组次数 · **gRPC 和 protobuf**（使用 `.proto` 文件，或原始数据）和 **GraphQL** 的网络解码，列表中给操作命名 · **编辑并重新发送**请求 · 代理中的**网络条件**：3G、慢速 Wi-Fi、高延迟、离线和丢包的预设。

本阶段留待以后的内容：**重组次数**（Android Studio 通过加载到应用中的代理获得，它使用的是非公开接口）、SwiftUI 自己的视图类型名称（无障碍不会暴露它们），以及在会话中记录网络条件。

## 阶段 9：可读的堆栈与一键缺陷报告（v0.9） ✅

崩溃和堆栈的**符号化**：基于 `mapping.txt` 的 R8 和 ProGuard 还原，Mac 上用 `atos` 处理 dSYM，以及 Hermes 和 Metro 的 **source map**，让 React Native 的 JavaScript 堆栈指向正确的文件和行 · 按应用和版本保存的映射文件，以及命令行中的 `--mapping` · 一键生成的**缺陷报告**：会话、截图、设备详情、过滤后的日志和失败的请求，打包成一个 HTML 文件，或作为 issue 在 GitHub、GitLab 或 Jira 上打开。

## 阶段 10：时间线与回放（v0.10） ✅

一条**时间线**，把触摸、日志问题、请求、慢帧、垃圾回收和崩溃对齐在同一条时间轴上，基于对照设备测量的时钟 · 以短片段录制到会话中的**屏幕视频**（Android 上用 `screenrecord`，iOS 上用 `simctl io recordVideo`），在桌面应用中与时间线同步，崩溃前后的片段会加入缺陷报告 · **网络回放**：由会话或 HAR 文件回答应用的请求，使应用可以对着录制的响应离线运行，适用于后端此后已经变化的缺陷，以及确定性的 CI 运行（`--replay`、`--fail-on-miss`）。

本阶段留待以后的内容：iOS 上的触摸（模拟器不会把它们交给自身之外的任何东西）、iOS 上逐帧的卡顿、回放 WebSocket 和 gRPC 调用（HAR 文件不保存它们的消息和 trailer），以及匹配带有时间戳或随机 id 的请求。

## ✅ 阶段 11：作为数据的应用状态（v0.11）

**[应用状态快照](app-state.md)**：把整个沙盒保存为一个 `.ezstate` 文件并恢复（可调试的 Android 应用用 `run-as` 和 tar，模拟器用 data 容器），并对恢复所替换的内容自动备份 · **[模拟器快照与设备生命周期](devices.md)**：创建、启动、停止、清除和删除 AVD 与模拟器，以及 `avd snapshot save|load|delete` · Storage 中针对 SQLite 行和 SharedPreferences 的**[写入模式](storage-editing.md)**（属性列表通过命令行），需要确认，带 Undo，且只读仍是默认 · **DataStore、MMKV、AsyncStorage、Core Data 和 Realm** 的查看器（带 Realm 文件格式的读取器） · 两个状态之间的 **schema 差异对比**，用于测试迁移 · **[React Native 状态](react-native-state.md)**：Redux、React Query、上下文和已注册的 store，带 Redux 操作日志。

限制：Realm 的集合、字典、mixed、decimal 和 UUID 列不会被读取；MMKV 的类型是猜测的；Core Data 的属性名会丢失大小写；来自 iOS 设备的 AsyncStorage 和 Core Data 是在固定样本文件上测试的，而不是在模拟器上；不读取 Flutter 状态。

## ✅ 阶段 12：超越性能的质量（v0.12）

应用运行时的**[安全与隐私审计](security-audit.md)**：明文流量、日志和偏好设置中的令牌与个人数据、导出的组件（`dumpsys package`）、`debuggable` 和 `allowBackup` 标志、`Info.plist` 中的 App Transport Security 例外，以及隐私清单中缺失的域名 · **[APK、AAB 和 IPA 分析器](package-size.md)**：按文件夹、dex 和资源统计大小，两个构建之间的差异，以及 `emulabzip.toml` 中的大小预算 · **[本地化检查](localization.md)**：伪区域设置（`en-XA`、`ar-XB`）、字体缩放和显示大小控制、跨语言和大小的扫描，以及通过 Layout 找出放不下的文字 · **更多无障碍检查**：焦点顺序和色盲模拟（文字缩放通过扫描完成） · **[视觉回归](visual-regression.md)**：把截图与基线比较，带差异图和 CI 中的检查 · **[分析事件检查器](analytics.md)**：由代理解码的 Segment、Amplitude、Mixpanel 和 Firebase 事件，以及 Firebase 的调试日志。

限制：审计看不到 `exported` 属性（`dumpsys` 不会打印它），并且按模式判断文本；大小分析器读取大小、文件夹和 dex 头，而不是字节码，目前也不支持已安装的应用；被截断的文本是启发式判断；Android 按应用而不是按设备切换语言；Firebase 解码器是在按 Google 的布局制作的载荷上测试的，而不是在真实流量上；没有通过 MCP 提供的分析工具。

## ✅ 阶段 13：自动化与混合应用（v0.13）

通过 Chrome DevTools Protocol 在 Android 上检查 **WebView**，经由 ADB 的 `localabstract` 套接字，不转发任何端口：它们的控制台、JavaScript 和请求都在 EmuLabZip 内查看（请求出现在 Network 视图中，标记为 WEBVIEW），在 iOS 上则提供通往 Safari Web Inspector 的引导式快捷方式 · Android 上的**输入**（点按、滑动、文本、按键和启动应用），可在应用、命令行和 MCP 中使用，以及基于 `getevent` 的**流程录制**，可重新播放并导出为 Maestro 流程或 shell 脚本；iOS 没有公开的触摸接口，因此这仅限 Android · 带历史和固定项的**深层链接测试器**，检查域名的 `assetlinks.json` 或 `apple-app-site-association`、应用的签名密钥、团队 id 和 entitlement、设备的判定结果（`pm get-app-links`）以及谁会打开该链接 · **可编程的网络规则**：映射到本地和映射到远程、规则链、对照 OpenAPI 3 文件检查响应，以及运行在沙盒 WebAssembly 中的规则脚本，只有其确切字节被批准后才会运行。参见[应用中的网页](webviews.md)、[输入与流程](automation.md)、[深层链接](deep-links.md)、[网络规则](network-rules.md)和[规则脚本](rule-scripts.md)。

限制：WebView 检查需要可调试的应用，iOS 只是通往 Safari 的快捷方式，而不是应用内控制台；流程只录制触摸屏（不含注入的点按和键入的文本），并且只有在及时读取了屏幕时才会给点按加标签；在模拟器上进行的深层链接检查无法像设备那样验证域名；OpenAPI 检查涵盖 JSON 响应，不涵盖请求；映射到本地和映射到远程不适用于 gRPC 调用；脚本在两次调用之间不保存任何内容；助手可以列出 WebView、读取它们的控制台、检查链接和播放流程，但不能在页面中运行 JavaScript，也不能批准脚本。

## ✅ 阶段 14：有上下文的 AI（v0.14）

MCP 服务器从 67 个无状态工具增长到 **118 个工具、8 个提示词和 13 个资源**（开启更改后为 130 个工具），它们在运行期间保持状态：**代理和捕获的请求**（启动、连接设备、过滤和读取请求、规则、条件、回放、断点、HAR 导出、OpenAPI 检查）、日志、性能、触摸以及 WebView 和 React Native 流的**实时监视**、**录制会话**并比较两个会话、**只读的应用数据**（SQLite、Realm、Core Data、偏好设置、DataStore、MMKV、AsyncStorage）、用已保存映射读取崩溃、创建设备、流程、视觉基线、语言扫描和构建预算 · 每个工具都有标题，并说明它是读取、更改还是销毁 · 针对常见工作的**提示词**（解释崩溃、为什么慢、总结会话、复现缺陷、调试请求、检查版本、检查链接、模拟后端）以及助手可以读取的**资源**（设备、会话、流程、实时捕获、一份指南） · **写入需要主动开启**：`emulabzip mcp --allow-writes` 会提供清除、删除、恢复或重写数据的工具，每次调用都会通过客户端请用户确认；批准脚本和插件以及 `ci` 永远不会提供 · 设置中的 **AI assistants** 卡片，附带 Claude Code、Claude Desktop、Cursor 和 VS Code 的配置 · 一份生成的 [MCP 参考](mcp-tools.md)，由测试使其与服务器保持一致。参见 [MCP 服务器](mcp.md)。

限制：编辑行和偏好设置、撤销编辑、通过触摸录制流程以及导入抓包，都不通过 MCP 提供（它们仍在应用和命令行中）；耗时较长的工具尚不报告进度；会话的视频帧不会作为图像返回；无法请求确认的客户端不能使用写入工具，除非给出 `--no-confirm`；监视保留最近 5,000 项，并在空闲十分钟后结束。

## 阶段 15：设备与平台（v0.15）

通过 Xcode 自带的工具（`devicectl`、`usbmuxd`）支持**实体 iPhone 和 iPad**：在 iOS 允许的范围内提供日志、应用、截图、崩溃和文件，不允许的地方则给出简明说明 · **通过 Wi-Fi 的 Android**，可在应用中配对无线调试 · **Wear OS 和 Android TV** 模拟器，以及 **watchOS、tvOS 和 visionOS** 模拟器，带适用于各自的视图 · **同时使用多台设备**：两台设备的日志和请求在同一个列表中，按设备标记。

## 阶段 16：团队与生态，1.0（v1.0）

并排**比较会话**，以及会话的**可分享 HTML 报告**（一个文件，不需要服务器） · **共享规则库**：保存在应用仓库中的规则文件，由应用和命令行加载 · 一个 **VS Code 扩展**，在编辑器中提供设备、日志和 MCP 服务器 · **插件目录**：带校验和列出的社区插件，仍由用户逐个启用 · **葡萄牙语界面**和其他语言 · **更新**：应用内的签名自动更新，以及 Homebrew、winget 和 Scoop 软件包 · **1.0 承诺**：命令行的选项和 JSON、MCP 工具和提示词，以及 `.ezsession` 格式都是稳定的，只会随主版本变化。

## 想法

尚未列入任何阶段的内容：为自行绘制界面的应用（游戏、画布）提供布局 · 针对超大响应的 HTTP/3 流式传输 · 读取会话的插件 · 一个可选的**配套 SDK**（npm、Gradle、SwiftPM），用于只有应用内代码才能看到的内容，例如 Redux、Zustand 或 React Query 状态、重组次数和功能开关，它与默认方式分开，默认方式不会在设备上安装任何东西。
