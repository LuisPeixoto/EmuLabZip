---
source: dabee4094ffd
description: 介绍 Compare 视图：在两台设备上同时使用应用时并排查看它们，包括双屏幕、CPU 与内存图表、差异列表，以及对应的终端命令。
---
# 比较两台设备

**Compare** 会在你于两台设备上使用应用时，把它们并排放在一起：旧的 Android 对比新的 Android，Pixel 对比 iPhone，手机对比平板。

启动第二个模拟器后，它就会出现在 **against** 选择器中。随后该视图会提供：

- **Both screens**，每隔几秒刷新一次。
- 两台设备的 **CPU 和内存图表**同时显示，针对工具栏中选择的应用（当它在两台设备上都运行时；否则为整台设备的数据），这样你就能看出哪一台负担更重。
- **Differences**：两台设备报告的每一项信息（操作系统、构建、硬件、显示、电池、语言区域）并排对齐，并高亮显示不同之处。勾选 **Show what matches** 可查看完整列表。

在终端中：

```bash
emulabzip compare emulator-5554 "iPhone 17"          # only what differs
emulabzip compare emulator-5554 emulator-5556 --all  # everything
emulabzip compare emulator-5554 emulator-5556 --json
```

某台设备没有的信息显示为 `—`。允许比较 Android 设备和 iOS 设备，不过大多数行都会不同，因为两个平台对事物的命名方式不一样。
