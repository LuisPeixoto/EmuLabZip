---
source: 88df05457f3f
title: 网络规则：文件、其他服务器与契约
description: 介绍四个改变请求应答方式的规则动作：用本地文件应答、转发到另一台服务器、按 OpenAPI 文件检查响应、运行脚本，以及如何串联规则。
---
# 网络规则：文件、其他服务器与契约

[模拟响应与断点](mocks.md)可以应答、延迟、阻止或暂停请求。另有四个动作作用于请求被应答的方式，并且规则可以串联。

| 动作 | 会发生什么 |
|--------|--------------|
| **File**（`mapLocal`） | 用这台电脑上的文件应答，或用路径所指向的文件夹中的文件应答。服务器不会被请求。 |
| **Server**（`mapRemote`） | 把请求发送到另一台服务器，就像应用本来就请求的是它一样。 |
| **Contract**（`validate`） | 根据 OpenAPI 文件检查响应，并显示不匹配的地方。它不会改变任何内容。 |
| **Script**（`script`） | 对请求和响应运行 WebAssembly [脚本](rule-scripts.md)。 |

在应用中，这些动作位于 **Network → Rules**，并带有文件选择器。在命令行中，它们写在同一个 `--rules` 文件里。

## 映射到本地

```json
{ "name": "bundled assets", "matcher": { "host": "cdn.example.com", "path": "/assets/*" },
  "action": { "type": "mapLocal", "path": "/Users/me/app/dist" } }
```

使用文件夹时，位于规则自身模式之前的那部分路径会被去掉：使用 `/assets/*` 时，对 `/assets/app.js` 的请求会得到文件夹中的 `app.js`，对某个文件夹的请求会得到它的 `index.html`。使用文件时，该文件就是对规则匹配的每个请求的应答。内容类型来自扩展名（或来自 `contentType`），`status`（默认 200）是应答的状态。**文件夹之外的内容绝不会被提供**：`..`、编码后的 `%2e%2e`、盘符或指向外部的符号链接都会被拒绝。文件最大 50 MB。

流程会显示 **MOCK** 和被提供的文件。

## 映射到远程

```json
{ "name": "use my laptop", "matcher": { "host": "api.example.com", "path": "/api/*" },
  "action": { "type": "mapRemote", "scheme": "http", "host": "localhost", "port": 3000, "pathPrefix": "/v2/" } }
```

规则没有指定的部分保持应用请求时的样子，因此只写主机和端口，就会把相同的路径发送到本地服务器。`pathPrefix` 会替换路径中位于模式第一个 `*` 之前的部分：`/api/users` 变成 `/v2/users`。除非指定了端口，新的协议或主机会使用该协议通常的端口。新服务器会被告知它自己的主机，除非设置了 `"preserveHost": true`。Network 视图会继续显示应用请求的内容，并说明请求被发送到了哪里。映射到远程不适用于 gRPC 调用。

## 契约

```json
{ "name": "the API contract", "matcher": { "host": "api.example.com" },
  "action": { "type": "validate", "spec": "/Users/me/app/openapi.yaml" }, "thenContinue": true }
```

每个响应在发送之后，都会根据 JSON 或 YAML 格式的 **OpenAPI 3.0 或 3.1** 描述进行检查（文件在保存规则时读取，因此无法使用的文件会被立即拒绝）。操作由方法和路径确定（`/users/{id}`，带上服务器的基础路径），schema 由状态（`200`，然后是 `2XX`，然后是 `default`）和内容类型确定，然后验证正文。流程会得到一个 **Contract** 标签页和列表中的一个徽标：

- 响应**匹配**（`ok`）；
- 它**不匹配**：每个问题都带有指向正文的 JSON 指针（`/items/0/name`）和一条消息，规范中没有描述的状态或内容类型也算一个问题；
- 规范**没有描述**该请求（`unknown`），或**没有 schema** 可用来检查这个响应，或者正文不是 JSON（`unchecked`）。

![Contract 标签页中，一个请求有两处违反其 OpenAPI 文件](images/network-contract.png)

离线时，`emulabzip openapi check --spec api.yaml --har run.har` 会检查 HAR 文件中的响应，并在有响应不匹配时以错误退出，这正是 CI 步骤所需要的。只检查响应（不检查请求），只检查 JSON 正文，并且只处理文件内部的 `$ref`。Swagger 2.0 文件会被拒绝，并提示先转换。

## 串联规则

第一条匹配的规则决定结果，除非它写了 `"thenContinue": true`。**改变**请求的规则（延迟、映射到另一台服务器、契约检查、脚本）可以把请求交给它下面的规则；**应答**、阻止或暂停请求的规则（应答、文件、阻止、断点）总是会结束查找。因此，延迟后接文件，就会从文件缓慢地应答；契约检查后接模拟响应，就会检查这个模拟响应。流程会列出所有生效的规则。
