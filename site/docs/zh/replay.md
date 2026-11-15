---
source: 77184675abce
description: 说明如何用录制内容回放应用的请求，让服务器完全不被访问：匹配规则、不支持的内容、放行选项、CI 用法、应用内操作，以及分享录制前的注意事项。
title: 回放录制：用录制内容应答应用的网络请求
---
# 回放录制

<p align="center">
  <img src="images/replay.png" alt="回放录制后的 Network：三个请求已应答，一个不在录制中" />
</p>

**回放**会用录制内容来应答应用的请求，因此服务器完全不会被访问。应用在没有互联网的情况下，面对服务器曾经说过的内容运行。它适用于后端此后已发生变化的缺陷，以及每次都必须得到相同结果的 CI 运行。

```bash
# Record once, with the network.
emulabzip session record -o run.ezsession --app com.example.app --network

# Run the app against it. Press Ctrl+C to stop.
emulabzip net --replay run.ezsession
```

带网络数据的[会话](sessions.md)，或任何 HAR 文件（浏览器的导出文件，或 `emulabzip net --har`），都可以作为录制内容。

## 它做什么

代理会先检查录制内容，再去询问服务器。录制中存在的请求会得到录制的应答：状态、请求头和正文，与当时完全一致。**不在**录制中的请求会以 `504` 失败，并提示 *not in recording*，在请求列表中标为失败。它绝不会被发往服务器。这正是重点：依赖某个没人告诉你的地址的测试会失败，而不是悄悄使用网络。

- 请求按方法、主机、路径和查询参数匹配。查询参数的顺序无关紧要，端口也无关紧要。
- **录制了多次**的请求，会按录制顺序依次得到应答，之后一直重复最后一个。一个先后返回 1、2 的计数器，会依次返回 1、2、2。
- 当发往同一地址的请求正文不同时（例如 GraphQL 查询），优先选用正文相同的那一个。
- HTTPS 可以工作，因为代理自己终结 TLS（设备必须信任该[证书](network.md#the-certificate)，与任何抓包一样）。
- [Mocks](mocks.md) 中的规则仍然优先于录制内容，因为那是你有意编写的。

## 它不做什么

回放录制时，HAR 文件无法应答的内容会被拒绝，而不是放行到互联网：

- **WebSocket**。它们的消息在文件中，但不会回放。
- **gRPC 调用**。HAR 文件没有 trailer，因此调用无法完成。
- **代理无法解码的连接**：其他协议，以及不接受 EmuLabZip 证书的应用。
- 从 Network 视图**重新发送请求**。

录制时失败的请求不在录制内容中。

## 放行部分请求

```bash
emulabzip net --replay run.ezsession --passthrough
```

使用 `--passthrough` 时，不在录制中的请求会发往真实服务器。当你只想固定应用的一部分行为时使用它。`--timing` 会让每个应答等待与服务器相同的时间，适用于网络变慢时行为会不同的应用。

## 在 CI 中

```bash
emulabzip ci -o run.ezsession --network --replay baseline.ezsession --fail-on-miss -- ./gradlew connectedCheck
```

运行照常自行录制，其请求由 `baseline.ezsession` 应答。当应用请求了不在录制中的内容时，`--fail-on-miss` 会使运行失败，并在运行结束时列出这些内容。`session record` 也支持 `--replay`。两者都需要 `--network`。

## 在应用中

在 **Network** 中，**Replay → Choose a file…** 会加载 HAR 文件或会话。一个栏会显示正在回放的内容、已应答的请求数和未应答的请求数（具体是哪些，见 **Not recorded**）。在 **Sessions** 中，**Replay network** 会加载已打开的会话。像往常一样启动代理并连接设备；**Stop replay** 会回到真实服务器。

## 须知

- 录制内容会最先被读取，因此文件错误或会话没有网络数据时，会立即给出说明。
- 应答使用的是录制的正文，已解码。`Content-Encoding`、`Content-Length` 和 `Alt-Svc` 不会重复，因此应用不会被告知使用正文已不具备的压缩方式，也不会被告知绕过代理去尝试 HTTP/3。
- 录制内容包含服务器发送的一切，包括令牌和个人数据。分享之前请先检查。
