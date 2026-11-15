---
source: 8c8e10338584
description: 在 Network 中按所调用的方法显示 gRPC 调用及服务器返回的状态，并把 protobuf 消息读成 JSON，既可以没有 schema，也可以使用你的 .proto 文件。
---
# gRPC 与 protobuf

![Network 中的 gRPC 调用，显示其状态和读成 JSON 的 protobuf 消息](images/network-grpc.png)

gRPC 调用在 **Network** 中按它们所调用的方法显示，并带有服务器返回的状态。

- **Operation** 列显示 `Service/Method`，取自路径。
- gRPC 通过 **trailers**（在正文之后发送的标头，`grpc-status`、`grpc-message`）来说明一次调用如何结束。它们会被记录并显示在 **Headers** 标签页中，状态会在列表中显示为 `OK`、`UNAVAILABLE`、`NOT_FOUND`…
- 调用在发生时就以流的方式通过代理，因此流式调用可以工作，长时间的调用会在消息到来时显示它们。请求断点无法暂停 gRPC 调用，因为它需要完整的消息。规则仍然可以延迟、阻止或应答它。
- 与所有 HTTPS 一样，只有基于 TLS 的 gRPC（通过 ALPN 协商的 HTTP/2）会被解密。不支持明文 HTTP/2（`h2c`）。

## 读取消息

gRPC 正文是一系列消息，每条都是 protobuf。**Request** 和 **Response** 标签页以 JSON 列出它们，有两种方式：

- **没有 schema。**线路上的 protobuf 只有字段编号，没有名称。EmuLabZip 仍然会读取它们：`{"1": "hello",
  "2": 150, "3": {"1": 5}}`。文本显示为文本，嵌套消息显示为对象，重复字段显示为列表，其他字节显示为十六进制。不是 protobuf 的消息会如实说明。
- **使用你的 `.proto` 文件。**在 **Network → Network → gRPC and protobuf** 中，选择存放它们的文件夹（即 `import` 路径所相对的那个文件夹）。它们在内存中编译，不需要 `protoc`。这样，调用的方法就会说明其请求和响应是哪些消息类型，消息会以 protobuf 的 JSON 形式显示，并带有真实的字段名称。与其类型不符的消息会回退到没有 schema 的读取方式。

使用 `grpc-encoding: gzip` 压缩的消息会被解压。
