---
source: a0fef516194b
description: 介绍 Analytics 检查器：在单独的列表中实时查看应用发往 Segment、Amplitude、Mixpanel 和 Firebase Analytics 的事件，了解解码范围、桌面端用法和限制。
---
# Analytics 检查器

<p align="center">
  <img src="images/analytics.png" alt="Analytics 视图列出应用发往 Segment 和 Mixpanel 的事件，其中一个已展开" />
</p>

在单独的列表中，实时查看应用发往 **Segment**、**Amplitude**、**Mixpanel** 和 **Firebase Analytics** 的事件，并且是已解码的。

```bash
emulabzip analytics                          # runs the proxy, points the device at it, prints each event
emulabzip analytics --service segment,mixpanel --json
emulabzip analytics --firebase-log           # Firebase's debug-mode lines from the device log
```

Analytics 调用本来就是普通请求，因此它们仍会出现在 Network 列表中。检查器读取它们的主机、地址、请求头和正文，所以它适用于代理录制的任何内容，也适用于之后打开的会话或 HAR。

## 解码内容

| 服务 | 主机 | 内容 |
|---------|------|------|
| Segment | `*.segment.io`, `*.segment.com` | `/v1/track`、`/v1/identify`、`/v1/page`、`/v1/screen`、`/v1/group`、`/v1/alias` 和 `/v1/batch`：JSON，单次调用或批量调用 |
| Amplitude | `*.amplitude.com` | HTTP API（`/2/httpapi`、`/batch`：带 `events` 的 JSON）和较旧的表单正文（`e=`），包含事件属性和用户属性 |
| Mixpanel | `*.mixpanel.com` | `/track` 和 `/import`（`data=` 为 base64，位于正文或查询中，或为纯 JSON）以及 `/engage` 的用户资料变更 |
| Firebase Analytics | `app-measurement.com` | `/a` 的批量 protobuf：在撤销正文的 `Content-Encoding` 之后，得到事件名称、参数和时间 |

每个事件由服务、类型（`track`、`screen`、`page`、`identify`、`profile`……）、名称、属性、用户（或匿名 ID、设备 ID）以及时间（若有）组成。

Firebase 也可以**不通过代理**读取。为应用开启调试模式后（`adb shell setprop debug.firebase.analytics.app <package>`），Firebase 会把每个事件以标签 `FA` 写入日志，`emulabzip analytics --firebase-log` 会把它们打印出来。

## 在桌面端

**Analytics** 视图会列出代理已录制请求中的事件，最新的在前，可按服务过滤，并可在名称和属性中搜索。打开一个事件即可查看其属性。请先在 **Network** 视图中启动代理。

## 限制

- **固定证书**的 SDK 会被原样放行，无法读取。对它们而言，Firebase 的调试日志仍然可用。
- Firebase 的批量布局是目前所理解的 Google 测量协议的布局，解码器是用按该布局构造的载荷测试的，**而不是用从运行中的 Firebase 应用捕获的流量**。当 Google 更改它时，事件可能会缺失：请在 issue 中告诉我们，如有抓包请一并附上。
- 事件是按请求解码的，不会存储：代理已不再保留的请求中的事件就没有了。
- 通过 [MCP](mcp.md) 没有 analytics 工具：服务器不运行代理。
