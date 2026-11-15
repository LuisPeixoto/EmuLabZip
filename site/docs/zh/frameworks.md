---
source: dcdd776b1e52
description: 介绍 EmuLabZip 如何识别应用使用的框架，并借助 Flutter、React Native、Jetpack Compose 和 SwiftUI 各自的工具查看其内部。
---
# 框架内部

Flutter 和 React Native 自己绘制屏幕，Jetpack Compose 和 SwiftUI 绘制屏幕的方式也不同于
平台的视图。常规的视图树对它们说明很少，因此 EmuLabZip 会找出应用使用的是哪个框架，并
借助该框架自己的工具查看内部。

```bash
emulabzip frameworks com.example.app
```

它读取应用的文件，无论应用是否在运行：

| 框架 | 判断依据 |
|-----------|--------------------|
| Flutter | APK 中的 `libflutter.so`，或应用包中的 `Flutter.framework` |
| React Native | `libreactnative.so`、`libreactnativejni.so` 或 `index.android.bundle`；iOS 上为 `main.jsbundle` 或 `React.framework` |
| Hermes | `libhermes.so` 或 `libhermesvm.so`；`hermes.framework` |
| Jetpack Compose | 库写入 APK 的 `META-INF/androidx.compose.*.version` 文件（版本从中读取） |
| SwiftUI | 可执行文件所链接的库中的 `SwiftUI.framework` |

在桌面应用中，**Frameworks** 显示同样的内容，其下是 Flutter 和 React Native 的面板。

## Flutter

![Frameworks 视图中 Flutter 应用的 widget 树，附有应用创建的每个 widget 的文件和行](images/frameworks-flutter.png)

Debug 和 profile 构建有一个 **Dart VM service**，应用会在日志中公布它：
`The Dart VM service is listening on http://127.0.0.1:41233/token=/`。EmuLabZip 找到这一行，通过设备
访问该端口（Android 上用 `adb`，模拟器上用 Mac 自己的网络，不会转发任何东西，也不会留下任何东西），
并通过 WebSocket 使用 JSON-RPC 通信。

```bash
emulabzip flutter tree com.example.app
emulabzip flutter frames com.example.app --seconds 10     # use the app meanwhile
emulabzip flutter memory com.example.app
```

- **Widgets**：Flutter inspector 显示的树。`●` 标记由你自己的代码创建的 widget，并附有文件和行；
  `○` 标记框架和软件包的 widget。要有文件名，应用必须知道它的 widget 是在哪里创建的。用
  `flutter run` 启动的应用知道。对于 `flutter build`，请添加 `--extra-front-end-options=--track-widget-creation`
  （`tools/flutter-app/install.sh` 会添加）。没有它，树里就没有文件名，并且会说明这一点。
- **Frames**：应用绘制的每一帧的构建时间（UI 线程）和光栅化时间（绘制线程），
  并统计卡顿帧：任一时间超过 16.7 ms 的帧就是卡顿帧。**应用空闲时不会绘制任何内容**，因此
  帧数取决于你在录制期间的操作。没有显卡的模拟器用软件光栅化，因此它的光栅化时间
  对手机来说参考价值不大。
- **Memory**：正在使用的 Dart 堆及其大小、由 Dart 对象保持存活的堆外内存（图片、
  缓冲区），以及占用最多的类。

如果日志中已经没有该地址（繁忙应用的日志会滚动），请重新启动应用，或者传入它：
`--uri http://127.0.0.1:41233/token=/`。发布构建没有 VM service。

## React Native

![Frameworks 视图中 React Native 应用的 React 组件树，含 props](images/frameworks-react-native.png)

开发中的 React Native 应用会与开发服务器 **Metro** 通信，Metro 列出每个已连接应用的 JavaScript 运行时，
并代理其 Chrome DevTools 协议。EmuLabZip 连接到你电脑上的 Metro（端口 8081，或
`--metro <port>`），选出属于该应用的运行时，并读取：

```bash
emulabzip rn tree com.example.app           # the React components, with props
emulabzip rn heap com.example.app           # Hermes' heap in use
emulabzip rn console com.example.app --seconds 20
```

- **Components**：React 树，从开发构建安装的 React DevTools hook 读取。`●` 标记
  组件，`○` 标记它们变成的原生视图。Props 会被缩短：`title="Hello"`、`onPress=ƒ`。
- **Hermes heap** 以及 Hermes 的版本。
- **Console**：应用在你监听期间通过 `console.log` 等方法写出的内容。

**State** 标签页（以及 `emulabzip rn state`）读取 Redux、React Query、React context 和你自己注册的 store：
见 [React Native 状态](react-native-state.md)。

**Metro 一次只允许一个调试器连接到应用。** 从 EmuLabZip 读取可能会关闭 React Native DevTools 或
Chrome 的 inspector，反过来也一样。发布构建没有 Metro，也没有 hook。

在 Android 上，应用通过 `adb reverse tcp:8081 tcp:8081` 连接 Metro，EmuLabZip 不会去动它。

## Jetpack Compose 和 SwiftUI

![Layout 中的 Jetpack Compose 屏幕，其视图被标记为 Compose，检查器标明了框架](images/layout-compose.png)

在 **Layout** 中，由框架绘制的视图会被标记，检查器会显示是哪个框架。`ComposeView` 下的所有内容
都是 Compose；当应用开启 `testTagsAsResourceId` 时，测试标签（`Modifier.testTag`）会显示为 id。在
`FlutterView` 或 React Native 根视图下，标记写作 `flutter` 和 `react-native`。

在 iOS 上，无障碍树中没有说明视图由什么绘制的类，因此使用 `layout --app <bundle id>`（以及在桌面应用中
选择应用）时，整个屏幕会标记为该应用的框架：按 Flutter、React Native、
Compose、SwiftUI 的优先顺序。SwiftUI 自己的视图类型（`VStack`、`Button`）无法通过
无障碍获得：你看到的是无障碍角色和你设置的标识符。

**无法获得重组次数。** Android Studio 的 Layout Inspector 从加载到被调试应用中的代理获取它们，
而代理使用的是未公开的接口。EmuLabZip 只使用公开接口，因此不会显示
它们。

## 试一试

`tools/` 中为每种框架都有一个演示应用：`compose-app`、`flutter-app`、`rn-app` 和 `swiftui-app`，每个都带有一份 README。
