---
source: 0f80060ba988
description: 介绍 Devices 视图以及 avd、sim 和 snapshot 命令，如何创建、启动、停止、清除和删除模拟器，并管理快照，同时说明哪些操作需要 --yes。
---
# 设备：模拟器

<p align="center">
  <img src="images/devices-manage.png" alt="Devices 视图：正在运行或未运行的模拟器，带有 Start、Headless、Wipe 和 Delete" />
</p>

**Devices** 视图以及 `avd`、`sim` 和 `snapshot` 命令可以创建、启动、停止、清除和删除模拟器，
并保存模拟器的快照。在此之前，EmuLabZip 只能看到已经在运行的设备。

```bash
emulabzip devices --all                       # running or not; a stopped one has the state "shutdown"
emulabzip avd options                         # system images and device definitions
emulabzip avd create Pixel_Test --image "system-images;android-35;google_apis;arm64-v8a" --model pixel_6
emulabzip avd boot Pixel_Test --headless
emulabzip snapshot save before-login -d emulator-5554
emulabzip snapshot load before-login -d emulator-5554 --yes
emulabzip avd shutdown Pixel_Test
emulabzip avd wipe Pixel_Test --yes
emulabzip avd delete Pixel_Test --yes
```

`sim` 对 iOS 模拟器的作用相同（`sim options` 列出运行时和设备类型；`sim wipe` 也可写作 `sim erase`）。

## 哪些操作需要 `--yes`

任何会丢弃数据的操作：`wipe`（`erase`）、`delete`、`boot --wipe`，以及 `snapshot load` 和 `delete`。命令会在查找任何设备之前
说明它需要什么。桌面应用会用对话框询问。启动、停止、创建和保存快照则不需要。

正在运行的设备不会被删除，运行中的 Android 模拟器也不会被清除：请先将其关闭。

## Android

| | 方式 |
|---|-----|
| 列表 | `~/.android/avd/*.ini`（或 `ANDROID_AVD_HOME`）与 `adb` 看到的内容合并；未运行的模拟器 id 为 `avd:<name>` |
| 选项 | SDK 下已安装的系统镜像，以及 `avdmanager list device` |
| 创建、删除 | `avdmanager` |
| 启动 | `emulator -avd <name>`，以分离方式运行，直到 `sys.boot_completed`；`--headless` 没有窗口，`--cold` 跳过快速启动快照，`--wipe` 从出厂状态启动 |
| 停止 | 模拟器控制台（`kill`） |
| 清除 | 删除 AVD 的 `userdata-qemu.img`、缓存和快照，使其下次从出厂状态启动（模拟器必须已停止）；`boot --wipe` 以 `-wipe-data` 启动 |
| 快照 | 控制台：`avd snapshot list`、`save`、`load`、`del` |

SDK 通过 `ANDROID_HOME`、`ANDROID_SDK_ROOT` 和常用文件夹找到。运行中的模拟器报告的名称会把
下划线换成空格；EmuLabZip 会将两者匹配。

## iOS Simulator

`simctl create`、`boot`、`shutdown`、`erase` 和 `delete`，使用 `simctl list` 中的运行时和设备类型标识符。这里会拒绝 `delete` 已启动的模拟器（`simctl` 本身
是允许的）。

## 名称

名称由字母、数字、`.`、`_` 和 `-` 组成，最长 64 个字符，因此它绝不可能向 `avdmanager`、
`emulator` 或 `simctl` 传递其他内容。镜像和运行时还允许 `;` 和 `:`。

## 面向助手

通过 [MCP](mcp.md)：`list_all_devices`、`boot_device`、`shutdown_device`、`list_emulator_snapshots` 和
`save_emulator_snapshot`。启动绝不会清除数据。清除、删除和加载快照**不**提供。
