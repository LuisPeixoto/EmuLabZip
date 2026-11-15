---
source: f024f5686c62
description: 说明如何用 mapping.txt、source map 和 dSYM 把发布构建中被混淆的堆栈还原为可读形式，包括版本匹配、文件保存位置和命令行用法。
---
# 可读的堆栈

发布构建会隐藏名称。R8 和 ProGuard 把类重命名为 `a.b.c`，React Native 发布一个 JavaScript bundle，
其中每个位置都是 `1:608617`，而没有符号的 iOS 应用只有裸地址。只要有构建为此生成的文件，EmuLabZip 就能把它们
还原成你写的内容。

| 崩溃的对象 | 文件 | 你得到什么 |
|--------------|----------|--------------|
| 用 R8 或 ProGuard 压缩的 Java 或 Kotlin | `mapping.txt` | `MainActivity.kt:39` 中的 `Cart.average`，内联的调用会还原为各自独立的帧 |
| React Native JavaScript（Hermes 或 JavaScriptCore） | bundle 的 source map | `App.tsx 17:14`，而不是 `1:608617` |
| iOS 应用 | 其 `.dSYM` | 每个地址对应的函数、文件和行（在 Mac 上使用 `atos`） |

<p align="center">
  <img src="images/symbolication.png" alt="同一个 Android 崩溃：用 mapping.txt 还原后的可读版本，以及设备写下的原始版本" />
</p>

## 快速开始

为应用及其对应的版本保存一次文件。此后该构建的每个崩溃都会用它来读取。

```bash
emulabzip mappings add com.example.shop "1.2.0 (42)" app/build/outputs/mapping/release/mapping.txt
emulabzip crash dropbox:data_app_crash:20261023205928        # readable
emulabzip crash dropbox:data_app_crash:20261023205928 --raw  # as the device wrote it
```

在桌面应用中，打开 **Crashes**。可以读得更清楚的崩溃会说明缺少什么，并提供 **Add mapping or
source map…** 和 **Add dSYM…**：选择文件，崩溃就会被重新读取。**Readable** 和 **Original** 在
两者之间切换。

## 版本必须匹配

mapping 只对生成它的构建正确，用它去读另一个构建，结果比不读还糟。因此文件按
应用和版本保存，崩溃只会使用其自身版本对应的文件。版本就是崩溃中
显示的那个：

- **Android**：`1.2.0 (42)`，即版本名称和版本号，来自报告的 `Package:` 行。
- **iOS**：`1.2.0 (42)`，即短版本号和构建号。

没有合适的文件时，崩溃会说明这一点，并指出它需要的版本。

## 文件放在哪里

你可以把它们放在任何地方，并在命令行上指定，一次处理一个崩溃：

```bash
emulabzip crash <id> --mapping app/build/outputs/mapping/release/mapping.txt
```

或者用 `mappings add` 永久保存。它们会被复制到 EmuLabZip 的数据文件夹
（macOS 上为 `~/Library/Application Support/EmuLabZip/mappings/<app>/<version>/`），因此执行 clean 构建不会把它们
删掉。`emulabzip mappings list` 显示保存了什么，`emulabzip mappings remove <app> <version>` 会忘掉某个版本。
文件是什么类型由文件本身判断，因此无需指定类型；无法判断时，用 `--kind mapping|sourcemap|dsym`
指定。

## Android：mapping.txt

当 `isMinifyEnabled` 开启时，R8 会把它写到 `app/build/outputs/mapping/<variant>/mapping.txt`。为你发布的
每个版本保留它：这是从崩溃中的名称还原回来的唯一途径。

- 帧会还原出类、方法、文件和行。
- 当 R8 把一个方法内联进它的调用者时，那个被打乱的帧会变成整条链，内层调用在前，就像
  没有 R8 时那样。
- 异常标题中的类名也会被还原。
- 请在你的规则中保留 `-keepattributes SourceFile,LineNumberTable`，否则就没有行号可以还原。

## React Native：source map

React Native 崩溃的 JavaScript 在报告中以异常文本的形式出现，而不是帧。EmuLabZip 把它读成
一个 **JavaScript** 线程，Java 和原生线程排在其后。位置会在该
bundle 的 source map 中查找：

- **Hermes** 发布构建会写出合成的 map，`android/app/build/generated/sourcemaps/react/release/index.android.bundle.map`。
  请使用这个；旁边的 `packager` 和 `compiler` map 是组成它的两半。
- **JavaScriptCore** 和 **Metro** bundle 使用 `react-native bundle --sourcemap-output` 写出的 map。

## iOS：dSYM

Xcode 会在每个归档旁写出一个 `.dSYM`。EmuLabZip 检查其 UUID 是否与崩溃的镜像一致，然后
一次性向 `atos` 请求该镜像的每个地址。这需要 Mac，因为 `atos` 是 Xcode 工具的一部分；在
Linux 和 Windows 上，带有裸地址的崩溃会说明这一点，已有名称的崩溃则原样显示。能找到 dSYM 时，
Simulator 通常会自己对崩溃报告做符号化，那时就无事可做了。

## 来自别处的堆栈

从日志、缺陷报告或 CI 运行中复制的堆栈可以用同样的方式读取：

```bash
emulabzip symbolicate stack.txt --mapping mapping.txt
adb logcat -d | emulabzip symbolicate - --mapping mapping.txt
```

Java 帧用 `mapping.txt` 读取，JavaScript 帧用 source map 读取。`.dSYM` 需要崩溃报告中的地址和
镜像，而粘贴的堆栈没有这些。

## 面向助手

`get_crash` 用为其应用和版本保存的文件读取崩溃，并说明用了哪些文件、缺少什么
（`symbolication`）。`original: true` 会保留设备写下的名称。见 [MCP](mcp.md)。
