---
source: a63f6e8c9dee
description: 按症状整理的故障排除指南，涵盖设备和模拟器不显示、日志、截图、网络、存储、崩溃、追踪、控制、插件和 MCP 等常见问题的原因与解决办法。
---
# 故障排除

## 我的 Android 模拟器没有显示

1. 运行 `adb devices`。如果模拟器没有列在那里，EmuLabZip 也看不到它。重启它，或运行
   `adb kill-server && adb start-server`。
2. 找不到 `adb` 时，设备选择器的页脚会显示 **adb unavailable**。请安装 Android SDK 的 platform
   tools，或将 `ANDROID_HOME` 或 `EMULABZIP_ADB` 指向你的 SDK。
3. 列为 **unauthorized** 的设备正在等你接受 USB 调试提示。
4. 自定义的 ADB 服务器端口也必须为 EmuLabZip 设置 `ANDROID_ADB_SERVER_PORT` 或 `ADB_SERVER_SOCKET`。

## 我的模拟器没有显示

- 只会列出**已启动的 iOS** 模拟器。用 `xcrun simctl list devices booted` 检查。
- 缺少 Xcode 命令行工具时，页脚会显示 **simctl unavailable**。运行 `xcode-select --install`，
  如果安装了多个 Xcode 版本，则运行 `sudo xcode-select -s /Applications/Xcode.app`。

## 日志是空的

- 在 Android 上，检查 `adb logcat` 是否会输出行。
- 查看最低级别和过滤器；当有日志行但没有一行匹配时，会出现 **Reset filters**。
- 应用过滤器需要应用正在运行。启动应用；过滤器会自动获取新的进程 id。

## 日志太繁忙

见[统一日志很繁忙](ios-simulator.md#the-unified-log-is-busy)。在 Android 上，提高最低级别或按
标签过滤。

## 截图失败

- 在 Android 上，`adb exec-out screencap -p > shot.png` 必须能正常工作。一些安全屏幕（DRM 视频、银行应用）
  按设计就是黑的。
- 在 iOS 上，`xcrun simctl io booted screenshot shot.png` 必须能正常工作。

## Storage 提示 “only debuggable apps can be browsed”

Android 只允许 `run-as` 读取可调试应用的私有数据。请安装你应用的 debug 构建，或用示例应用试用
这些工具（`tools/sample-app/install.sh`）。共享存储对每个应用都有效。

## Android 上堆转储失败

`am dumpheap` 需要可调试的应用，或 `userdebug`/`eng` 类型的模拟器镜像（不带 Google Play 的镜像）。
错误信息会显示 Android 给出的原因，例如 `Process not debuggable`。

## Performance 没有显示帧率

只有在应用绘制时才会统计帧。请操作应用；静止的屏幕不会渲染任何内容。在 iOS
模拟器上，帧指标会随第 4 阶段的 Instruments 追踪一起提供。

## Network：什么也没有显示

- 检查设备是否显示 *sending traffic here*。在 Android 上，`adb shell settings get global http_proxy` 应该
  输出 `127.0.0.1:<port>`。
- 有些应用会忽略系统代理（自己打开 socket 的应用，或使用 HTTP/3 的应用）。它们无法被捕获。
- 大量使用缓存的应用可能根本不请求网络。清除它们的数据，或使用一个新的 URL。

## Network：HTTPS 请求失败或显示为 “Connections”

设备不信任该证书，或者应用固定了其服务器的证书。点击 **Certificate** 并按提示操作。在来自 Google Play 的 Android 镜像上，只有 Chrome 和允许用户证书的应用才会信任它；见
[证书](network.md#the-certificate)。横幅会列出正在直通的主机；修复之后请使用 **Decrypt
again**。

## Network：设备或 Mac 仍然指向代理

点击 **Disconnect**，或者在 Android 上运行 `adb shell settings put global http_proxy :0`。对于 Mac，请打开一次 EmuLabZip：
如果之前的运行被强制终止，它会恢复自己保存的代理设置。否则请在
*System Settings › Network › Details › Proxies* 中关闭代理。

## Network：macOS 要求输入密码

更改 Mac 的代理是一项系统设置。EmuLabZip 先以你的身份尝试，只有在
macOS 拒绝时才会请求管理员权限。

## Network：规则不起作用

规则看到的是已解密的请求，因此 HTTPS 需要安装证书，拒绝了该证书的主机会被直通，
无法修改。第一条匹配的已启用规则生效；请把你期望生效的那条移到其他规则之上。主机
模式只匹配主机（`api.example.com`，而不是 `https://api.example.com/`）。

## Network：HTTP/3 没有显示 `h3`

该功能默认关闭（**HTTP/3 → Use HTTP/3 to servers**）。只有当某个主机的响应表明
它支持 HTTP/3 之后，才会尝试它，所以第一个请求总是 TCP。阻止 UDP 的网络会使 QUIC 失败；该主机随后
会在 TCP 上停留 10 分钟。

## Layout：在 iOS 上没有可读取的内容

Simulator.app 必须在该设备上打开，并且运行 EmuLabZip 的程序必须在 System Settings,
Privacy & Security, Accessibility 中获得允许（应用，或运行 `emulabzip layout` 时的终端）。如果设备显示
黑屏，请唤醒它。

## Flutter：“Could not find where the Dart VM service listens”

只有 debug 和 profile 构建才有，并且应用只在启动时向日志写一次它的地址。请重新启动应用，
或用 `--uri` 提供 `flutter run` 输出中的地址。发布构建没有 VM service。

## Flutter：树里没有文件名

应用不是用 `--track-widget-creation` 构建的。`flutter run` 会添加它；对于 `flutter build`，请添加
`--extra-front-end-options=--track-widget-creation`。

## React Native：“Metro is not running” 或 “does not list a runtime”

启动 Metro（`npx react-native start`），并在能连到它的设备上打开应用（Android 上用 `adb reverse tcp:8081 tcp:8081`）。
只有当应用连接到 Metro 时，Metro 才会列出运行时，并且一次只允许一个调试器接入：请关闭 React
Native DevTools 和 Chrome 的 inspector，然后重试。发布构建没有 Metro，也没有 React DevTools hook。

## State：“RN state is empty” 或 “no DevTools hook”

Redux、React Query 和 context 是通过 React 的 DevTools hook 找到的，只有开发构建才有它。在发布
构建中，请在 `globalThis.__EMULABZIP_STORES__` 中注册 store（见 [React Native 状态](react-native-state.md)）。未使用 react-redux `Provider`
的 store 也必须注册。

## State：在 Android 上恢复或保存失败

应用必须是可调试的（与 [Storage](storage.md) 相同）。`run-as` 和 `tar` 在设备上运行：`run-as` 被锁定的模拟器镜像
无法保存状态。提示 tar 被拒绝的恢复，是发现了应用文件夹之外的条目；该文件不是来自 EmuLabZip，或者已损坏。

## Edit：“the table cannot be changed”

行是通过 `rowid` 或其主键找到的。视图，或者两者都没有的表，无法区分它的各行：
请改用应用中的 SQL 来更改。读回内容不一致的编辑会放回旧文件，并说明这一点。

## Devices：模拟器无法启动

`emulabzip avd boot` 需要 SDK 的 `emulator`（通过 `ANDROID_HOME`、`ANDROID_SDK_ROOT` 和常用文件夹找到）。
请自己运行 `emulator -avd <name>`，查看它自己的消息。运行中的模拟器绝不会被清除或删除：请先
将其关闭。

## Audit：“the area could not be read”

日志要求应用**正在运行**，这样才能把它的行与系统的行区分开：启动并使用它，然后再次审计。
Android 上的 Storage 需要可调试的应用，与 [Storage](storage.md) 相同。请求来自代理（在
Network 视图中启动它，或传入 `--har`）。

## Language：“Android changes the language of one app at a time”

Android 无法通过 shell 更改设备的语言，只能更改某个应用的语言（`cmd locale set-app-locales`，Android 13
及更高版本）。请传入 `--app`，或在工具栏中选择应用。iOS 模拟器会为每个应用更改语言，每个应用在
重新启动时生效。

## 文本检查：报告了一个并没有被截断的标签

`text-cut-off` 是一种启发式检查：单行文本，其框被限制在其容器（或同一行中的下一个视图）
留出的空间内，并且文本一直延伸到框的末尾。请在 130% 和 200% 下检查；如果放得下，就忽略它。

## Visual：“the screenshots are not the same size”

它们来自不同的设备、方向或显示尺寸。请在同类条件下比较：相同的设备、方向和
显示尺寸（`control reset-display` 可让 Android 设备恢复）。

## Network：Gradle 构建无法下载其发行版

这不是 EmuLabZip 的问题，但在使用演示应用时会遇到：有些网络让 `curl` 能访问 Gradle 服务器，
而 Java 却超时。请用 `curl` 下载发行版，并放到 Gradle 期望的位置
（`~/.gradle/wrapper/dists/<name>/<hash>/`），或者使用你已有的 Gradle 构建。

## Layout：Android 提示 uiautomator 没有产生任何内容

有些窗口禁止它：安全屏幕和少数播放器。请切换到另一个屏幕再试。如果设备很忙
（开机动画、ANR 对话框），请稍等片刻。

## Sessions：录制留下了一个 `.partial` 文件夹

录制在能够自行打包之前就被终止了。什么都没有丢失：`emulabzip session show run.ezsession.partial`
可以打开它。在你再次录制到同一名称之前，请删除或移走该文件夹。

## Timeline：缺少触摸

触摸是通过 `getevent` 从内核的输入设备读取的，因此模拟器窗口上的手指，或通过
模拟器控制台发送的事件都能被看到。`adb shell input tap` 是在更高层，也就是 Android 内部注入的，因此看不到。iOS 不会
把模拟器的触摸交给它之外的任何东西，所以那里没有触摸。请使用 `--taps` 录制。

## Timeline：日志时间是 “estimated”

录制是在 EmuLabZip 0.10 之前完成的，或者设备无法给出它的时间，因此日志的本地时间
是通过比较第一行与录制开始时间推算出来的。它们可能相差两个时钟之间的差值。
请重新录制以获得准确的时间。

## Video：“screenrecord made no video”

有些屏幕无法录制（安全窗口），还有少数镜像根本不支持 `screenrecord`。消息中
带有该工具打印的内容。在 iOS 模拟器上，`recordVideo` 需要模拟器已启动；如果它在
20 秒内没有开始，消息会说明原因。

## Video：报告里有一个单独的 `.mp4`

最大 20 MB 的片段是 HTML 文件的一部分。更大的片段会写在它旁边，名为 `emulabzip-report.mp4`，文件
以该名称引用它：请把两者一起移动或附加。

## Replay：“not in the recording”

应用请求了录制中没有的内容：另一个地址、另一个查询、一个未使用过的方法，或
录制时失败的请求。代理会回复 504，并把它列在 **Not recorded** 下（或列在
`emulabzip net --replay` 的末尾）。请连同该请求重新录制，或添加 `--passthrough` 让它到达服务器。匹配
会忽略查询参数的顺序和端口，但不会忽略值：为每个请求添加时间戳或随机 id 的应用
总是会匹配不上。

## Replay：WebSocket 和某些连接失败

回放录制期间，无法从 HAR 文件作答的内容会被拒绝，因此无法访问互联网：
WebSocket、gRPC 调用、不接受 EmuLabZip 证书的应用，以及代理无法读取的非 TLS 连接。
如果这些必须工作，请使用 `--passthrough`。

## Traces：“Perfetto did not record a trace”

Perfetto 需要 Android 9 或更高版本。如果消息提到某项权限，请重启模拟器，或运行
`adb shell setprop persist.traced.enable 1` 后重新录制。带有语法错误的自定义配置会由 Perfetto 自己报告；
用 `adb shell perfetto --txt -c - -o /data/misc/perfetto-traces/test <config.txt` 检查它。

## Traces：CPU profile 失败

simpleperf 只对*可调试*或*可分析*的应用采样，并且应用必须正在运行。对于整个系统，
你需要允许 `adb root` 的模拟器镜像（不带 Google Play）。“No samples were recorded” 表示应用在录制期间
什么也没做：请使用它，然后重新录制。

## Traces：Instruments 提示 Developer Mode 已关闭

在终端中运行 `sudo DevToolsSecurity -enable`，输入你的密码，然后重新录制。`DevToolsSecurity -status`
显示当前状态。

## Traces：trace processor 下载失败

它来自 `github.com/google/perfetto/releases`。在代理后面时，请设置 `HTTPS_PROXY`。与固定的 SHA-256 不匹配的文件
会被丢弃，绝不会运行。你仍然可以在 Perfetto UI 中打开任何 Perfetto 追踪。

## Crashes：什么也没有列出

在 Android 上，系统会把崩溃保存在 DropBox 中并对重复崩溃限速，因此循环崩溃的应用被列出的次数少于它
实际崩溃的次数。在 iOS 模拟器上，macOS 会在崩溃几秒后把 `.ips` 报告写入
`~/Library/Logs/DiagnosticReports`；不在任何模拟器文件夹中的应用的报告不会被列出。点击
**Refresh**，或运行 `emulabzip crashes`。

## Crashes：名称仍然是乱的

mapping 只会用于它所对应的应用版本，崩溃会说明是哪个版本（“Add the
mapping.txt of com.example 1.2.0 (42)”）。请用完全一致的版本添加文件：`emulabzip mappings add com.example
"1.2.0 (42)" mapping.txt`。另一个构建的文件会给出错误的名称，因此不会使用。当
R8 没有保留行号时，帧也会保持原样：请在下一个构建的规则中添加 `-keepattributes SourceFile,LineNumberTable`。

## Crashes：React Native 显示 `1:608617` 而不是文件

发布构建运行的是 bundle，其位置需要该 bundle 的 source map。对于 Hermes，请使用合成的 map，
`android/app/build/generated/sourcemaps/react/release/index.android.bundle.map`，而不是 `packager` 或 `compiler` 的 map。
它还必须是崩溃的那个构建的 map。

## Crashes：iOS 显示裸地址

添加崩溃那个构建的 `.dSYM`（`emulabzip mappings add com.example.app "1.2.0 (42)" MyApp.app.dSYM`）。它的
UUID 必须与应用的一致，因此另一个构建的 dSYM，即使版本相同，也不会被使用。读取地址需要
Mac，因为它使用 `atos`。

## Bug report：没有失败的请求

请求来自 Network 中的代理。请在缺陷发生之前启动它并连接设备；用
`--network` 录制的[会话](sessions.md)所生成的报告也包含它们。

## Controls：位置在设备上不起作用

只有模拟器才接受新的位置，方式是通过模拟器控制台。它需要
`~/.emulator_console_auth_token` 中的令牌，该文件由模拟器在你的账户下首次运行时创建。Android 上的真机
和其他类型的模拟器不支持设置位置。

## Controls：Android 上通知没有出现

通知是通过 shell 用 `cmd notification post` 发出的（在 Android 14 上试过）。如果设备隐藏了通知，请为 “Android Shell” 开启
通知。在 Android 上会忽略应用参数。

## Web pages：我的 WebView 没有列出

只有当 WebView 所在的应用调用了 `WebView.setWebContentsDebuggingEnabled(true)`（debug 构建通常会）并且
页面已打开时，才会列出该 WebView。Chrome 在打开时会被列出。列表在你打开 **Frameworks** 时以及点击
**Refresh** 时读取。在 iOS 上请使用 Safari 的 Develop 菜单；见[应用中的网页](webviews.md)。

## Flows：我的触摸没有被录制

只会读取触摸屏：请触摸模拟器自己的窗口。从 EmuLabZip 发出的点击（**Interact**、**Play**）或通过
`adb shell input` 发出的点击都不会到达它，输入的文字也不会被录制。只有当屏幕在触摸之前
两三秒被及时读取时，点击才会得到它所点中视图的名称；无论如何，点击的位置都会保留。见
[输入与流程](automation.md)。

## Deep links：检查提示域名未验证

在 Android 上，用 **Verify again** 或 `emulabzip link verify <app>` 让设备重试，几秒后再检查。指纹不匹配通常
意味着文件中列出的是发布密钥，而设备上的构建是用
debug 密钥签名的。重定向的域名文件（到 `www`，或从 http 到 https）会被两个系统拒绝。见
[深层链接](deep-links.md)。

## Network：脚本规则不起作用

脚本只有在其确切字节被批准之后才会运行（规则中的 **Approve this script**，或 `emulabzip script approve`）。
修改文件后需要再次批准。如果脚本运行了但请求没有变化，流程的详情会说明
脚本写了什么，或为什么失败。见[规则脚本](rule-scripts.md)。

## Plugins：插件无法运行

必须先启用它，编辑其 `plugin.json` 会再次将其禁用。以错误退出的插件会显示其错误输出的
末尾。请检查 `command` 中的程序是否存在（先在插件的文件夹中查找，再在
PATH 中查找），以及它是否可执行（`chmod +x run.sh`）。

## MCP：助手看不到任何工具

手动运行 `emulabzip mcp`，并向它发送 `initialize` 请求，如 [MCP 服务器](mcp.md#trying-it)中所示。如果客户端
找不到 `emulabzip`，请给它完整路径。标准输出只承载协议；请查看标准错误
（`EMULABZIP_LOG=debug`）了解出了什么问题。

## 获取诊断信息

使用 `EMULABZIP_LOG=debug` 启动应用或 CLI，并在[提交 issue](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=bug_report.yml)时附上输出。
