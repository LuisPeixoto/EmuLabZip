---
source: 11355a8d3815
description: 介绍如何用 emulabzip size 拆解 APK、AAB 或 IPA 并比较两个构建，在 emulabzip.toml 中设置大小预算让 CI 在构建增长过多时失败，以及桌面端用法和限制。
title: 构建大小：拆解 APK、AAB 与 IPA 并设置大小预算
---
# 构建大小

<p align="center">
  <img src="images/size-diff.png" alt="Build size 视图比较两个构建：按类型和按文件显示增长了什么" />
</p>

`emulabzip size` 会拆解 APK、AAB 或 IPA，并说明字节都在哪里；`emulabzip size diff` 会说明两个构建之间发生了什么变化；`emulabzip.toml` 中的**大小预算**会在构建增长过多时让 CI 失败。它读取这台电脑上的文件：不需要设备，不需要 SDK 工具，也不需要 Xcode。

```bash
emulabzip size show app-release.apk
emulabzip size show app-release.aab --json
emulabzip size diff app-1.4.apk app-1.5.apk
emulabzip size diff app-1.4.apk app-1.5.apk --fail-on-growth 5      # exit 1 over 5% growth
emulabzip size show app-release.apk --budget --baseline app-1.4.apk # hold it to [size] in emulabzip.toml
```

## 你能得到什么

- 下载大小（压缩后，即文件中存储的大小）和安装后大小，按**类型**划分：`dex`（编译后的代码）、`native`（`lib/<abi>/*.so`，在 iOS 上还有可执行文件和 `Frameworks/`）、`resources`、`assets`、`manifest`、`signing` 和 `other`。
- **按 ABI** 划分的原生代码（`arm64-v8a`、`x86_64`……），因此打包了四种 ABI 的构建一目了然。在 IPA 上则是 `Frameworks` 和可执行文件。
- 每个 **dex 文件**及其类和方法，从文件头读取，并与单个 dex 文件所能容纳的 65,536 个方法对比总数。
- AAB 的**模块**（`base` 以及功能模块）。
- 最大的文件。
- 使用 `diff` 时：总量和百分比，按类型和按文件夹划分的变化，以及**新增、删除和变更**的最大文件。

判断一个文件是 APK、AAB 还是 IPA，依据的是其内部内容，而不是文件名。

## 预算

```toml
[size]
max_total_mb = 25          # the download size, compressed
max_dex_mb = 8
max_native_mb = 12
max_resources_mb = 6
max_methods = 60000        # all dex files together
max_growth_percent = 5     # against a baseline build (--baseline, or ci --size-baseline)
```

大小以**压缩后**大小的兆字节（1,048,576 字节）计。每个键都是可选的，未知的键会报错，而没有基线构建的增长限制会被报告，而不是视为通过。在 CI 中：

```bash
emulabzip ci -o run.ezsession --size app-release.apk --size-baseline last-release.apk -- ./gradlew connectedCheck
```

构建会在任何内容运行之前被读取，因此文件缺失不会浪费一次测试运行。参见[预算](budgets.md)。

## 在桌面端

**Build size** 视图通过文件对话框打开一个构建（不需要设备），显示按类型划分的条形图、原生代码、dex 文件和最大的文件，并可通过 **Compare with…** 与第二个构建比较。

## 限制

- 它不会打开编译后的 manifest 或 dex 字节码：只读取大小、文件夹和 dex 文件头。如需类级别的细分，请使用 Android Studio 的 APK Analyzer。
- 它目前还不能从设备读取已安装的应用：请先拉取文件（`adb shell pm path`、`adb pull`）。
- 通过 MCP（`analyze_package`、`diff_packages`），只接受扩展名为 `.apk`、`.aab` 或 `.ipa` 的文件。
