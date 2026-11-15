---
source: a19d308974b9
title: 能耗测量：应用使用期间的耗电估算
description: 测量应用在你使用期间一段时间内的能耗：Android 上给出以 mAh 估算的用量、唤醒锁和网络字节数，iOS 模拟器上给出 CPU 时间和热状态。
---
# 能耗

测量应用在你使用它期间的一段时间内消耗了多少能量。

```bash
emulabzip energy com.example.app --seconds 30
emulabzip energy com.example.app --json
```

启动后使用应用，时间到了，或你按下 <kbd>Ctrl+C</kbd> 时，它会读取结果。

## Android

设备在电池服务看来被拔掉了电源（`dumpsys battery unplug`），并且统计数据被重置（`dumpsys batterystats --reset`），这样它们就是在设备上计数的。结束时，会针对应用的 uid 读取 `dumpsys batterystats --charged <package>`，并且**始终**会还原电池（`dumpsys battery reset`），出错或按 <kbd>Ctrl+C</kbd> 之后也是如此。

报告包含：

- 系统的**估算用量**（单位 mAh），以及它的组成部分（CPU、无线电、屏幕、传感器…）；
- 应用使用的**处理器时间**；
- **唤醒锁**（每个持有了多久、多少次）、**闹钟**和**作业**；
- 接收和发送的**网络**字节数。

模拟器没有电池。Android 根据功耗模型计算这些数字，因此请拿一次运行与另一次运行比较，而不要与真机比较。在这段时间内用量太少、或没有运行的应用不会得到估算值，报告会说明这一点。

## iOS 模拟器

模拟器没有功耗模型，因此没有 mAh 数值。报告给出应用使用的 **CPU 时间**（真机上最先消耗电量的部分）和 Mac 的**热状态**（`pmset -g therm`）：当 Mac 降低处理器速度时，两次运行就不可比较。

## 在哪里找到它

- **桌面端**：Performance → **Energy**（Start，然后 Stop 并读取）。
- **CLI**：`emulabzip energy`。
- **MCP**：`measure_energy`（最长 120 秒）。
