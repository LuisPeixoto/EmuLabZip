---
source: 7a600d6f8eb4
description: 了解如何编写 EmuLabZip 插件：一个包含 plugin.json 和程序的文件夹，程序按行输出 JSON，应用将其绘制为图表、表格、文本和日志。
---
# 插件

插件是你自己的采集器。它是一个包含 `plugin.json` 和一个程序的文件夹。EmuLabZip 为某台设备启动该程序，程序在标准输出上每行打印一个 JSON 对象，EmuLabZip 再把它们绘制出来：指标显示为图表或磁贴，另有表格、文本和日志行。

应用内部没有任何插件代码。插件无法改变界面，也无法破坏界面，它只能做你自己启动的程序能做的事。

<p align="center">
  <img src="images/plugins.png" alt="绘制电池温度的流式插件" />
</p>

## 安装、启用、运行

```bash
emulabzip plugin new my-plugin                       # a template in the plugins folder
emulabzip plugin install examples/plugins/top-processes   # copy a plugin folder in
emulabzip plugin list
emulabzip plugin enable my-plugin
emulabzip plugin run my-plugin --app com.example.app
```

在应用中，打开 **Plugins**。插件位于 EmuLabZip 数据文件夹内的 `plugins` 文件夹中（**Open folder**）。

**插件是以你的身份运行的程序。**因此在你启用它之前它不会运行，启用时会显示确切的命令和运行位置。批准与 `plugin.json` 的内容绑定：只要修改该文件，插件就会再次被禁用。应用和 MCP 服务器只运行已启用的插件。在命令行中，`emulabzip plugin run <name>` 就是你自己要求运行它。启用插件之前请先阅读它。

## `plugin.json`

```json
{
  "protocol": 1,
  "name": "battery-temperature",
  "version": "1.0.0",
  "description": "Plots the battery temperature of an Android device.",
  "platforms": ["android"],
  "command": ["./run.sh"],
  "mode": "stream",
  "timeoutS": 60
}
```

| Field | 含义 |
|-------|---------|
| `protocol` | 必须是 `1`。 |
| `name` | 字母、数字、`-` 和 `_`。必须与文件夹同名。 |
| `version`, `description` | 显示在列表中。 |
| `platforms` | `android`、`ios`，或两者都有。为空表示两者都有。 |
| `command` | 程序及其参数，绝不通过 shell 运行。程序先在插件文件夹中查找，然后在 PATH 中查找。 |
| `mode` | `once`：程序打印结果后退出。`stream`：程序持续运行并随时打印，直到被停止。 |
| `timeoutS` | 对 `once`：程序被停止前的秒数（默认 60，最多 600）。 |

## 程序会得到什么

工作目录是插件的文件夹，环境中包含：

| Variable | 值 |
|----------|-------|
| `EMULABZIP_DEVICE` | adb 序列号或模拟器 UDID |
| `EMULABZIP_PLATFORM` | `android` 或 `ios` |
| `EMULABZIP_APP` | 所选的应用（包名或 bundle id），未选时为空 |
| `EMULABZIP_ADB` | `adb` 的路径（SDK 中的副本，或 `adb`） |
| `EMULABZIP_PLUGIN_DIR` | 插件的文件夹 |
| `EMULABZIP_PROTOCOL` | `1` |

## 程序打印什么

每行一个 JSON 对象，带有 `type`：

```json
{"type":"metric","name":"battery temperature","value":25.0,"unit":"°C"}
{"type":"table","title":"Top processes","columns":["PID","MB","Name"],"rows":[[552,217.9,"system_server"]]}
{"type":"text","title":"Picked app","body":"com.example uses 120 MB"}
{"type":"log","level":"warn","message":"adb was slow"}
{"type":"error","message":"The device did not answer"}
```

- `metric`：一个数字。在 `stream` 模式下，每个值都是图表上的一个点（保留最近 120 个）；在 `once` 模式下，它是一个磁贴。
- `table`：单元格是文本、数字、布尔值或 null。一行可以比列数短，但不能更长，单元格不能包含列表或对象。
- `text`、`log`：原样显示。
- `error`：显示为错误。如果程序以错误码退出且什么也没说，则会根据其退出码和错误输出的末尾生成一条错误。

不是 JSON 的行，以及此版本不认识的类型的事件，都会被跳过，因此插件可以为人打印进度，也可以放心使用较新的事件。当插件流式输出时，请在每行之后刷新。

## 限制

一行最长 256 KB（更长的会被跳过），一次运行总共最多打印 8 MB，超过后插件会被停止。超过超时时间的 `once` 插件会被停止。停止运行、关闭应用或禁用插件都会终止该程序。

## 示例

`examples/plugins` 中有两个：`battery-temperature`（流式输出指标的 `sh` 脚本）和 `top-processes`（打印表格的 Python 脚本）。`emulabzip plugin new` 会写出一个打印指标和一些文本的 Python 模板。
