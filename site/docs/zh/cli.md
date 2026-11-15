---
source: 951822beac6a
description: emulabzip 命令行参考：在终端或脚本中获得与桌面应用相同的数据，涵盖设备选择、日志、性能、网络、存储、会话、插件、MCP 和环境变量。
title: emulabzip 命令行参考：命令、选项与环境变量
---
# 命令行

`emulabzip` 命令让你在终端或脚本中获得与桌面应用相同的数据。它是单个静态二进制文件，除 `adb` 和 Xcode 外没有任何运行时依赖。

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

适用于 macOS、Linux 和 Windows 的预构建压缩包附在每个[版本](https://github.com/LuisPeixoto/EmuLabZip/releases)中。

## 选择设备

除 `devices` 外，每个命令都只作用于一台设备。只有一台设备在运行时会自动使用它。否则请传入 `--device`（或设置 `EMULABZIP_DEVICE`），值为 adb 序列号、模拟器 UDID、名称或名称的一部分：

```bash
emulabzip --device pixel info
emulabzip -d "iPhone 17" logs
```

## 命令

### `emulabzip devices`

```text
PLATFORM  NAME       OS                   STATE   ID
Android   Pixel 9    Android 16 (API 36)  online  emulator-5554
iOS       iPhone 17  iOS 26.2             online  F8CE84C1-04D1-4E33-B52B-02B59E8A09C5
```

`--json` 以 JSON 输出列表。别名 `emulabzip ls` 作用相同。

### `emulabzip info`

输出与 Overview 相同的分组（设备、软件、硬件、显示、电池、区域设置）。`--json` 输出由对象组成的对象。

### `emulabzip apps`

列出用户应用及其版本和进程 id。`--all` 会加入系统应用，`--running` 只保留正在运行的应用。

### `emulabzip logs`

跟随设备日志，按级别着色，直到你按下 <kbd>Ctrl+C</kbd>。

| 选项 | 含义 |
|--------|---------|
| `-a, --app <id>` | 只显示来自此应用的行。进程 id 每隔几秒刷新一次，因此会跟随重启。 |
| `-l, --level <level>` | 最低级别：`verbose`、`debug`、`info`、`warn`、`error`、`fatal`，或一个字母。 |
| `-t, --tag <text>` | 标签包含该文本。 |
| `-g, --grep <text>` | 消息或标签包含该文本。 |
| `-E, --regex` | 将 `--grep` 视为正则表达式。 |
| `--tail <n>` | 先输出的最近行数（Android，默认 100）。 |
| `--json` | 每行一个 JSON 对象，可直接用于 `jq`。 |

```bash
emulabzip logs --app com.example.app --level warn
emulabzip logs -g 'skipped \d+ frames' -E
emulabzip logs --json | jq -r 'select(.level == "error") | .message'
```

### `emulabzip screenshot`

将屏幕保存为 PNG。`-o, --output <file>` 指定路径；默认情况下文件以设备名称和时间命名。

### `emulabzip perf`

每个采样输出一行：应用和设备的 CPU、应用和设备的内存、帧率、卡顿帧和电池。

| 选项 | 含义 |
|--------|---------|
| `-a, --app <id>` | 要采样的应用。没有它则只有设备范围的值。 |
| `-i, --interval <ms>` | 采样间隔（默认 1000，最小 250）。 |
| `--json` | 每个采样一个 JSON 对象。 |

### `emulabzip mem <app>`

内存明细：Android 上按类别显示 PSS，iOS 上按 VM 区域显示占用。参见 [Memory](memory.md)。

### `emulabzip heapdump <app>`

保存 `.hprof` 堆转储（Android，可调试的应用）或 `.memgraph` 内存图（iOS）。`-o` 指定文件。使用 `--analyze` 时还会查找泄漏，参见[查找泄漏](leaks.md)。

### `emulabzip heap analyze <file>`

在 `.hprof`（任何平台）或 `.memgraph`（在 Mac 上）中查找泄漏。`--json`；`--top <n>` 设置列出的对象和类的数量；`--all-heaps` 会查看应用自身堆之外的部分。参见[查找泄漏](leaks.md)。

### `emulabzip startup <app>`

测量应用打开所需的时间。`-k, --kind cold|warm|hot`（默认 `cold`），`-r, --runs <n>`（1 到 50，默认 5），`--json`。参见[应用启动耗时](startup.md)。

### `emulabzip energy <app>`

测量你使用应用时它消耗的能量。`-s, --seconds <n>`（默认 30；<kbd>Ctrl+C</kbd> 会读取已有的数据），`--json`。参见 [Energy](energy.md)。

### `emulabzip files <app> [path]`

列出应用的一个文件夹。`--root` 选择存储根目录（Android 上为 `data` 或 `external`；iOS 上为 `data`、`bundle` 或 `group:<id>`）；默认使用第一个可读的根目录。参见 [Storage](storage.md)。

### `emulabzip pull <app> <path>`

将一个文件复制到这台电脑。`-o` 指定目标位置。

### `emulabzip db <app> <path>`

以只读方式打开 SQLite 数据库。不带选项时列出表和视图；`--table` 输出行（`--limit`、`--offset`）；`-q, --query` 运行只读语句。它们都接受 `--json`。

### `emulabzip net`

运行代理，让设备指向它，并在每个请求结束时输出它，直到 <kbd>Ctrl+C</kbd>。停止时设备会被恢复，出错之后也一样。参见 [Network](network.md)。

| 选项 | 含义 |
|--------|---------|
| `-p, --port <n>` | `127.0.0.1` 上的端口（默认 9090，或 `EMULABZIP_PORT`）。 |
| `--no-connect` | 只运行代理；不更改任何设备。 |
| `--har <file>` | 停止时将请求保存为 HAR 文件。 |
| `--json` | 每个请求一个 JSON 对象。 |
| `--rules <file>` | 使用 JSON 文件中的规则模拟、延迟、拦截、映射到文件或其他服务器、对照 OpenAPI 检查，或用脚本处理请求。参见[模拟响应与断点](mocks.md)和[网络规则](network-rules.md)。 |
| `--http3` | 与提供 HTTP/3 的服务器使用 HTTP/3 通信。`--show-h3` 还会显示向应用提供它的情况。 |
| `--conditions <preset>` | 减慢网络：`fast-3g`、`slow-3g`、`slow-wifi`、`high-latency`、`packet-loss` 或 `offline`。参见[网络状况](network-conditions.md)。 |
| `--down <kbps>`, `--up <kbps>`, `--latency <ms>`, `--loss <percent>` | 你自己的数值，或对预设的调整。 |

每个请求之后会输出 GraphQL 操作以及 gRPC 方法和状态。

`emulabzip net import <file> [--har <file>] [--json] [--dns]` 读取 `.pcap` 或 `.pcapng` 抓包文件，并输出其中的请求，需要时还包括 DNS 查询。参见[导入抓包文件](pcap.md)。

### `emulabzip layout` 和 `emulabzip a11y`

`layout` 以树的形式输出屏幕上的视图（`--json` 输出全部内容，`--png <file>` 保存同时截取的屏幕截图，`--depth <n>`，`--app <id>` 用应用的框架标记 iOS 屏幕）。`a11y` 审计屏幕并输出发现的问题；`--fail-on note|warning|error` 在发现至少该严重程度的问题时以 1 退出。参见[布局与无障碍](layout.md)。

### `emulabzip frameworks <app>`

列出应用所用的 UI 框架（Flutter、React Native、Hermes、Jetpack Compose、SwiftUI）以及检测到它们的依据。`--json`。参见[深入框架内部](frameworks.md)。

### `emulabzip flutter tree|frames|memory <app>`

通过 Dart VM 服务查看 Flutter 调试或性能分析（profile）应用的内部。`--uri <address>` 给出它监听的地址（日志中没有时使用）；`tree` 有 `--depth <n>`，`frames` 有 `-s, --seconds <n>`（默认 10），`memory` 有 `--top <n>`。它们都接受 `--json`。

### `emulabzip rn tree|heap|console|state <app>`

通过 Metro（`--metro <port>`，默认 8081）查看 React Native 开发应用的内部。`tree` 有 `--depth <n>`，`console` 有 `-s, --seconds <n>`。`state` 读取 Redux、React Query、上下文和已注册的存储；`--watch` 跟随 Redux action 并输出变化的内容。参见 [React Native 状态](react-native-state.md)。它们都接受 `--json`。Metro 一次只允许一个调试器连接。

### `emulabzip state save|restore|show|diff`

`save <app> -o file.ezstate [--external]` 将应用的数据写入一个文件。`restore <file> --yes` 用它替换应用的数据，并先保存被替换的内容。`show <file>` 说明文件包含什么。`diff <a> [b]` 列出两个文件之间，或一个文件与应用当前状态之间，发生变化的表、列、索引和键。参见[应用状态](app-state.md)。

### `emulabzip edit`

更改设备上的数据：`cell`（`-t <table>`、`--rowid <n>` 或 `--pk col=value`、`--set`、`--text`、`--null`）、`insert`、`delete`、`pref-set <key> <value> [--as type]`、`pref-remove`、`plist-set <key/path> <value> [--as type]`、`plist-remove` 和 `undo <folder>`。每个命令都接受应用和文件，并且**每一个都需要 `--yes`**。参见[更改应用的数据](storage-editing.md)。

### `emulabzip avd`、`sim` 和 `snapshot`

`avd` 和 `sim` 有 `list`、`options`、`create <name> --image <id> --model <id>`、`boot <device>`（`--headless`、`--cold`、`--wipe`）、`shutdown`、`wipe`（在 `sim` 上为 `erase`）和 `delete`。`snapshot` 有 `list`、`save`、`load` 和 `delete`，作用于用 `-d` 选取的运行中的模拟器。`wipe`、`delete`、`boot --wipe`、`snapshot load` 和 `snapshot delete` 需要 `--yes`。`emulabzip devices --all` 还会列出未运行的设备。参见[设备](devices.md)。

### `emulabzip audit security <app>`

查找日志、偏好设置和数据库中的密钥与个人数据，请求中的明文 HTTP 和密钥（`--har`），`debuggable` 和 `allowBackup` 标志，其他应用可以访问的组件，App Transport Security 例外，以及隐私清单的缺漏。`--only manifest,logs,storage,traffic`、`--json`、`--fail-on <note|warning|error>`。证据会被截短，因此密钥永远不会被重复输出。参见[安全审计](security-audit.md)。

### `emulabzip size show|diff`

`show <file>` 按类型、原生 ABI 和 dex 文件拆解 `.apk`、`.aab` 或 `.ipa`（`--top`、`--json`）；使用 `--budget` 时，它会让构建符合 `emulabzip.toml` 中 `[size]` 部分的要求（`--config`、`--baseline`），超出限制时以 1 退出。`diff <before> <after>` 说明发生了什么变化（`--fail-on-growth <percent>`）。参见[构建大小](package-size.md)。

### `emulabzip i18n check|sweep`

`check` 查找屏幕上按原样显示时被截断、超出屏幕、被裁剪或未镜像的文字（`--rtl`、`--fail-on`）。`sweep` 在其他语言和文字大小下尝试（`--locales en-XA,ar-XB`、`--scales 100,130,200`、`--app`、`--fail-on-worse`），并恢复设备。参见[语言、文字大小与放不下的文字](localization.md)。

### `emulabzip control locale|reset-locale|font-scale|display|reset-display`

更改应用（Android）或模拟器（iOS）的语言、文字大小，以及 Android 的显示大小。`a11y --simulate <kind> [-o file]` 显示色盲者看到的屏幕。

### `emulabzip visual capture|update|compare|list|diff|session`

屏幕的基线，以及此后发生的变化。`capture` 和 `compare` 接受 `--status-bar`（干净的 9:41 状态栏）、`--dir`、`--threshold`；`update` 需要 `--yes`。`compare` 超出限制时以 1 退出，并绘出差异。参见[视觉回归](visual-regression.md)。

### `emulabzip analytics`

运行代理，并输出应用发送给 Segment、Amplitude、Mixpanel 和 Firebase 的事件（`--service`、`--port`、`--no-connect`、`--json`），或使用 `--firebase-log` 输出设备日志中 Firebase 的调试行。参见[分析检查器](analytics.md)。

### `emulabzip session` 和 `emulabzip ci`

`session record -o run.ezsession [--app <id>] [--network] [--screens <s>] [--duration <s>] [--video [<s>]] [--taps]
[--replay <file> [--passthrough] [--timing]]` 录制直到 <kbd>Ctrl+C</kbd>；`--video` 分段录制屏幕（默认 30 秒，1 到 180），`--taps` 录制触摸，`--replay` 在此期间用录制内容应答应用的请求（需要 `--network`）。`session show <file> [--json | --markdown]` 汇总一个会话，或被中断会话的 `.partial` 文件夹。`session timeline <file> [--lane <name>…] [--json]` 在同一条时间轴上输出触摸、请求、日志问题、慢帧、垃圾回收、崩溃、屏幕截图和视频，`session video <file> [-o
<dir>]` 将屏幕录制的各个分段写出。`ci -o run.ezsession [--app <id>] [--network] [--screens <s>]
[--fail-on-crash] [--summary <file>] [--video [<s>]] [--taps] [--replay <file> [--fail-on-miss]] -- <command…>` 围绕一个命令进行录制，并以该命令的退出码退出。参见[会话](sessions.md)、[时间线与屏幕视频](timeline.md)、[回放录制](replay.md)和 [CI 模式](ci.md)。

### `emulabzip compare <a> <b>`

列出两台设备之间的差异（`--all` 同时列出相同之处，`--json`）。参见[比较设备](compare.md)。

### `emulabzip crashes` 和 `emulabzip crash <id>`

`crashes` 列出设备记录的内容，最新的在前，每条都有一个 id；`-a, --app` 和 `-k, --kind
crash|native|anr` 可缩小范围。`crash` 显示一条记录及其堆栈和提示：`--all-threads` 输出每个线程，`--raw` 按设备写入的样子输出报告，`-m, --mapping <file>` 使用你指定的 `mapping.txt`、source map 或 `.dSYM` 来读取它，此外还会使用为其应用和版本保存的那些。两者都接受 `--json`。参见[崩溃](crashes.md)和[可读的堆栈](symbolication.md)。

### `emulabzip mappings` 和 `emulabzip symbolicate`

`mappings add <app> <version> <file>` 为一个构建保存 `mapping.txt`、source map 或 `.dSYM`（无法从文件判断时用 `-k, --kind
mapping|sourcemap|dsym`），`mappings list [app] [--json]` 显示已保存的内容，`mappings remove <app> <version> [-k <kind>]` 将其删除。`symbolicate <stack.txt|-> -m <file>` 读取你粘贴的堆栈，来自文件或标准输入。参见[可读的堆栈](symbolication.md)。

### `emulabzip report`

将缺陷报告所需的内容写入一个 HTML 文件：`[--session <file>] [-a, --app <id>] [-o <file>] [--title <text>]
[--description <text>] [--without screenshot|logs|requests|crashes|video] [--issue github:owner/repo | gitlab:[host/]group/project |
jira:site/PROJECTID/ISSUETYPEID] [--open]`。使用 `--issue` 时，它会输出一个链接，打开已填好摘要的问题，`--open` 会在浏览器中打开它。参见[缺陷报告](bug-reports.md)。

### `emulabzip control`

更改设备。参见 [Controls](controls.md)。

| 命令 | 作用 |
|---------|--------------|
| `control info [-a <app>]` | 设备支持什么、它的外观，以及应用的权限。 |
| `control location set <lat> <lon>` / `location clear` | 设置或重置位置。 |
| `control appearance light\|dark` | 浅色或深色模式。 |
| `control statusbar set [--time 9:41] [--battery 100] [--wifi true] [--cellular true]` / `statusbar clear` | 干净的状态栏，或恢复真实的状态栏。 |
| `control battery set <level> [--charging]` / `battery reset` | 假装电池处于某个电量（Android）。 |
| `control permission <app> <permission> grant\|revoke\|reset` | Android 权限名称，或 iOS 服务，例如 `photos`。 |
| `control notify [<app>] [--title …] [--body …] [--payload <file>]` | 通知（Android）或模拟推送（iOS）。 |
| `control open <url>` | 在设备上打开链接。 |
| `control tap <x> <y>` / `control swipe <x1> <y1> <x2> <y2> [--ms n]` | 触摸或拖动，以像素为单位（Android）。参见[输入与流程](automation.md)。 |
| `control text <text>` / `control key <name>` | 输入可打印的 ASCII，或按下一个键（Android）。 |
| `control launch <app>` | 启动应用。 |

### `emulabzip webview`

Android 设备上应用的 WebView 或 Chrome 内的页面。参见[应用中的网页](webviews.md)。

| 命令 | 作用 |
|---------|--------------|
| `webview list [--json]` | 可以检查的套接字及其页面。 |
| `webview console [--seconds n]` | 页面写入控制台的内容，以及它没有捕获的错误。 |
| `webview eval <expression>` | 在页面中运行 JavaScript 并输出值。脚本抛出异常时失败。 |
| `webview network [--seconds n] [--reload] [--har <file>]` | 页面发出的请求，按浏览器看到的样子。 |

每个命令都接受 `--app <package>`、`--socket <name>` 和 `--page <id or part of its address>` 来指定哪个页面，以及 `--json`。

### `emulabzip flow`

将触摸录制为流程，回放它，并导出它。参见[输入与流程](automation.md)。

| 命令 | 作用 |
|---------|--------------|
| `flow record --name <name> [--app <app>] [--no-launch] [--seconds n] [--out <file>]` | 录制设备上的触摸，直到 <kbd>Ctrl+C</kbd>。 |
| `flow list`, `flow show <flow> [--json]`, `flow remove <flow>` | 已保存的流程、某个流程的步骤，以及删除某个流程。流程可以是它的 id 或一个 `.json` 文件。 |
| `flow play <flow>` | 在设备上回放它，并按其屏幕缩放。 |
| `flow export <flow> --maestro\|--script [--out <file>]` | Maestro 流程，或由 `emulabzip control` 命令组成的 shell 脚本。 |

### `emulabzip link`

打开深层链接，并检查域名和应用对它的看法是否一致。参见[深层链接](deep-links.md)。

| 命令 | 作用 |
|---------|--------------|
| `link open <url> [--app <app>]` | 在设备上打开它，并保留在历史记录中。 |
| `link check <url> [--app <app>] [--json]` | 读取域名的 `assetlinks.json` 或 `apple-app-site-association`，询问设备，并列出发现的问题及其修复方法。当有问题时以错误退出。 |
| `link verify <app>` | 让 Android 再次验证应用的域名。 |
| `link history [--json]` | 以前打开和检查过的链接。 |

### `emulabzip openapi check`

`openapi check --spec <file> --har <file> [--json]` 对照 OpenAPI 3 描述检查 HAR 文件中的每个响应，有不匹配时以错误退出。参见[网络规则](network-rules.md#contract)。

### `emulabzip script`

批准规则所运行的 WebAssembly 脚本。`script show <file>`、`script approve <file>`、`script revoke <file or hash>` 和 `script list`。参见[规则脚本](rule-scripts.md)。

### `emulabzip plugin`

运行你自己的采集器。参见[插件](plugins.md)。

| 命令 | 作用 |
|---------|--------------|
| `plugin list [--json]` | 已安装的插件，以及每个插件是否已启用。 |
| `plugin new <name>` | 从模板开始创建插件。 |
| `plugin install <folder> [--force]` | 复制一个插件文件夹进来。它初始为禁用状态。 |
| `plugin enable <name>` / `plugin disable <name>` | 允许或不再允许插件运行。 |
| `plugin run <name> [-a <app>] [--seconds <n>] [--json]` | 在设备上运行插件，并输出它报告的内容。 |

### `emulabzip mcp [--allow-writes [--no-confirm]]`

通过标准输入和输出，以 MCP 向 AI 助手提供设备。参见 [MCP 服务器](mcp.md)和 [MCP 参考](mcp-tools.md)。

| 标志 | 作用 |
|------|------|
| `--allow-writes` | 同时提供会清除、删除、恢复或改写数据的工具。每次调用都会先请你确认 |
| `--no-confirm` | 与 `--allow-writes` 一起使用时，跳过询问。仅用于可丢弃设备上的无人值守场景 |

### `emulabzip trace`

录制并分析追踪。参见[追踪](traces.md)。录制期间按 <kbd>Ctrl+C</kbd> 会提前停止，并保留已录制的内容。

| 命令 | 作用 |
|---------|--------------|
| `trace record` | 录制 Perfetto 系统追踪（Android 9+）。`-p, --preset cpu\|frames\|memory\|custom`，用于 `custom` 的 `--config <file>`，`-D, --duration <s>`（默认 10），`-a, --app <pkg>`，`-o, --output <file>`，`--open` 在 Perfetto UI 中显示它。 |
| `trace cpu` | 使用 simpleperf 对应用（`-a, --app`）或整个系统（`--system`）采样，并输出热点函数。`-o` 保存报告，`--folded <file>` 保存折叠的堆栈，`--top <n>` 设置列表长度。 |
| `trace instruments` | 从模拟器录制 Instruments 模板（`-t, --template`，默认 Time Profiler）到一个 `.trace` 文档。需要开发者模式。 |
| `trace summary <file>` | Perfetto 追踪的 CPU、帧、内存和最繁忙的切片。`--json` 用于脚本。 |
| `trace query <file> "<sql>"` | 对 Perfetto 追踪运行 PerfettoSQL。`--json` 用于脚本。 |
| `trace open <file>` | 在 Perfetto UI 中打开 Perfetto 追踪。 |
| `trace list` | 列出在桌面应用中进行的录制。 |

`summary` 和 `query` 会在第一次使用时下载追踪处理器（参见[追踪](traces.md#the-trace-processor)）。

### `emulabzip ca install`

让设备信任该证书：可获得 root 的 Android 模拟器使用系统存储，iOS 模拟器使用钥匙串，其他 Android 设备则复制文件并输出步骤。`emulabzip ca path` 输出证书文件所在的位置。

## 环境

| 变量 | 作用 |
|----------|-------|
| `EMULABZIP_DEVICE` | `--device` 的默认值。 |
| `EMULABZIP_LOG=debug` | 在 stderr 上输出详细诊断。 |
| `EMULABZIP_HOME` | 证书颁发机构、已保存的代理设置、录制内容和追踪处理器所在的文件夹。 |
| `EMULABZIP_PORT` | `net --port` 的默认值。 |
| `ADB_SERVER_SOCKET`, `ANDROID_ADB_SERVER_PORT`, `EMULABZIP_ADB` | 参见 [Android](android.md)。 |
