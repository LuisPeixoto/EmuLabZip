---
source: 18e7488c6820
description: 介绍如何导入 .pcap 或 .pcapng 抓包文件，像查看实时流量一样查看其中的请求、连接和 DNS 查询，以及如何在 Android 模拟器和 Mac 上抓包。
---
# 导入抓包文件

EmuLabZip 可以打开 `.pcap` 或 `.pcapng` 文件，并像显示实时流量一样显示其中的内容。当你无法使用代理时，它很有用：例如应用忽略代理设置、流量来自系统服务，或者别人发给你一份抓包文件。

```bash
emulabzip net import capture.pcap            # lists the requests
emulabzip net import capture.pcap --dns      # and the DNS lookups
emulabzip net import capture.pcap --har out.har
```

在应用中，点击 Network 视图里的 **Import**。这些请求会带着 `PCAP` 标记加入列表，横幅会显示文件包含的内容，点一下即可查看 DNS 查询。

## 解析结果

| 抓包中的内容 | 在 EmuLabZip 中 |
|----------------|--------------|
| TCP 上的明文 HTTP/1.x | 请求和响应，包含请求头、正文（分块和压缩的正文会被解码）以及耗时 |
| TLS 连接 | 一个**连接**，以其第一条消息中的服务器名称（SNI）命名，并显示双向的字节数 |
| 其他任何 TCP 连接 | 指向其地址的连接 |
| UDP 53 端口上的 DNS | 查询列表，附带应答和状态 |

TLS 保持加密，因此无法从抓包中获得其内容；请使用代理。TCP 流会按序列号重新拼合：乱序和重传的分段都会处理，缺失的部分会如实报告而不是猜测（“some packets are missing”）。分片的 IP 包会被跳过。最大可读取 512 MB 的文件。

支持以太网、Linux “cooked”（`any` 接口）、原始 IP 和回环抓包，支持各种字节序，也支持纳秒级时间戳。

## 抓包

在 Android 模拟器上，用 `-tcpdump` 启动它：

```bash
emulator -avd Pixel_9 -tcpdump capture.pcap
```

模拟网络传输的所有内容都会写入该文件，模拟器退出后文件才完整。在已获取 root 的镜像或设备上，可以在 shell 中使用 `tcpdump`：`adb shell tcpdump -i any -w /sdcard/capture.pcap`，然后用 `adb pull` 取回。在 Mac 上，对回环接口运行 `tcpdump -i lo0 -w capture.pcap` 即可抓取模拟器的流量。
