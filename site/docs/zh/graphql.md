---
source: 57ea062b424d
title: GraphQL 请求：按操作名称显示查询与变量
description: 在 Network 中，GraphQL 请求会以其所请求的内容命名，并显示查询和变量，这样一长串 POST /graphql 请求也能一目了然，批量请求同样适用。
---
# GraphQL

![Network 中的 GraphQL 请求，显示为查询和变量](images/network-graphql.png)

在 **Network** 中，GraphQL 请求会以其所请求的内容命名，因此一长串 `POST /graphql` 也能看得清楚。

- **Operation** 显示在路径之前：客户端发送的 `operationName`，否则是写在 `query`、`mutation` 或 `subscription` 之后的名称。没有名称的操作显示为 `anonymous query`（或 `mutation`）。批量请求（一个正文中的一组请求）会显示前几个名称以及还有多少个。
- GraphQL 请求通过其正文识别：带有 `query` 的 JSON `POST`、`application/graphql` 正文，或带有 `?query=` 的 `GET`。
- 过滤器也会搜索操作名称。
- **Request** 标签页：按原样书写的查询，然后是 JSON 形式的变量，以及一个 **GraphQL** 徽标。在批量请求中，每个操作都有自己的标题。
- 在命令行中，`emulabzip net` 会在每个请求之后打印操作。

GraphQL 调用的响应与其他响应一样是 JSON。即使操作失败，GraphQL 服务器通常也会返回 `200`，失败信息放在 `errors` 列表中：打开 **Response** 标签页即可查看。
