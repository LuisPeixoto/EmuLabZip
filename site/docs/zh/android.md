---
source: 854d8470ae63
description: 说明 EmuLabZip 如何通过 ADB 主机协议连接 Android 模拟器和真机，包括可用设置、收集的数据来源、设备状态和使用提示。
---
# Android

EmuLabZip 支持 Android 模拟器，以及已启用 USB 或无线调试的真机。

## 连接方式

EmuLabZip 直接使用 **ADB 主机协议**与 `127.0.0.1:5037` 上的 ADB 服务器通信，这与 `adb`
命令和 Android Studio 使用的是同一个服务器。设备上不会安装任何东西：每次查询都是一条你自己也可以用
`adb shell` 运行的 shell 命令。

| 设置 | 作用 |
|---------|--------|
| `ADB_SERVER_SOCKET=tcp:host:port` | 使用另一个 ADB 服务器，例如从远程机器转发过来的服务器。 |
| `ANDROID_ADB_SERVER_PORT=5038` | 使用另一个本地端口。 |
| `EMULABZIP_ADB=/path/to/adb` | 服务器未运行时，用来启动它的 `adb` 二进制文件。 |
| `ANDROID_HOME`, `ANDROID_SDK_ROOT` | 查找 SDK 的位置。 |

## 收集的内容

| 功能 | 来源 |
|---------|--------|
| 设备列表 | `host:devices-l`，然后对每个在线设备执行 `getprop` |
| 名称 | 模拟器使用 AVD 名称（`ro.boot.qemu.avd_name`），否则使用型号 |
| Overview | `getprop`、`wm size`、`wm density`、`dumpsys battery`、`/proc/meminfo`、`uname -r`、`df`（一次批量调用） |
| 应用 | `pm list packages -3` 和 `-s`，进程 id 来自 `ps -A` |
| 日志 | `logcat -v threadtime -T <history>` |
| 截图 | 通过 `exec:` 执行 `screencap -p`，使 PNG 不被改动 |

## 设备状态

| 状态 | 含义 |
|-------|---------|
| Online | 已启动并就绪。 |
| Booting | 已连接，但尚未设置 `sys.boot_completed`。EmuLabZip 会等待它。 |
| Unauthorized | 在设备上接受 *Allow USB debugging* 提示。 |
| Offline | ADB 服务器丢失了该设备。重启模拟器，或运行 `adb reconnect`。 |

## 提示

- 运行 **Google Play** 镜像的模拟器是 `user` 构建：`run-as` 只对可调试的应用有效，这对第 2 阶段的
  Storage 工具很重要。
- 可以同时运行多个模拟器；用设备选择器在它们之间切换。
