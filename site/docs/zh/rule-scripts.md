---
source: 45af944a7748
title: 规则脚本：沙箱化的 WebAssembly 脚本
description: 规则脚本是沙箱化的 WebAssembly 小程序，可以检查请求或响应并决定如何处理，例如添加标头、隐藏字段、自行应答，必须先批准其确切的字节才会运行。
---
# 规则脚本

**脚本**是一个小型 WebAssembly 程序，它查看请求或响应，并决定对它做什么：添加标头、隐藏字段、自行应答、让三个请求中的一个失败。动作为 **Script** 的规则会对该规则匹配的每个请求运行它。

脚本运行在沙箱中。它不能读取文件、访问网络、读取时钟，也不能触碰你电脑上的任何其他东西：它唯一能导入的函数是 `env.log`。它在完成固定量的工作和用完固定量的内存之后会被停止。而且只有在**你批准了它的确切字节**之后才会运行。

## 批准

脚本文件在被批准之前不会做任何事。批准在规则的对话框中进行（**Approve this script**，它会显示脚本的 SHA-256 以及它查看什么内容），或在命令行中进行：

```bash
emulabzip script show add-header.wasm       # what it does, and whether it is approved
emulabzip script approve add-header.wasm
emulabzip script list
emulabzip script revoke 09e74ff6            # a file, or the start of its hash
```

批准针对的是文件的 SHA-256：更改一个字节，就必须重新批准。只批准你信任的脚本：脚本会看到其规则匹配的请求和响应，包括它们的标头和正文。脚本没有运行的流程，会在详情中说明原因。

![Rules 对话框，其中有一条尚未批准的脚本规则](images/rules-script.png)

## 接口

脚本是一个导出以下内容的模块：

| 导出 | 类型 | |
|--------|------|-|
| `memory` | memory | 脚本的线性内存。 |
| `alloc` | `(len: i32) -> i32` | 指向 `len` 字节的指针，宿主可以向其中写入。 |
| `on_request` | `(ptr: i32, len: i32) -> i64` | 收到请求时调用。可选。 |
| `on_response` | `(ptr: i32, len: i32) -> i64` | 收到响应时调用。可选。 |

`on_request` 和 `on_response` 至少需要一个。宿主会在 `ptr` 处写入一个 JSON 文档（`len` 字节），函数返回 `(out_ptr << 32) | out_len`，其指向的位置就是它的答复，同样是 JSON。

**输入**

```json
{ "method": "GET", "url": "https://api.example.com/users?id=1",
  "headers": [["accept", "application/json"]],
  "body": "<base64>", "bodyOmitted": false,
  "status": 200 }
```

`status` 只在响应中出现。响应正文是解码后的（gzip、brotli、zstd）。超过 1 MB 的正文不会被交给脚本：`bodyOmitted` 为 `true`，`body` 为空。

**答复**

```json
{ "action": "continue", "status": 201, "headers": [["x-by", "me"]], "body": "<base64>", "text": "…", "message": "…" }
```

- `"continue"`（或没有 `action`）让请求继续或让响应通过。`headers`、`body`（base64）或 `text`，以及响应的 `status`，会替换原有的内容；省略的部分保持不变。标头列表会**替换**整个列表，因此请从输入的列表开始。
- `"replace"`（对请求而言）会立即用该 `status`（默认 200）、`headers` 和正文进行应答，服务器完全不会被请求。对响应而言，它与 `continue` 相同。
- `"block"` 会以网关错误让请求失败，并附上 `message`。

脚本发生陷阱、用完工作量、给出无效的答复，或请求了它没有获得的东西，**不会**破坏请求：请求会像没有这个脚本一样通过，原因记录在流程的详情中。

## 限制

- **工作量：**每次调用 1000 万单位的 fuel，大致相当于同样数量的指令。
- **内存：**64 MB。模块最大 2 MB，答复最大 8 MB。`env.log` 最多 20 行，每行 1 KB。
- 每次调用都使用新的实例，因此脚本保存的任何内容都不会留到下一个请求；当某个选择必须可重复时，请使用 URL。
- 除 `env.log(ptr: i32, len: i32)` 之外没有其他导入，它会写入一行，显示在流程中。

## 编写脚本

任何能编译为 WebAssembly 的语言都可以。`tools/rule-scripts` 里有三个 Rust 示例，每个都为 `wasm32-unknown-unknown` 构建，并带有一个负责打包的小型辅助 crate：

| 脚本 | 作用 |
|--------|--------------|
| `add-header.wasm` | 向每个请求和响应添加 `x-emulabzip-script`。 |
| `redact-json.wasm` | 把 JSON 响应中 `email`、`token`、`password` 和 `authorization` 的值替换为 `***`。 |
| `flaky.wasm` | 对大约三分之一的地址返回 503，这些地址由 URL 选出，因此同一个地址总是失败。 |

```bash
cd tools/rule-scripts && ./build.sh          # needs: rustup target add wasm32-unknown-unknown
```

`dist/` 中的文件已经构建好了。把脚本添加到一条规则中，批准它，然后观察流程。
