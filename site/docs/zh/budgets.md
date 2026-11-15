---
source: 4044d9ed6c76
title: 预算与基线：让 CI 在成本超标时失败
description: 在 emulabzip.toml 中写下一次运行允许消耗的上限，emulabzip ci 会在超标时让运行失败；还可以与之前的会话基线对比，发现性能回退。
---
# 预算与基线

写下一次运行允许消耗多少，超过时 `emulabzip ci` 就会让这次运行失败。

## `emulabzip.toml`

把它放在项目文件夹中（或传入 `--config`）。每个限制都是可选的；运行只会受已写下的限制约束。

```toml
[budget]
start_time_ms = 800        # the median start time; needs [startup]
memory_peak_mb = 300
cpu_avg_percent = 40
janky_frames_percent = 5
requests = 200
failed_requests = 0
log_errors = 10
crashes = 0

[startup]                  # optional: measure the start time before the command runs
runs = 5
kind = "cold"              # cold, warm or hot

[baseline]
tolerance_percent = 10     # how much worse than the baseline counts as a regression

[size]                     # a build's size: see Build size (package-size.md)
max_total_mb = 25
max_dex_mb = 8
max_native_mb = 12
max_methods = 60000
max_growth_percent = 5

[visual]                   # screenshots: see Visual regression (visual-regression.md)
threshold_percent = 0.1
tolerance = 8
```

`[size]` 通过 `emulabzip size show --budget` 和 `emulabzip ci --size` 对构建文件进行检查；`[visual]` 设置 `emulabzip visual` 如何比较，以及忽略屏幕的哪些部分。请参阅[构建大小](package-size.md)和[视觉回归](visual-regression.md)。

不存在的键会报**错误**，因此拼写错误不会让某个限制悄悄失效。无法测量的限制视为超标：没有 `[startup]` 表和 `--app` 的 `start_time_ms` 会失败，而不是悄悄通过。

## `ci` 中的预算

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

运行结束后，会检查各项限制，并在 Markdown 报告的 **Budgets** 下列出。如果有任何一项超标，退出码为 1；如果命令本身失败，则以它自己的退出码为准。

## 基线

把一次运行与更早的会话对比，通常是最近一次正常的会话：

```bash
emulabzip ci -o run.ezsession --app com.example.app --baseline last-good.ezsession --fail-on-regression -- ./test.sh
emulabzip session compare last-good.ezsession run.ezsession
```

当一个数字的增长超过容差（默认 10%，在 `session compare` 上用 `--tolerance`）时，它就是**更差**。报告会把每个数字列为更好、更差或相同。没有 `--fail-on-regression` 时，回退只会被报告。

在 GitHub Actions 中，把最近一次正常的会话保存为默认分支的缓存或制品，并在运行前下载它：

```yaml
- uses: actions/cache@v4
  with:
    path: baseline.ezsession
    key: baseline-${{ github.base_ref || github.ref_name }}
    restore-keys: baseline-
- run: >
    emulabzip ci -o run.ezsession --app com.example.app
    --baseline baseline.ezsession --fail-on-regression -- ./gradlew connectedCheck
- if: github.ref_name == 'main'
  run: cp run.ezsession baseline.ezsession
```

启动耗时保存在会话中（`startup.json`），因此也可以对比。在此之前生成的会话仍然可用：该文件是可选的，格式版本仍为 1。
