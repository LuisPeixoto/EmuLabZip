---
source: c99e4d2c05ff
description: 像 Chrome DevTools 一样读取混合应用中的 WebView：列出页面、查看控制台、运行 JavaScript，并在 Network 视图中查看页面发出的请求。
---
# 应用中的网页

混合应用在 **WebView** 中显示网页。EmuLabZip 读取它们的方式和 Chrome 的 DevTools 一样：列出页面，显示它们的控制台，在其中运行 JavaScript，并在 Network 视图中与代理看到的所有内容一起显示页面发出的请求。在 iOS 上，它会把你带到 Safari 的 Web Inspector。

## 应用需要允许什么

在 Android 上，只有应用开启了网页调试，WebView 才能被检查：

```kotlin
WebView.setWebContentsDebuggingEnabled(true)   // in a debug build
```

设备上的 Chrome 打开时也会被列出。无需安装任何东西，也无需转发端口：EmuLabZip 读取设备上打开的 socket 列表，找到 `webview_devtools_remote_<pid>` 和 `chrome_devtools_remote`，并通过 ADB 与它们通信。

`tools/compose-app` 里有一个用于试用的界面：**Open the web page** 会启动一个 WebView，页面会向控制台写入内容、从网站获取数据、获取一个不存在的资源，并抛出一个错误。

## 在应用中

**Frameworks → Web pages** 会列出每个可检查的页面，以及它所属的应用。选择一个页面后：

- **控制台**实时跟随：日志、警告、错误，以及页面没有捕获的错误，每条都带有来源位置；
- 底部的输入行会在页面中**运行 JavaScript**，例如 `document.title`，并显示返回值；
- **Reload** 会重新加载页面，并忽略其缓存；
- **Show requests in Network** 还会在 [Network](network.md) 视图中列出该页面的请求，并标记为 **WEBVIEW**。修改后请重新选择该页面。

这些请求来自页面本身，因此即使代理无法解密流量，它们也会出现（证书锁定对页面没有影响），而且无需安装证书。它们不能像经过代理的请求那样被修改，也不能在断点处被暂停；如需这样做，请使用代理。

![Web pages 面板，显示演示应用 WebView 的控制台](images/webview-console.png)

![Network 中 WebView 的请求，标记为 WEBVIEW](images/webview-network.png)

## 在 iOS 上

Apple 不允许其他工具读取 `WKWebView`，因此在模拟器上，该面板会给出操作步骤和一个打开 Safari 的按钮：

![iOS 模拟器上的 Web pages 面板](images/webview-ios.png)

1. 在 Safari 中打开 Settings → Advanced，并开启 "Show features for web developers"。
2. 在你的应用中打开该页面。`WKWebView` 必须设置 `isInspectable = true`（iOS 16.4 及更高版本；调试构建通常会设置它）。
3. 在 Safari 的 **Develop** 菜单中，先选择模拟器，再选择该页面。

## 在命令行中

```bash
emulabzip webview list                                   # the sockets and their pages
emulabzip webview console --app com.example.shop         # what the page writes, for 10 seconds (--seconds N)
emulabzip webview eval "document.title"                  # run JavaScript; a script that throws fails the command
emulabzip webview network --reload --seconds 10 --har page.har
```

`--app` 选择应用的 WebView，`--socket` 按名称选择 socket，`--page` 按 id 或地址的一部分选择页面。只有一个选项时，这些参数都可以省略。每个命令都支持 `--json`。

## 通过 AI 助手

`webview_list` 和 `webview_console` 是 [MCP 工具](mcp.md)。MCP 不提供运行 JavaScript 的功能，因为脚本可能会改变应用存储的内容；该功能只在应用和命令行中提供。

## 限制

- 仅支持 Android，且仅限可调试的 WebView 和 Chrome。没有开启调试的发布构建无法被读取。
- 响应正文最多读取 10 MB。如果请求正文时它已从浏览器内存中消失，则只显示没有文本的正文。
- 页面通过它自己的一个连接来读取。另一个调试器（`chrome://inspect`）可以同时打开。
