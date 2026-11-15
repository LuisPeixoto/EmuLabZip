---
source: 0c82c94d90e4
title: Network 工具：调试代理与 HTTPS 解密
description: Network 工具是内置的调试代理，用只存在于你电脑上的证书颁发机构解密 HTTPS，列出每个请求的标头、正文和耗时，无需修改你的应用。
---
# 网络

Network 工具是 EmuLabZip 内置的调试代理。它位于你的应用和互联网之间，用只存在于你电脑上的证书颁发机构解密 HTTPS，并列出每个请求的标头、正文和耗时。你不需要修改你的应用。

<p align="center">
  <img src="images/network.png" alt="Network 工具列出请求，并打开了一个 JSON 响应" />
</p>

## 快速开始

1. 打开 **Network**（<kbd>⌘6</kbd>）并选择你的设备。
2. 每台设备点击一次 **Certificate**，让它信任 EmuLabZip 的证书（见[下文](#the-certificate)）。
3. 点击 **Connect**。如果代理没有运行，它会启动，设备会把它的 HTTP 和 HTTPS 流量发送给它。
4. 使用你的应用。请求会在完成时出现。点击其中一个，可查看它的标头、正文和耗时。
5. 完成后点击 **Disconnect**（或直接退出）。EmuLabZip 会把设备恢复成它原来的样子。

在终端中：

```bash
emulabzip ca install                 # once per device
emulabzip net --har capture.har      # Ctrl+C to stop; the device is restored
```

## 设备如何连接到代理

代理只监听你电脑的 `127.0.0.1`，绝不监听网络。

| | Android | iOS 模拟器 |
|---|---------|---------------|
| 路由 | `adb reverse tcp:<port> tcp:<port>` 让设备上的 `127.0.0.1:<port>` 连到你的电脑，然后 `settings put global http_proxy 127.0.0.1:<port>` | 模拟器共享 Mac 的网络，因此 **Mac 自己的** HTTP 和 HTTPS 代理会指向 EmuLabZip |
| 范围 | 你所连接的那一台设备 | 开启期间，**Mac 上的每个应用** |
| 撤销方式 | 断开连接、退出，或 `emulabzip net` 停止。只会移除 EmuLabZip 创建的转发；Metro 的 `tcp:8081` 会保留 | 断开连接、退出，或在 EmuLabZip 被强制结束后的下一次启动 |
| 适用于 | 模拟器、USB 设备、无线调试 | macOS 上的模拟器 |

对于 Mac，EmuLabZip 会在更改之前保存每个网络服务的代理设置，并原样写回，包括已关闭但仍保留地址的代理。如果 macOS 第一次要求输入管理员密码，那是 `networksetup` 在更改系统设置。

## 证书

为了读取 HTTPS，EmuLabZip 会用由它自己的证书颁发机构签发的、针对该网站的证书来应答。设备必须信任这个颁发机构。它在首次使用时创建，名为 *EmuLabZip CA*，保存在 `~/Library/Application Support/EmuLabZip`（Windows 上为 `%APPDATA%\EmuLabZip`，Linux 上为 `~/.local/share/EmuLabZip`），私钥仅你自己可读。设置 `EMULABZIP_HOME` 可使用其他文件夹。

| 设备 | **Certificate** 的作用 | 哪些应用信任它 |
|--------|---------------------------|---------------------|
| iOS 模拟器 | `simctl keychain add-root-cert` | 所有应用。请重启已经在运行的应用 |
| 镜像允许 `adb root` 的 Android 模拟器（Google APIs、AOSP） | 把它添加到**系统**存储 | **每个应用**，直到设备重启 |
| 来自 Google Play 的 Android 镜像，或真机 | 把 `emulabzip-ca.crt` 复制到 Downloads 并打开 Settings，让你把它安装为**用户**证书 | Chrome 和主动选择信任的应用。你自己的调试构建可以用下面的代码片段选择信任 |

在可获取 root 的镜像上，`adbd` 会像执行 `adb root` 之后那样保持以 root 运行。再次点击 **Certificate** 是安全的：它会替换文件，而不是再叠加一份副本。

### 让你的调试构建信任它

从 Android 7 起，应用会忽略用户证书，除非它们主动要求使用。请仅把下面的内容添加到你的**调试**构建，并在该构建的清单中引用它（`android:networkSecurityConfig="@xml/network_security_config"`）：

```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <debug-overrides>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </debug-overrides>
</network-security-config>
```

发布构建不受影响，因为 `debug-overrides` 只适用于可调试的应用。

### 无需安装任何东西

代理运行时，设备上的浏览器可以直接从代理获取证书：`http://127.0.0.1:9090/ca.crt`（DER 格式，适用于 Android 和 iOS）或 `/ca.pem`。

## 你看到的内容

每一行是一个请求。列包括状态、方法、主机、路径、类型、大小和时间。可以按文本（URL、方法或状态）、状态类别或方法过滤。使用 <kbd>↑</kbd> <kbd>↓</kbd> 在列表中移动。

详情窗格有四个标签页：

| 标签页 | 显示 |
|-----|-------|
| Headers | 请求和响应的标头 |
| Request | 发送的正文 |
| Response | 返回的正文 |
| Timing | 协议、到第一个字节的时间和到结束的时间 |

正文会被解码以便阅读（gzip、deflate、Brotli、zstd）：你会看到线路上的大小和解码后的大小。JSON 会被格式化，图片会被显示，二进制数据以十六进制转储显示。代理会原样转发字节；只有你读取的那份副本会被解码。

**Connections** 是代理无法解码的内容：非 HTTP 流量，以及拒绝该证书的应用。它们会被列出，让你知道它们的存在，并附上原因。WebSocket 会被列出，并带有一个 **Messages** 标签页，显示每条消息（见 [WebSocket 消息](websockets.md)）。

**Proto** 列显示代理与服务器使用的协议：`h1`、`h2` 或 `h3`。

## 不止是观察

- **Rules**：自己应答请求、让它变慢、阻止它，或暂停它并编辑。请参阅[模拟响应与断点](mocks.md)。
- **Import**：把 `.pcap` 或 `.pcapng` 抓包文件作为请求打开。请参阅[导入抓包文件](pcap.md)。
- **Send again**：**Resend** 会把请求原样再发送一次；**Edit and resend** 允许你先修改方法、地址、标头和正文。这份副本是列表中标有 `RESEND` 的新一行。规则和断点不适用于它，因为是你选择发送它的。被模拟的请求、WebSocket 以及从抓包文件读取的请求都不能再次发送，正文太大而未能保存的请求也不能。
- **GraphQL 和 gRPC**：列表中会标出操作的名称，正文会按其协议的含义来读取。请参阅 [GraphQL](graphql.md) 和 [gRPC 与 protobuf](grpc.md)。
- **Network conditions**：缓慢、高延迟、丢包或断开的连接。请参阅[网络条件](network-conditions.md)。
- **Replay**：用 HAR 文件或录制的会话来应答，让应用在没有互联网的情况下运行。请参阅[回放录制](replay.md)。
- **HTTP/3**：见下文。

![用另一个变量再次发送 GraphQL 请求的编辑器](images/network-resend.png)

### 固定证书的应用

有些应用（许多 Google 服务、银行应用）只接受它们自己服务器的证书。EmuLabZip 无法在不破坏它们的情况下解密它们，因此在某个主机拒绝证书一次之后，之后与它的连接会被原样**放行**。应用继续正常工作，列表会显示一个带有原因的失败连接，并有一个横幅列出这些主机。**Decrypt again** 会重试它们，例如在你安装了证书之后。

## 导出

下载按钮会把请求保存为 [HAR](http://www.softwareishard.com/blog/har-12-spec/) 文件，Chrome DevTools、Firefox 和大多数 API 工具都能读取。正文以解码后的形式保存。Connections 不包含在内。在命令行中：`emulabzip net --har capture.har`。

## HTTP/3

位于 HTTP 代理之后的设备绝不会向它使用 QUIC，因此 HTTP/3 只涉及代理的第二段，即到服务器的那一段。服务器通过 `Alt-Svc: h3=":443"` 标头宣告 HTTP/3。代理会按主机记住它，开启 **HTTP/3 → Use HTTP/3 to servers**（默认关闭）后，之后对该主机的请求会通过 QUIC 发送。如果 QUIC 失败（网络阻止 UDP、服务器没有响应），请求会回退到 HTTP/2 或 1.1，并且该主机会在 10 分钟内保持使用 TCP。当它成功时，流程会在 Proto 列显示 `h3`。HTTP/3 响应会先被完整读取再转发，因此很大的下载不会以流的方式传输。

**Hide HTTP/3 from apps**（默认开启）会从应用收到的 `Alt-Svc` 标头中移除 `h3` 条目。没有它，应用可能得知服务器支持 HTTP/3，并绕过代理直接尝试 QUIC，它的流量就会从列表中消失。流程仍然会记录原始标头。在命令行中：`emulabzip net --http3`，以及用 `--show-h3` 向应用显示这些宣告。

## 限制

- 每个正文只存储前 **10 MB**。其余部分仍会到达目的地。
- 列表保留最新的 **10,000** 个请求。
- 直接使用 QUIC、绕过代理的应用无法被解码。在默认设置下，应用不会被告知 HTTP/3，因此它们会通过代理保持使用 HTTP/2。
- 代理用于**你自己的**应用和设备。不要把它指向你无权检查的流量。

## 命令行

```bash
emulabzip net                       # the only running device; prints one line per request
emulabzip net --json | jq 'select(.status >= 400)'
emulabzip net --no-connect --port 9191   # only run the proxy
emulabzip net --rules rules.json    # mock, delay or block requests
emulabzip net --http3               # talk HTTP/3 to servers that offer it
emulabzip net --replay run.ezsession   # answer from a recording; --passthrough lets the rest through
emulabzip net import capture.pcap   # read a packet capture
emulabzip ca install                # make the device trust the certificate
emulabzip ca path                   # where the certificate file is
```

所有选项请参阅[命令行](cli.md)。
