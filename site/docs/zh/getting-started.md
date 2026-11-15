---
source: 76f23448cd4b
description: 从全新安装到几分钟内读取应用日志的入门指南，涵盖安装 EmuLabZip、准备平台工具、启动设备、各个工具概览和常用键盘快捷键。
---
# 快速开始

本指南带你从全新安装开始，几分钟内就能读到应用的日志。

## 1. 安装 EmuLabZip

从[最新版本](https://github.com/LuisPeixoto/EmuLabZip/releases)下载适合你系统的安装程序，
或者[从源码构建](../README.md#build-from-source)。

## 2. 准备好平台工具

| 要检查的对象 | 你需要 |
|------------|----------|
| Android 模拟器和设备 | Android SDK 的 **platform tools**（`adb`）。Android Studio 会安装它们。 |
| iOS 模拟器 | 安装了 **Xcode** 及其命令行工具的 Mac（`xcode-select --install`）。 |

EmuLabZip 会在 `PATH`、`$ANDROID_HOME` / `$ANDROID_SDK_ROOT` 或默认的 SDK 位置
（`~/Library/Android/sdk`、`%LOCALAPPDATA%\Android\Sdk`、`~/Android/Sdk`）中查找 `adb`。如果 ADB 服务器没有
运行，它会启动该服务器。详情见 [Android](android.md) 和 [iOS Simulator](ios-simulator.md)。

## 3. 启动设备

```bash
# Android
emulator -list-avds
emulator -avd Pixel_9

# iOS
xcrun simctl boot "iPhone 17"
open -a Simulator
```

打开 EmuLabZip。正在运行的设备会在几秒内出现在左上角的 **device picker**（设备选择器）中，
第一个就绪的设备会被自动选中。

## 4. 四处看看

- **Overview** 显示设备的基本信息：操作系统和构建、硬件、屏幕、电池、存储和区域设置，并附有屏幕的实时预览。点击任意值即可复制。
- **Logs** 从选中设备的那一刻起跟踪设备日志，因此打开时历史记录已经在那里。见[日志](logs.md)。
- **Performance** 实时绘制 CPU、内存、帧率和电池的图表。见 [Performance](performance.md)。
- **Memory** 拍摄快照、对比快照并保存堆转储。见 [Memory](memory.md)。
- **Storage** 浏览应用的文件，并打开数据库、偏好设置和属性列表。见 [Storage](storage.md)。
- **Network** 捕获设备的 HTTP 和 HTTPS 流量。见 [Network](network.md)。
- **Traces** 记录系统和应用做了什么，并显示时间花在了哪里。见 [Traces](traces.md)。
- **Crashes** 列出崩溃、原生崩溃和 ANR 及其堆栈，并在出现新的崩溃时通知你。见
  [崩溃](crashes.md)。
- **Controls** 更改设备的位置、外观、状态栏、电池和权限。见
  [Controls](controls.md)。
- **Plugins** 运行你自己的采集器。见 [Plugins](plugins.md)。要让 AI 助手使用你的设备，见
  [MCP 服务器](mcp.md)。
- 工具栏中的 **Screenshot** 会截取屏幕，并让你将其保存为 PNG。

## 5. 聚焦你的应用

在设备旁边的 **app picker**（应用选择器）中选择你的应用。然后在 Logs 中打开应用过滤器，只查看
其进程写入的行。EmuLabZip 会跟踪进程 id，因此应用重启后过滤器仍然有效。

## 键盘快捷键

| 快捷键 | 操作 |
|----------|--------|
| <kbd>⌘1</kbd> … <kbd>⌘6</kbd> | Overview、Logs、Performance、Memory、Storage、Network |
| <kbd>⌘F</kbd> | 搜索日志 |
| <kbd>⌘K</kbd> | 清除日志 |
| <kbd>⌘R</kbd> | 立即查找设备 |
| <kbd>⌘,</kbd> | Settings |

在 Windows 和 Linux 上，使用 <kbd>Ctrl</kbd> 代替 <kbd>⌘</kbd>。
