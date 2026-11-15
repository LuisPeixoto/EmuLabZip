---
source: ffe4b55ec1e8
description: 介绍会话的时间线与屏幕录像：把触摸、请求、日志、慢帧、GC 和崩溃放在同一条时间轴上，说明录制选项、统一时钟、读取方式、分段视频和缺陷报告。
---
# 时间线与屏幕录像

<p align="center">
  <img src="images/timeline.png" alt="Timeline 标签页：触摸、请求、日志、慢帧、GC 和崩溃各占一条轨道，以及崩溃时刻的视频" />
</p>

[会话](sessions.md)是一堆彼此独立的数据流：日志行、采样、请求、崩溃。每一种都以自己的方式标记时间，所以要看到*崩溃之前刚刚发生了什么*，就得打开四个标签页并做算术。**时间线**把这一切放在同一条轴上，而**屏幕录像**让你可以亲眼看到。

## 时间线上有什么

| 轨道 | 内容 | 来源 |
|------|------|---------------------|
| **Touches** | 点按、长按和滑动，以及它们在屏幕上的位置 | 触摸屏上的 `getevent`（Android；见下文） |
| **Requests** | 每个请求，长度即耗时，失败时为红色 | 代理（`--network`） |
| **Log** | 警告和错误 | 日志 |
| **Slow frames** | 应用绘制耗时过长的帧。三十帧的卡顿算一个事件：一次*突发* | `dumpsys gfxinfo framestats`（Android） |
| **GC** | 垃圾回收，以及它们耗时多久、使应用暂停多久 | ART 自身的日志行（Android） |
| **Crashes** | 每次崩溃，位于其发生的时刻 | 设备的崩溃记录 |
| **Screenshots**、**Video** | 每张截图拍摄的位置，以及每段视频覆盖的范围 | 录制 |

## 录制时间线所需的内容

```bash
emulabzip session record -o run.ezsession --app com.example.app --network --video --taps
```

- `--taps` 会录制触摸。它们是从内核的输入设备读取的，因此记录的是到达那里的触摸：手指在模拟器窗口上的操作，或通过模拟器控制台发送的事件（只有当事件带有 `ABS_MT_PRESSURE` 时，应用才会把它当作点击）。`adb shell input tap` 是在更高层、Android 内部注入的，**看不到**。iOS 不会把模拟器的触摸交给它之外的任何东西，所以这仅限 Android。内核只在位置变化时才报告位置，因此录制中的第一次触摸可能没有位置：它会被保留，并显示为不带坐标的“Tap”。
- `--video` 以 30 秒为一段录制屏幕（`--video 10` 表示 10 秒）。参见[屏幕录像](#screen-video)。
- 慢帧和垃圾回收不需要任何选项：在 Android 上，只要有应用就会录制。

在桌面应用中，**Sessions** 视图在 **Network** 旁边有 **Video** 和 **Touches** 复选框。

## 同一个时钟

每个数据流都以自己的方式标记时间：日志打印设备的本地时间，不带年份；采样从开始时起计数；请求使用这台电脑的时钟；触摸和帧则从设备启动时起计数。录制开始时，EmuLabZip 会向设备询问其时钟、时区和运行时长，并保存在会话中。时间线用它们把每个数据流放到这台电脑的时钟上，以距录制开始的毫秒数表示。

在 EmuLabZip 0.10 之前做的录制没有测量这些。它的日志时间是根据第一行推算出来的（它是在开始后片刻写入的，由此可以推断出时区），时间线会说明这些时间是估算的。

## 读取

在应用中，打开一个会话及其 **Timeline** 标签页。悬停在事件上可以阅读它，点击它可以查看更多内容并让视频跳到那里，拖动可以沿着运行过程移动，按住 ⌘（Ctrl）并滚动可以缩放。**Open** 会跳转到事件背后的请求、日志或崩溃。

在命令行中：

```bash
emulabzip session timeline run.ezsession                    # everything, oldest first
emulabzip session timeline run.ezsession --lane requests --lane crashes
emulabzip session timeline run.ezsession --json             # for scripts
```

```text
+00:04.174  taps      Tap (527, 1464)  (154 ms)
+00:04.206  frames    6 slow frames, worst 118 ms  (216 ms) !
+00:04.749  requests  GET time.akamai.com/ 200  (83 ms)
+00:50.000  crashes   java.lang.IllegalStateException: boom !!
```

一条轨道最多保留 3,000 个事件。超出时，错误会被保留，其余的会均匀分布；输出末尾（以及应用中轨道名称下方）的计数会说明原来有多少个。

AI 助手可以使用 [MCP 服务器](mcp.md)的 `session_timeline` 工具读取它：按轨道，有上限，最旧的在前。

## 屏幕录像

屏幕在 Android 上用 `screenrecord` 录制，在 iOS 模拟器上用 `simctl io recordVideo` 录制，分成短段。这样做有三个原因：无论你怎么要求，Android 都会在三分钟后停止 `screenrecord`；影片文件只有在工具关闭它之后才能读取；而一小段就是一个可以直接使用的片段，机器上不需要视频编辑器。

- 下一段在上一段还在从设备复制出来时就已开始，因此两段之间的间隔在 Android 上不到一秒，在模拟器上约三分之一秒。
- Android 的分段会缩放到长边为 1280 像素，码率为每秒 2 兆比特：30 秒约 7 MB。模拟器的分段保持其自身尺寸，为 H.264。
- 停止录制时，会保留正在录制的那一段已录下的内容。
- 视频在会话中不经压缩直接存储，因为它本身已经压缩过。

`emulabzip session video run.ezsession -o pieces/` 会把各段写成 `.mp4` 文件。在应用中，Timeline 标签页会播放它们：视频和时间线互相跟随。

分享文件时很容易忘记视频。屏幕可能显示任何内容：消息、电子邮件、密码。在把录制内容附加到公开 issue 之前，请先查看视频。

## 在缺陷报告中

由带视频的会话生成的报告，会包含含有崩溃的那一段（没有崩溃时则是最后一段），以及崩溃前最后一分钟的**条带**：每种事物一行，每个事件一个点。条带由数字绘制，没有脚本。最大 20 MB 的片段会成为 HTML 文件的一部分；更大的则会以 `emulabzip-report.mp4` 写在它旁边，文件会引用它，所以请把两者放在一起。使用 `--without video` 可以省略它。参见[缺陷报告](bug-reports.md)。
