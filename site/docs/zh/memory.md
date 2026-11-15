---
source: e2d7a7419077
title: EmuLabZip 内存工具：快照、对比与堆转储
description: Memory 工具显示所选应用的内存去向，并让你对比两个时刻来找出增长的部分；还支持导出 Android 和 iOS 模拟器的堆转储，并可通过命令行使用。
---
# 内存

Memory 工具会显示所选应用的内存去向，并让你对比两个时刻，看看是什么增长了。

## 快照

点击 **Take snapshot** 记录一份明细：

| | Android | iOS 模拟器 |
|---|---|---|
| 来源 | `dumpsys meminfo <package>` | Mac 上的 `footprint --json <pid>` |
| 总计 | PSS（按比例分摊的集大小） | Footprint，iOS 用它来决定何时终止应用 |
| 类别 | Java 堆、原生堆、代码、堆栈、图形、其他私有内存、系统 | VM 区域，例如 `MALLOC_SMALL`、`__DATA`、`CoreAnimation` |
| 详情 | 总 RSS、swap，以及对象数量（视图、Activity、binder、WebView…） | 干净且可回收的内存、区域数量 |

先拍一份快照，使用应用（打开再关闭某个屏幕几次），然后再拍一份。第二份快照会自动与前一份对比；也可以从 **Compared with** 菜单中选择任何其他基线。类别按变化大小排序，并带有箭头和符号，因此方向从不只依赖颜色来表达。

如果某个类别在重复操作后持续增长，那就是查找泄漏的好地方。

## 堆转储

| | Android | iOS 模拟器 |
|---|---|---|
| 按钮 | **Heap dump…** | **Memory graph…** |
| 文件 | `.hprof`，由 `am dumpheap` 写出 | `.memgraph`，由 `leaks --outputGraph` 写出 |
| 用以下工具打开 | Android Studio、Eclipse MAT 或 [Perfetto](https://ui.perfetto.dev) | Xcode（File › Open）、`leaks`、`heap`、`vmmap` |
| 要求 | **可调试**的应用（或 `eng`/`userdebug` 镜像） | 无 |

在 Android 上，转储会写入 `/data/local/tmp`，复制到你的电脑，并从设备上删除。

EmuLabZip 也可以自己读取转储：**Find leaks**（或用 **Open dump…** 打开磁盘上的文件）会列出已销毁但仍然存活的 Activity 和已移除的 Fragment、保持每个对象存活的路径，以及保持最多内存存活的对象。请参阅[查找泄漏](leaks.md)。

## 通过命令行

```bash
emulabzip mem com.example.app
emulabzip mem com.example.app --json
emulabzip heapdump com.example.app -o before-login.hprof
emulabzip heapdump com.example.app --analyze
emulabzip heap analyze before-login.hprof
```
