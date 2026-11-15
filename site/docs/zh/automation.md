---
source: d4619f203dc1
description: 介绍如何用 EmuLabZip 点按、输入和按键操作 Android 设备，把操作录制成流程、回放，并导出为 Maestro 流程或 shell 脚本，以及自动化视图和限制。
---
# 输入与流程

EmuLabZip 可以触摸设备、在设备上输入文字并按下它的按键；它可以把你的操作**录制**成**流程**，再次回放该流程，并把它导出为 [Maestro](https://maestro.mobile.dev) 流程或 shell 脚本。这一切都仅适用于 **Android**：iOS 没有公开的方式来发送或读取触摸。

## 点按、输入与按键

| 控制 | CLI | 作用 |
|---------|-----|--------------|
| Tap | `emulabzip control tap <x> <y>` | 在屏幕上的某一点触摸，以像素为单位，与截图中一致。 |
| Swipe | `emulabzip control swipe <x1> <y1> <x2> <y2> [--ms 300]` | 拖动。从一点到它自身即为长按。 |
| Text | `emulabzip control text "hello"` | 输入到获得焦点的字段中。仅限可打印的 ASCII。 |
| Key | `emulabzip control key back` | `back`、`home`、`app_switch`、`enter`、`tab`、`delete`、`volume_up`…… |
| Launch | `emulabzip control launch <app>` | 像启动器一样启动应用。iOS 上也可用。 |

位置会被检查（最大 20000 像素），文字会被检查（不能包含 `%s`，Android 会把它读作空格，也不能包含 `input text` 无法输入的任何内容），按键是一份名称列表，因此没有任何内容会未经检查地到达 shell。同样的控制也是 [MCP 工具](mcp.md)：`tap`、`swipe`、`input_text`、`press_key`、`launch_app`。

在应用中，**Overview → Screen → Interact** 会让屏幕图片可以触摸：点击就是点按，拖动就是滑动，按住半秒就是长按。**Controls** 中有一张用于文字和按键的卡片。

## 录制流程

```bash
emulabzip flow record --name "Add to cart" --app com.example.shop     # touch the emulator, then press Ctrl+C
```

请触摸设备本身：在模拟器上，就是它自己的窗口。EmuLabZip 会读取触摸屏（`getevent`），并保留每次点按、长按和滑动，以及它们之间的停顿（一秒或更长的停顿，精确到十分之一秒）。除非你传入 `--no-launch`，否则流程会以一个启动应用的步骤开头。

有两样内容不会被录制，应用也会这样说明：

- **由 EmuLabZip 或 `adb shell input` 发送的点按。**它们是在 Android 更高层注入的，永远不会到达触摸屏，因此在录制时点击 Interact 图片或回放流程不会添加步骤。
- **输入的文字。**键盘不是触摸屏。请事后添加一个文字步骤。

每次触摸之后，屏幕会在后台再次读取，这样下一次点按就能记住**它点中了什么**：视图的 id（Compose 的 `testTag`、Android 资源 id）或它的文字。读取一次屏幕需要两三秒，因此在另一次点按之后不到这个时间就发生的点按，会保留为一个点。

流程是 EmuLabZip 的 `flows` 文件夹中的 JSON 文件，由应用和命令行共用：

```bash
emulabzip flow list
emulabzip flow show add-to-cart          # the steps; --json for the file
emulabzip flow play add-to-cart          # Ctrl+C stops it
emulabzip flow remove add-to-cart
```

当回放所在屏幕的尺寸与录制流程时的不同，回放会把各点缩放到所在的屏幕。每个步骤之后会等待三分之一秒，让屏幕有时间做出反应。

![带有已录制流程的 Automation 视图](images/automation.png)

## Automation 视图

侧边栏中的 **Automation** 用于录制和回放流程，列出已保存的流程，并显示你所选流程的步骤。你可以删除一个步骤，或手动添加文字、按键或停顿，然后点击 **Save changes**。**Maestro** 和 **Script** 会把流程保存为这些格式。

## 导出

```bash
emulabzip flow export add-to-cart --maestro > add-to-cart.yaml
emulabzip flow export add-to-cart --script --out add-to-cart.sh
```

**Maestro。**知道自己视图的点按会写成 `tapOn: id: "checkout"` 或 `tapOn: "Checkout"`；其他的则以屏幕百分比写出一个点（`point: "50%,80%"`）。文字会针对 Maestro 的正则表达式转义，然后为 YAML 加上引号。按键使用 Maestro 的名称（`Back`、`Home`、`Enter`、`Backspace`……），它没有名称的按键则保留为注释。停顿会变成 `waitForAnimationToEnd`。文件中没有断言：请自行添加，因为录制只知道你做了什么，而不知道它应该显示什么。

**Script。**一份由 `emulabzip control …` 行组成的 shell 脚本，行与行之间有 `sleep`，针对 `EMULABZIP_DEVICE` 中的设备。

## 限制

- 仅限 Android。无法在模拟器上录制或回放流程。
- 标签是尽力而为的。在没有触摸的情况下发生变化的屏幕（计时器、网络应答）可能会给点按标上片刻之前在那里的内容的名称；点始终会被保留，而 Maestro 只有在存在标签时才会使用标签。
- 一个流程最多 5000 个步骤，一次停顿最多一分钟。AI 助手的 `play_flow` 会拒绝耗时超过两分钟的流程；请用 `emulabzip flow play` 回放它们。
