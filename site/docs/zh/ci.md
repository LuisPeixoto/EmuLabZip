---
source: 9f0d25bee3d4
description: 介绍 emulabzip ci：在任意命令（通常是测试运行）周围录制会话，保留命令本身的退出码，并说明各选项、GitHub Actions 示例和使用注意事项。
---
# CI 模式

`emulabzip ci` 会在任意命令（通常是测试运行）周围录制一个[会话](sessions.md)，然后以该命令自己的退出码退出，因此你的流水线行为不变，并且在出问题时有一份录制可以查看。

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

它会开始录制，运行命令（其输出原样透传），停止，写入会话，并打印一行结果。

| 选项 | |
|--------|---|
| `-o, --output FILE` | 会话的写入位置（必需） |
| `-a, --app ID` | 应用：会针对它采样，崩溃也仅限于它 |
| `--network` | 录制 HTTP(S) 流量：运行期间设备会指向代理 |
| `--screens N` | 每 N 秒一张截图 |
| `--fail-on-crash` | 当应用在运行期间崩溃时让运行失败（退出码 1），即使命令通过了 |
| `--summary FILE` | 同时把报告以 Markdown 格式写入此文件 |
| `--config FILE` | 用来约束本次运行的限制（默认：存在时为 `./emulabzip.toml`）。参见[预算](budgets.md) |
| `--baseline FILE` | 用于比较的较早会话 |
| `--fail-on-regression` | 当某个数值比基线差出的幅度超过容差时失败 |
| `--video [N]` | 录制屏幕，每段 N 秒（默认 30）。参见[时间线与屏幕录像](timeline.md) |
| `--taps` | 录制触摸（Android） |
| `--replay FILE` | 用 HAR 文件或会话来应答应用的请求，使运行不依赖任何服务器。需要 `--network`。参见[回放录制](replay.md) |
| `--passthrough` | 与 `--replay` 一起使用：让不在录制中的请求到达真实服务器 |
| `--fail-on-miss` | 与 `--replay` 一起使用：当应用请求了录制中没有的内容时让运行失败 |
| `--size BUILD` | 让 APK、AAB 或 IPA 遵守 `emulabzip.toml` 的 `[size]` 限制。在运行之前读取。参见[构建大小](package-size.md) |
| `--size-baseline BUILD` | 与 `--size` 一起使用：用于衡量增长的较早构建 |
| `--fail-on-security LEVEL` | 在命令结束后审计应用的安全与隐私（需要 `--app`；包含代理的请求），并在出现至少达到此严重程度的发现时失败：`note`、`warning` 或 `error`。参见[安全审计](security-audit.md) |
| `--visual NAME` | 把命令结束后的屏幕与基线比较。参见[视觉回归](visual-regression.md) |
| `--fail-on-visual` | 与 `--visual` 一起使用：当屏幕差异超过限制时让运行失败 |

退出码是命令的退出码。如果命令通过了，但 `--fail-on-crash` 发现了崩溃、某个[预算](budgets.md)被突破，或者要求回归使运行失败，则为 1；如果你按下 Ctrl+C，则为 130。

当设置了 `GITHUB_STEP_SUMMARY` 环境变量（GitHub Actions）时，Markdown 报告会追加到其中，无需额外步骤即可显示在运行页面上。

## GitHub Actions

```yaml
name: UI tests
on: [pull_request]

jobs:
  android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17 }
      - name: Install EmuLabZip
        run: cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
      - uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: 34
          arch: x86_64
          script: >
            emulabzip ci -o run.ezsession --app com.example.app --fail-on-crash --screens 10
            -- ./gradlew connectedCheck
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: session
          path: run.ezsession
```

下载产物，并在 **Sessions → Open…** 中打开它，或用 `emulabzip session show` 读取。

你也可以在同一个任务中对无障碍设置门槛，在应用显示到需要检查的屏幕之后：

```bash
emulabzip a11y --fail-on error
```

## 注意事项

- 模拟器必须在 `ci` 启动之前运行；有多台设备时，请传入 `--device`。
- 已存在的会话文件绝不会被覆盖。
- 如果使用了 `--network`，设备必须信任 EmuLabZip 证书（参见 [Network](network.md#the-certificate)）。在 CI 上，请在可获取 root 的镜像上运行一次 `emulabzip ca install`。
