---
source: 6a33aedfa838
description: EmuLabZip 可以自己读取 Android 的 .hprof 堆转储，无需其他工具，列出疑似泄漏、保持对象存活的路径和占用内存最多的对象。
---
# 在堆转储中查找泄漏

EmuLabZip 自己读取 Android 的 `.hprof`，列出是什么让内存一直存活，无需任何其他工具。

```bash
emulabzip heapdump com.example.app --analyze         # take a dump and analyze it
emulabzip heap analyze before-login.hprof            # a dump you already have
emulabzip heap analyze dump.hprof --json --top 20
```

应用必须是**可调试**的（或设备使用 `eng`/`userdebug` 镜像），才能进行转储。

## 报告内容

**疑似泄漏**，即毫无疑问是泄漏的情况，并附上保持每个对象存活的路径：

- 一个已**销毁**（`mDestroyed`）或已**结束**（`mFinished`，仅报告为*可能*，因为系统可能还没有销毁它）但仍在内存中的 `Activity`；
- 一个没有 `mFragmentManager` 的 `Fragment`（`androidx.fragment.app` 或 `android.app`），这意味着它已被移除。

对每一项，报告会给出：它的类和地址、**它保持存活的内存量**，以及从 GC root 出发的最短路径，例如：

```text
static field: class com.example.LeakyActivity.kept → com.example.LeakyActivity
```

请从右向左阅读路径：最后一行是泄漏的对象，它前面的条目是持有它的对象。弱引用、软引用和虚引用（`Reference.referent`）不会被跟踪，因为它们不会让对象保持存活。

**保持最多内存存活的对象**，按保留大小排序。保留大小是对象被回收后会释放的内存；它由支配树计算得出，因此共享对象不会被重复计算。

**按大小排列的类**：实例数量和浅层大小。

默认情况下只查看应用自己的堆；`--all-heaps` 会加入 zygote 和系统镜像。

## 试一试

`tools/leak-app` 是一个小型可调试应用，会故意泄漏一个 Activity（见其 README）：

```bash
tools/leak-app/install.sh
adb shell am start -n dev.emulabzip.leaky/.MainActivity --es mode leak
emulabzip heapdump dev.emulabzip.leaky --analyze
```

## iOS 模拟器

`.memgraph` 由 Mac 的 `leaks` 工具读取，因此需要一台 Mac。报告按类型列出**保留环和泄漏的对象**及其大小（没有保留大小）。`tools/crash-app-ios --leak` 会制造一个保留环，供你试用。

## 在哪里找到它

- **桌面端**：Memory → **Heap dump…**，然后点击 **Find leaks**，或用 **Open dump…** 打开磁盘上的文件。共有三个标签页：Suspected leaks、Dominators 和 Classes。
- **CLI**：`emulabzip heap analyze`、`emulabzip heapdump --analyze`。
- **MCP**：`analyze_heap`（Android）。

分析一个 40 MB 的转储需要几秒钟。堆转储包含应用的数据（见[安全](../SECURITY.md)），因此请像对待应用本身一样保密。
