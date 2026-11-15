---
source: a85b9a63c2f6
description: 了解如何在设备上打开深层链接，并检查域名文件与应用声明是否一致，在 Android 和 iOS 上找出链接打开浏览器而不是应用的原因，也可用于 CI。
---
# 深层链接

只有当两件事一致时，网页链接才会打开应用而不是浏览器：**域名**表示允许该应用（网站上的一个文件），并且**应用**声明了该域名。只要其中一方有误，链接就会打开浏览器，而且没有任何提示说明原因。**Deep links** 会在设备上打开链接，并**检查**双方。

## 在应用中

在侧边栏中打开 **Deep links**，输入链接，然后：

- **Open on the device** 会打开该链接，并记住它；
- **Check** 会读取域名的文件，询问设备的判断结果，并列出正确的项（✓）、需要留意的项（!）和错误的项（✗），每一项都附有处理建议。

历史记录会保留最近 100 个链接及其上次检查的结果，并可以**固定**你常用的链接。

![深层链接检查及其发现的问题](images/deep-links.png)

## 在命令行中

```bash
emulabzip link open "myapp://open/cart" --app com.example.shop
emulabzip link check "https://shop.example.com/cart/3" --app com.example.shop        # exits with an error when something is wrong
emulabzip link verify com.example.shop       # asks Android to verify the app's domains again
emulabzip link history
```

`link check --json` 会输出检查结果，供 CI 使用。[MCP](mcp.md) 提供 `check_link`。

## 检查内容

**在 Android 上**

- `https://<domain>/.well-known/assetlinks.json` 返回状态码 200，没有重定向（Android 不会跟随重定向），类型为 `application/json`，并且内容有效；
- 它授权了该应用：应用的包名位于带有 `delegate_permission/common.handle_all_urls` 的声明中；
- 应用的**签名证书匹配**：设备报告的 SHA-256（`pm get-app-links`，Android 12 及更高版本）是文件中列出的其中之一。调试构建使用调试密钥签名，而不是发布密钥；
- **设备已为该应用验证域名**，并且用户的“Open supported links”开关已打开；如果没有，修复方法是 `pm verify-app-links --re-verify <app>`，**Verify again** 和 `link verify` 会运行它；
- **谁打开链接**（`cmd package resolve-activity`）：你的应用、其他应用，或没有应用。当域名已验证时，列出浏览器的选择器不会被报告，因为已验证的链接会直接进入应用。

**在 iOS 上**

- `https://<domain>/.well-known/apple-app-site-association` 返回 200，没有重定向，并且内容有效（带有 `components` 的 `applinks`，或较旧的 `paths`）；
- 应用的 bundle id 位于 `appIDs` 中，并且有**规则匹配链接的路径**：第一个匹配的组件起决定作用，`exclude` 规则可以拒绝该链接；
- 应用的 **Associated Domains entitlement** 包含 `applinks:<domain>`，并且其团队 ID 与文件中的一致（在模拟器上从应用的签名中读取；未签名的构建没有团队 ID，检查会说明这一点）。

**对于应用自身 scheme 的链接**（`myapp://…`）没有域名文件；检查会说明由谁打开它。

## 限制

- 模拟器验证域名的方式与真实设备不同，因此 iOS 检查针对的是文件和 entitlement。
- `pm get-app-links` 需要 Android 12 或更高版本。在更旧的版本上，文件仍然会被检查。
- 只读取域名自己的文件，通过 HTTPS，最大 256 KB，限时十秒。
- GitHub Pages 以 `application/octet-stream` 提供网站的 `apple-app-site-association`，检查会指出这一点：无法设置类型的主机会显示该警告。实际上 Apple 接受该文件；如果你希望类型正确，请使用其他主机。

## 演示

`tools/compose-app` 将 `https://emulabzip.luispeixoto.dev/demo` 作为已验证的应用链接处理，将 `emulabzip-demo://open/…` 作为它自己的链接处理。网站提供了一个 `assetlinks.json`，其中列出了该演示和构建它所用的调试密钥，因此在安装了该演示的模拟器上，`emulabzip link check https://emulabzip.luispeixoto.dev/demo/cart --app dev.emulabzip.composedemo` 会显示全部 ✓。它的 `apple-app-site-association` 特意使用了占位的团队 ID，因此 iOS 检查会展示不匹配时的样子。
