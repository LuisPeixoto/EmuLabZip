---
source: a4b2edad11c5
description: 介绍安全与隐私审计：查找应用遗留在日志、偏好设置、数据库和请求中的密钥与个人数据，以及清单配置问题，并说明检查规则、请求来源、CI 用法和局限。
---
# 安全与隐私审计

<p align="center">
  <img src="images/audit-security.png" alt="Audit 视图显示某个应用的发现：日志中的 bearer 令牌、偏好设置中的会话令牌、一个已导出的 receiver" />
</p>

`emulabzip audit security <app>` 和 **Audit** 视图会查找应用不该随意留下的内容：日志、偏好设置和数据库中的密钥与个人数据，请求中的明文 HTTP 与密钥，`debuggable` 和 `allowBackup` 之类的标志，其他应用可以访问的组件，App Transport Security 例外，以及 iOS 隐私清单中的缺口。每个发现都会说明它是什么、在哪里，以及如何修复。

```bash
emulabzip audit security com.example.shop
emulabzip audit security com.example.shop --har run.har       # also look at recorded requests
emulabzip audit security com.example.shop --only manifest,storage --json
emulabzip audit security com.example.shop --fail-on error     # exit 1 when something at least this serious is found
```

**密钥绝不会被原样重复。**证据会被缩减为前两个字符、后两个字符和长度（`ey…Xk (131 chars)`），在终端、JSON、报告和 MCP 中都是如此。

## 检查范围

| 范围 | 规则 |
|------|-------|
| **Manifest**（Android，来自 `dumpsys package`） | `debuggable`、`allow-backup`、`old-target-sdk`（低于 API 34）、`exposed-component`（带 intent filter 的 receiver、service 或 activity，启动器除外）、`content-provider`（仅列出；`dumpsys` 不会说明它是否已导出） |
| **Manifest**（iOS，`Info.plist`） | `ats-arbitrary-loads`、`ats-arbitrary-loads-scoped`（Web 内容、媒体）、`ats-insecure-domain`（某个域名允许明文 HTTP）、`ats-old-tls` |
| **Privacy**（iOS） | `no-privacy-manifest`、`tracking-not-declared`、`undeclared-tracking-domain`：将应用被观察到使用的广告和分析域名与 `PrivacyInfo.xcprivacy` 对照 |
| **Traffic**（已录制的请求） | `cleartext-traffic`（每个主机一次；`localhost`、`10.0.2.2` 和 `*.local` 不会报告）、`secret-in-url`（查询字符串中的令牌或密钥），以及仅在明文 HTTP 下的 `<kind>-in-cleartext-body` |
| **Log** | `<kind>-in-log` |
| **Storage** | `<kind>-in-storage`（SharedPreferences、DataStore、MMKV、属性列表）、`secret-in-storage`（名称像密钥且有值的键）、`sensitive-column`（名为 `password`、`token`、`api_key` 等的 SQLite 列） |

`<kind>` 可以是 `jwt`、`bearer-token`、`api-key`（Google、AWS、Stripe、GitHub 和 Slack 密钥的格式）、`secret`、`email` 或 `card-number`（通过 Luhn 校验的数字，因此订单号不会被当成银行卡号）。`password=****` 和 `${TOKEN}` 之类的占位符会被忽略，名称听起来像密钥的日期和二进制数据也会被忽略。

## 请求来自哪里

审计从代理读取请求。在桌面应用中，请先在 **Network** 视图中启动代理：审计随后会包含代理已录制的内容。在命令行中，请传入 HAR 文件（`--har`，来自 `emulabzip net --har`）。只有请求通过明文 HTTP 发送时，才会判断其正文，因为在 HTTPS 下，应用向服务器发送什么是应用自己的事。

## 日志

设备日志由所有应用共享。EmuLabZip 读取应用自身进程的日志行（以及提到该应用的行），因此应用必须正在运行。对于未运行的应用，该范围会被报告为 **not read**，而不是扫描系统的日志行，因为那会读到别人的密钥，以及只是看起来像银行卡号的数字。

## 在 `ci` 中

```bash
emulabzip ci -o run.ezsession --app com.example.shop --network --fail-on-security warning -- ./gradlew connectedCheck
```

命令结束后会运行审计（使用代理录制的请求），当有发现达到该严重程度时，运行会失败。发现会写入 Markdown 摘要。参见 [CI 模式](ci.md)。

## 限制

- `dumpsys package` 不会打印 `exported` 属性。带 intent filter 的组件可被其他应用访问，报告的就是这一点；没有 filter 但已导出的组件不会被发现。
- 只有**可调试**应用才能读取 Android 的存储和应用的文件，与 [Storage](storage.md) 相同。
- 这些模式查找的是看起来像密钥的内容。简短或不常见的密钥会被漏掉，看起来像密钥的值会被报告：采取行动之前请先阅读发现。
- iOS：只有在应用运行时才会读取日志；隐私声明会与已录制请求中看到的域名对比，因此请先录制一些请求。
- 这是初步检查，不是渗透测试。它不会查看应用的代码或其服务器。

## 面向 AI 助手

通过 [MCP](mcp.md)，`audit_security` 会返回相同的发现。它只读取，不会更改设备上的任何内容。
