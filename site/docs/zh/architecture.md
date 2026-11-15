---
source: a1e5e440e93b
description: 了解 EmuLabZip 的 Cargo 工作区如何划分：平台代码放在统一 trait 之后的小型 crate 中，桌面应用和 CLI 以同样方式使用它们，并说明数据流与设计原则。
title: EmuLabZip 架构：crate 划分、后端 trait、时间轴与代理设计
---
# 架构

EmuLabZip 是一个 Cargo 工作区。平台代码放在统一 trait 之后的小型 crate 中，两个前端（桌面应用和 CLI）以同样的方式使用它们。

```text
                ┌──────────────────────┐      ┌──────────────────┐
                │  apps/desktop        │      │  emulabzip-cli   │
                │  Svelte UI ◄─► Tauri │      │  (clap)          │
                └──────────┬───────────┘      └────────┬─────────┘
                           │        Lab (routes by platform)
                           └───────────────┬───────────┘
                                 ┌─────────┴─────────┐
                      Backend    │                   │   Backend
                ┌────────────────┴───┐        ┌──────┴──────────────┐
                │ emulabzip-android  │        │ emulabzip-apple     │
                │ logcat, getprop,   │        │ simctl, log stream, │
                │ dumpsys, pm, ps    │        │ listapps, plutil    │
                └─────────┬──────────┘        └──────┬──────────────┘
                ┌─────────┴──────────┐               │
                │ emulabzip-adb      │          xcrun simctl
                │ ADB host protocol  │
                └─────────┬──────────┘
                    adb server :5037
```

## Crate 一览

| Crate | 职责 |
|-------|----------------|
| `emulabzip-core` | 数据模型（`Device`、`App`、`InfoSection`、`LogEntry`）、日志过滤器、有界环形缓冲区、`Stream` 类型和 `Backend` trait。不含平台代码。 |
| `emulabzip-adb` | ADB host 协议的异步客户端：设备列表与跟踪、`shell:`、`exec:` 以及 `sync:` 文件协议。 |
| `emulabzip-android` | 基于 `emulabzip-adb` 为 Android 实现 `Backend`。包含 `getprop`、logcat、`dumpsys battery`、`wm`、`ps` 和 `pm` 的解析器。 |
| `emulabzip-apple` | 通过 `xcrun simctl` 为 iOS 模拟器实现 `Backend`。包含设备列表、ndjson 日志和应用列表的解析器。 |
| `emulabzip-audit` | 从应用外部审视应用的质量审计：安全与隐私规则、带脱敏的密钥模式表、`Finding` 和 `Audit` 类型、通过 `Lab` 从设备收集它们的输入、带大小对比和预算的 APK、AAB 和 IPA 分析器、图像比较与基线，以及语言和文字大小扫描。 |
| `emulabzip-storage` | 不依赖平台的查看器：文件类型检测、只读 SQLite 检查器、SharedPreferences 和属性列表、Preferences 和 Proto DataStore、MMKV、AsyncStorage、Core Data，以及 Realm 文件格式的读取器；**编辑器**（SQLite 行、偏好设置、属性列表，带备份和撤销）；`.ezstate` 快照格式和 schema 差异对比。 |
| `emulabzip-net` | 调试代理：证书颁发机构、TLS 拦截、HTTP/1.1 和 HTTP/2 转发、流式 gRPC、GraphQL 和 gRPC 的分类与解码、重新发送、网络条件（以及通过 `quinn` 和 `h3` 连接服务器的 HTTP/3）、可模拟、延迟、阻止或暂停请求的规则、HAR 录制的**回放**（代理在访问任何服务器之前先询问的 `ReplaySet`）、WebSocket 帧解析器、抓包文件读取器、带变更推送的有界流存储、正文解码和 HAR 导出。不含平台代码。 |
| `emulabzip-trace` | 理解录制内容：火焰图模型、simpleperf 报告和 `xctrace` 导出的解析器、Perfetto 配置、trace processor（下载、SQL、摘要）、供 Perfetto UI 使用的一次性服务器，以及录制库。不含平台代码。 |
| `emulabzip-frameworks` | 查看 Flutter 和 React Native 应用内部：一个小型的基于 WebSocket 的 JSON 客户端（`Rpc`）、Dart VM service（组件树、帧、内存），以及 Metro 与 Hermes（React 树、堆、控制台，以及 Redux、React Query、上下文和已注册 store 的**状态**，带操作日志和状态差异）。通过 `Lab::connect_port` 访问设备端口，因此这里不含平台代码。 |
| `emulabzip-symbols` | 让崩溃堆栈可读：R8 和 ProGuard `mapping.txt` 还原器（含内联帧）、带 VLQ 解码器的 source map（Metro 和 Hermes 组合映射）、通过 `atos` 查询 dSYM、按应用和版本存放这些文件的存储，以及 `symbolicate`，它会应用所有适用于某次崩溃的文件，并指出缺少什么。它处理文本和数字，因此无需设备即可处理文件。 |
| `emulabzip-report` | 缺陷报告：收集设备信息、截图、日志、失败的请求和可读的崩溃（来自设备或会话），写出一个独立的 HTML 文件和一份 Markdown 摘要，并为 GitHub、GitLab 和 Jira 生成预填的 issue 链接。不发送任何内容。 |
| `emulabzip-heap` | 读取 `.hprof` 堆转储并查找泄漏：两遍解析器、对象图、支配树和保留大小，以及泄漏规则。不含平台代码，也没有依赖，因此同样可以处理来自文件的转储。 |
| `emulabzip-session` | 会话：`.ezsession` 格式（版本 2，带设备时钟）、通过 `Lab` 跟随设备的录制器（日志、采样、崩溃、截图、触摸、慢帧和视频片段）、还能打开被中断录制的文件夹并仍可打开版本 1 文件的读取器、摘要、把所有流放在同一条轴上的**时间线**、用于回放的录制加载器、`emulabzip.toml` 预算，以及与基线的比较。 |
| `emulabzip-mcp` | MCP 服务器：基于 `Lab` 的工具，通过 `rmcp` 在标准输入和输出上提供服务。 |
| `emulabzip-plugin` | 插件：清单、批准步骤、JSON 行协议和运行器。不含平台代码。 |
| `emulabzip-cli` | `emulabzip` 命令。 |
| `apps/desktop` | Tauri 2 外壳（`src-tauri`）和 Svelte 5 界面（`src`）。 |

## 后端 trait

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    fn platform(&self) -> Platform;
    async fn is_available(&self) -> bool;
    async fn devices(&self) -> Result<Vec<Device>>;
    async fn info(&self, id: &str) -> Result<Vec<InfoSection>>;
    async fn apps(&self, id: &str) -> Result<Vec<App>>;
    async fn logs(&self, id: &str, options: LogOptions) -> Result<Stream<LogEntry>>;
    async fn screenshot(&self, id: &str) -> Result<Vec<u8>>;

    // Optional: the default implementations return `Error::Unsupported`.
    async fn perf(&self, id: &str, options: PerfOptions) -> Result<Stream<PerfSample>>;
    async fn memory(&self, id: &str, app: &str) -> Result<MemoryReport>;
    async fn heap_dump(&self, id: &str, app: &str) -> Result<HeapDump>;
    async fn storage_roots(&self, id: &str, app: &str) -> Result<Vec<StorageRoot>>;
    async fn list_files(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<u8>>;
    async fn trace_templates(&self, id: &str) -> Result<Vec<String>>;
    async fn record_trace(&self, id: &str, request: &TraceRequest, out_dir: &Path, stop: watch::Receiver<bool>) -> Result<TraceFile>;
    async fn clock(&self, id: &str) -> Result<DeviceClock>;
    async fn input(&self, id: &str) -> Result<Stream<InputEvent>>;
    async fn slow_frames(&self, id: &str, app: &str) -> Result<Stream<SlowFrame>>;
    async fn record_video(&self, id: &str, out_dir: &Path, segment_secs: u32, stop: watch::Receiver<bool>, segments: mpsc::UnboundedSender<VideoSegment>) -> Result<()>;
    async fn crashes(&self, id: &str, app: Option<&str>) -> Result<Vec<CrashSummary>>;
    async fn crash(&self, id: &str, crash_id: &str) -> Result<CrashReport>;
    async fn controls(&self, id: &str, app: Option<&str>) -> Result<ControlInfo>;
    async fn control(&self, id: &str, control: &Control) -> Result<()>;
    async fn network_status(&self, id: &str) -> Result<NetworkStatus>;
    async fn connect_network(&self, id: &str, port: u16) -> Result<NetworkStatus>;
    async fn disconnect_network(&self, id: &str) -> Result<NetworkStatus>;
    async fn install_ca(&self, id: &str, pem: &str, der: &[u8]) -> Result<CaInstall>;
}
```

`Lab` 持有所有后端，并按 `Target { platform, id }` 路由调用。缺少某个工具（`adb`、`xcrun`）只会让对应的后端不可用，不影响其他后端。

## 流

日志由后台任务产生，并通过有界通道（`Stream<T>`）传递。丢弃流会中止该任务，从而关闭 ADB 套接字或终止 `log stream` 子进程，因此没有任何东西会比读取方活得更久。

在桌面应用中，每个流都是由 Tauri 状态持有的一个*会话*。行每 100 ms 批量处理一次，并通过 Tauri IPC 通道发送到界面。一个批次最多包含 4,000 行；超出的部分会被计数并报告为已丢弃，而不是拖慢设备或界面。界面保留自己的有界缓冲区，并且只渲染可见的行。

## 统一的时间轴

每个流都以自己的方式标记时间（日志打印设备的本地时间，不带年份；采样从开始时计数；触摸和帧从设备启动时计数），因此一次录制会保存一个 `DeviceClock`：设备的挂钟时间与这台电脑的挂钟时间之差、它的时区，以及它的单调时钟读数为零的时刻。`emulabzip-session` 的 `timeline::build` 用它把触摸、日志问题、请求、慢帧、垃圾回收、崩溃、截图和视频放在同一条轴上，单位是从开始起的毫秒数。慢帧使用 `gfxinfo` 转储打印的 `Uptime` 来定位，这正是帧耗时所在的时钟。屏幕视频以短片段录制（见[时间线与屏幕视频](timeline.md)），每个片段完成后报告一次，因此被中断的录制会保留已有的内容。

## 代理

`emulabzip-net` 对设备一无所知。后端只决定设备如何访问它，以及设备如何信任证书；CLI 和桌面应用负责启动代理，并交给它一个 `FlowStore`。

```text
 device ──► 127.0.0.1:<port> ──► proxy ──► the internet
                                   │
              CONNECT, first byte is 0x16 (TLS)?
                 yes ─► terminate TLS with a leaf for the SNI name, serve HTTP/1.1 or HTTP/2,
                        forward over a new TLS connection, record the flow
                 no  ─► copy bytes both ways, record a tunnel
              handshake refused ─► remember the host, pass it through next time
```

**回放**是在询问服务器之前多出的一步：在规则之后，`forward` 在 `ReplaySet` 中查找该请求并用它作答；找不到时则回答 504。回放录制期间，无法用 HAR 回答的路径（代理无法解码的隧道、WebSocket、gRPC 调用、重新发送）会被拒绝，因此不会有任何内容到达网络。参见[回放录制](replay.md)。

响应正文在录制副本（最多 10 MB）的同时流式传给客户端，因此下载和事件流可以继续工作。请求正文在转发之前会被完整读取。WebSocket 升级在握手之后被隧道转发，同时这些字节也会交给帧解析器，由它记录消息。

在转发请求之前，代理会询问**规则**（第一个匹配的规则生效）：模拟响应会立即作答，延迟会等待，阻止会让请求失败，断点会把请求或响应暂存在注册表中，直到界面让它继续。开启 HTTP/3 后，声明了 `Alt-Svc: h3` 的主机会通过 QUIC 访问，并回退到 TCP。

## 存储路径

文件路径始终相对于由后端选择的*存储根*（`data`、`external`、`bundle`……）。`Lab` 会在后端看到路径之前拒绝绝对路径和 `..`，iOS 后端还会解析符号链接，并拒绝任何最终落在容器之外的路径。

## 设计原则

- **只使用公开接口。** EmuLabZip 读取的一切，都是你可以通过 `adb shell` 或 `xcrun simctl` 得到的。参见[官方工具如何工作](research/how-the-official-tools-work.md)。
- **没有 shell 注入。** 到达设备 shell 的值都会被加引号（`emulabzip_adb::quote`），包名会被校验，`simctl` 的调用不经过 shell。
- **合并设备往返。** 在 Android 上，Overview 只需一次 shell 调用；由解析器拆分其输出。
- **用真实输出测试解析器。** 从模拟器上捕获的固定样本放在各个 crate 旁边。
- **只做显式写入。** 只有在保存对话框或 CLI 标志之后才会写文件。
