---
source: fb1852db95e8
description: 以每次相同的方式测量应用打开所需的时间，并重复多次让中位数有意义；支持冷启动、温启动和热启动，可用于桌面端、CLI、MCP 和 CI。
---
# 应用启动耗时

测量应用打开需要多长时间，每次都用同样的方式，并重复几次，让中位数有意义。

```bash
emulabzip startup com.example.app                 # cold, 5 runs
emulabzip startup com.example.app --kind hot --runs 10
emulabzip startup com.example.app --json
```

| 类型 | Android | iOS 模拟器 |
|------|---------|---------------|
| **cold** | 先停止进程（`am force-stop`），让系统从零开始启动它 | 先终止应用 |
| **warm** | 用返回按钮关闭屏幕，并保留进程 | 不可用：模拟器没有返回按钮，因此没有单独的温启动 |
| **hot** | 用主页按钮把应用送到后台 | 把 Settings 应用放到前台 |

## 测量内容

- **Android** 使用 `am start -W`，它会报告 `TotalTime`（直到绘制出第一帧的时间）、`WaitTime` 和 `LaunchState`。启动器 Activity 通过 `cmd package resolve-activity` 找到。
- **iOS 模拟器** 读取统一日志。SpringBoard 会在被要求打开应用时记录一次，并在应用的 scene 变为活动状态时再记录一次。两者之间的时间就是用户实际等待的时间，包含动画。请拿多次运行互相比较，不要与真机比较。

结果会列出每次运行，以及中位数、最快、最慢和第 90 百分位。应关注的数字是中位数；单次较慢的运行通常只是噪声。

## 在哪里找到它

- **桌面端**：Performance → **Start time**（类型、运行次数、每次运行一根条形）。
- **CLI**：`emulabzip startup`。
- **MCP**：`measure_startup`。
- **CI**：在 [`emulabzip.toml`](budgets.md) 中有 `[startup]` 表时，`emulabzip ci --app …` 会在命令之前测量，并把中位数限制在 `start_time_ms` 以内。

运行期间应用会在设备上被停止并重新打开，因此不要在这样做会让你丢失工作的设备上运行它。
