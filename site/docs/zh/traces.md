---
source: 628549c96a94
title: 追踪工具：Perfetto 与 CPU 剖析
description: Traces 工具记录系统和应用在几秒钟内的行为，整合 Perfetto、simpleperf 和 Instruments 三种官方工具，并把输出保存在同一个库中，帮你找出变慢的原因。
---
# 追踪

Traces 工具会记录系统和应用在几秒钟内做了什么，然后帮你找出为什么慢。它整合了三种官方工具，并把它们的输出保存在同一个库中：

| 录制 | 设备 | 工具 | 你在 EmuLabZip 中得到什么 |
|-----------|--------|------|---------------------------|
| **System trace** | Android 9 及更高版本 | [Perfetto](https://perfetto.dev) | 摘要（CPU、帧、内存、最忙的 slice）、SQL 控制台，以及一个在 Perfetto UI 中打开完整时间线的按钮 |
| **CPU profile** | Android 8 及更高版本 | `simpleperf` | 带有热点函数的火焰图，以及折叠堆栈导出 |
| **Instruments** | iOS 模拟器 | `xcrun xctrace` | Time Profiler 录制的火焰图，以及一个在 Instruments 中打开 `.trace` 的按钮 |

<p align="center">
  <img src="images/traces-summary.png" alt="Perfetto 帧追踪的摘要" />
</p>

## 快速开始

1. 打开 **Traces**（<kbd>⌘7</kbd>）并选择你的设备。如果想只关注某个应用，请在工具栏中选择该应用。
2. 选择要录制的内容，设置时长（1 到 300 秒），然后点击 **Record**。<kbd>Stop</kbd> 会提前结束，并保留到目前为止录制的内容。
3. 录制会自动打开。每次录制都会保存在 **Recordings** 列表中，直到你删除它。

在终端中：

```bash
emulabzip trace record --preset frames --duration 15 -o frames.pftrace
emulabzip trace summary frames.pftrace
emulabzip trace query frames.pftrace "select name, count(*) n from slice group by name order by n desc limit 10"
emulabzip trace cpu --app com.example.app --duration 10 --folded stacks.txt
```

## 系统追踪（Perfetto）

四个预设涵盖了常见的问题。它们是普通的 Perfetto 文本配置，因此 **Custom** 可以接受你自己的配置。

| 预设 | 记录内容 | 用来回答 |
|--------|---------|------------------|
| **CPU & scheduling** | 调度器切换和唤醒、CPU 频率和空闲状态、进程列表 | 哪些线程使用了 CPU，CPU 是否可用？ |
| **Frames & jank** | SurfaceFlinger 的帧时间线，以及围绕它的视图、图形、输入和 binder slice | 哪些帧延迟了，它们延迟时在运行什么？ |
| **Memory** | 进程内存（`mem.rss`）和系统内存计数器（每 250 ms 一次），选择应用时还包括原生分配（`heapprofd`） | 内存在哪里增长？ |
| **Custom** | 你的配置所指定的任何内容 | 其他任何问题 |

**Summary** 标签页显示：

- 追踪时长、CPU 的繁忙程度、帧数，以及第 50、90 和 99 百分位的帧时间；
- 按进程和按线程划分的 CPU 时间；
- 对于帧：卡顿的种类（按 SurfaceFlinger 的命名），以及每个图层有多少帧延迟。当应用或合成器错过截止时间时，该帧算作延迟；仅有 *Prediction Error* 和 *Buffer Stuffing* 不算；
- 内存：系统计数器的最低值和最高值，以及每个进程的常驻内存峰值；
- 总耗时最多的 slice。

如果追踪中没有某部分的数据，该部分就会省略。内存追踪没有调度信息，CPU 追踪没有帧。其他所有内容请使用 **SQL** 标签页：它运行 [PerfettoSQL](https://perfetto.dev/docs/analysis/perfetto-sql-syntax)（<kbd>⌘</kbd>/<kbd>Ctrl</kbd> + <kbd>Enter</kbd>），可使用 `slice`、`sched`、`thread`、`process` 和 `counter` 等表，最多显示 5,000 行。

### 追踪处理器

读取 Perfetto 追踪需要 Google 的 `trace_processor_shell`。EmuLabZip 不内置它。你第一次打开 Perfetto 追踪时，它会说明下载内容并先征求你的同意：

- 它来自 GitHub 上的官方 Perfetto 发布（版本固定在源码中，根据平台不同为 12 到 81 MB）；
- 文件会对照源码中固定的 SHA-256 进行校验，不匹配则丢弃；
- 它保存在 EmuLabZip 数据文件夹内的 `tools/` 中，并在你打开追踪时仅在 `127.0.0.1` 上、使用一个空闲端口启动。当你打开另一个追踪或退出时，它会停止。

### 在 Perfetto UI 中打开

**Open in Perfetto UI** 会使用 [ui.perfetto.dev](https://ui.perfetto.dev) 所需的标头，从 `127.0.0.1` 提供该文件一次，然后打开你的浏览器。页面从你自己的电脑加载追踪：文件绝不会被上传。服务器只响应这一个文件，并在文件被取走后或两分钟后停止。

## CPU 剖析（simpleperf）

`simpleperf` 会带着调用堆栈对 CPU 采样。EmuLabZip 在设备上运行它，在设备上转换其报告并拉取结果，因此你的电脑上不需要 NDK。

- **应用**：应用必须正在运行，并且是*可调试*或*可剖析*的。否则 simpleperf 会拒绝，EmuLabZip 会告诉你原因。它不会等待尚未运行的应用。
- **整个系统**：对每个进程采样。它需要 `adb root`，因此请使用不带 Google Play 的模拟器镜像。

火焰图是冰柱图：最外层的调用在顶部，宽度与 CPU 时间成正比。点击一个帧可放大，使用面包屑导航可缩小，输入文字可高亮某个函数，也可以在其上方选择另一个进程或线程。颜色表明某个帧属于谁的代码：

| 颜色 | 含义 |
|--------|---------|
| 蓝色 | 应用自己的代码：已安装的应用、它们的库，以及 JIT 编译的应用方法 |
| 绿色 | 平台托管代码：Android 框架、ART、`*.jar`、`*.odex`、`*.oat` |
| 橙色 | 原生库（`.so`） |
| 灰色 | 内核 |

<p align="center">
  <img src="images/traces-flame.png" alt="整个系统的 CPU 剖析，以火焰图显示" />
</p>

在图的下方，**Hot functions** 列出时间花在函数自身而不是它所调用的函数上的位置。**Folded stacks** 会保存调用树，供 `flamegraph.pl` 和 speedscope 使用。原始的 `perf.data` 保存在录制旁边；*Show in folder* 会显示它的位置。

## Instruments（iOS 模拟器）

EmuLabZip 会对 `xctrace list templates` 报告的任何模板运行 `xcrun xctrace record`，附加到你选择的应用（或所有进程）上。结果是一个 `.trace` 文档：**Open in Instruments** 会打开它。对于 **Time Profiler** 和 **CPU Profiler**，EmuLabZip 还会导出样本（`xctrace export`），并绘制同样的火焰图。

要求：

- Xcode，并且 `xcrun xctrace` 可用。
- Mac 上开启 **Developer Mode**。没有它，`xctrace` 会等待一个没有人能给出的批准。EmuLabZip 会先检查 `DevToolsSecurity -status`，并告诉你该怎么做：在终端中运行 `sudo DevToolsSecurity -enable`。
- 对于单个应用，它必须正在模拟器中运行。

Time Profiler 导出读取器遵循 `xctrace` 文档中的格式（行带有线程、权重和堆栈，重复的元素只写一次并通过 id 引用）。它由针对手写导出文件的测试覆盖；编写时没有可用的、来自开启 Developer Mode 的 Mac 的录制。如果录制打开后没有火焰图，`.trace` 仍然完好，可以在 Instruments 中打开。请提交一个 issue 并附上 Xcode 版本。

## 录制保存在哪里

每次录制都是 EmuLabZip 数据文件夹内 `traces/` 中的一个文件夹（`EMULABZIP_HOME` 可以移动它）：

```text
traces/20261023-184501-3f2a/
    meta.json          what was recorded, where and when
    trace.pftrace      or profile.pb (+ perf.data), or recording.trace (+ time-profile.xml)
```

`emulabzip trace list` 会打印它们。在应用中删除一个录制会移除它的文件夹。写入设备的任何内容都会被再次删除：配置、原始录制和报告会在录制结束时从设备上删除，无论录制成功、失败还是被停止。

## 限制

- 每次录制最长 300 秒，每个 Perfetto 追踪最大 1 GB。
- Perfetto 需要 Android 9（API 28）或更高版本。在 Android 9 和 10 上，EmuLabZip 会开启 `persist.traced.enable`，因为系统镜像默认将其关闭。
