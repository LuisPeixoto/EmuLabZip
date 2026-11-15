---
source: 0fcc75cc4e31
description: 介绍 Layout 视图如何用截图和视图树显示屏幕上的视图，以及基于同一棵树的无障碍审计规则、色盲模拟、命令行与 AI 助手的用法。
---
# 布局与无障碍

**Layout** 显示屏幕上的视图：一张给每个视图加了边框的截图、视图树，以及
你所选视图的属性。同一棵树还为**无障碍审计**提供数据，用来查找屏幕阅读器或手指会遇到的障碍。

## Layout 视图

点击 **Capture**。截图和视图树会一起读取，因此两者相互吻合。

- 在截图上移动，会勾勒出指针下的视图；点击即可选中。树会展开到该视图。
- **Outline every view** 一次画出所有边框。
- 按文本、标签、id 或类搜索树。
- 属性显示类、文本、标签、id、位置、大小（Android 上以 dp 为单位）以及标志：clickable、focusable、
  editable、scrollable、disabled、checked。

### 视图树从哪里来

| | 来源 |
|---|---|
| Android | `uiautomator dump`，每个镜像都有。它就是 TalkBack 读取的无障碍树。不会安装任何东西。 |
| iOS Simulator | Simulator 窗口上的 macOS Accessibility API，它带有 iOS 无障碍树（VoiceOver 遍历的内容），含标签和标识符。 |

iOS 需要两件事：Simulator 窗口必须在该设备上打开，并且运行 EmuLabZip 的程序
（应用，或命令行时的终端）必须在 **System Settings, Privacy & Security,
Accessibility** 中获得允许。未允许时 EmuLabZip 会提示。位置会转换为点（point），因此与应用
声明的一致。不支持真机。

`uiautomator` 无法读取禁止读取的屏幕（安全窗口、某些视频播放器）；EmuLabZip 会报告这一点，而不是
显示一棵空树。

## Compose、Flutter、React Native 和 SwiftUI

由 UI 框架绘制的视图在树中带有标记（`compose`、`flutter`、`react-native`、`swiftui`），检查器
会标明框架。在 Android 上，`ComposeView`、`FlutterView` 或 React Native 根视图下的所有内容都会被标记。
在 iOS 上，在工具栏中选择应用（或在命令行中传入 `--app <bundle id>`），屏幕就会标记为
该应用所用的框架。见[框架内部](frameworks.md)。

## 审计

**Accessibility** 标签页列出发现的问题，最严重的在前。点击其中一项即可查看视图以及该怎么处理。

| 规则 | 检查内容 | WCAG |
|------|-----------|------|
| `missing-label` | 可以点按或输入、却没有名称的视图：没有文本、没有标签，其内部也没有任何带名称的内容 | 1.1.1, 4.1.2 |
| `unlabeled-focusable` | 能获得焦点却什么也不说的视图 | 4.1.2 |
| `touch-target` | 任一方向小于 48 dp（Android）或 44 pt（iOS）的控件 | 2.5.8 |
| `contrast` | 对比度低于 4.5:1（大号文字为 3:1）的文字，根据视图内截图的像素测量 | 1.4.3 |
| `duplicate-label` | 相邻控件的朗读方式相同（“Delete”、“Delete”、“Delete”） | 2.4.6 |
| `focus-order` | 焦点在屏幕上向上跳回：布局把一个控件列在它上方的控件之前（屏幕阅读器和键盘遵循布局的顺序）。同一行中的控件不算跳转；最多报告五个 | 2.4.3 |

对比度的测量方法是：取视图中最常见的颜色作为背景，取与之差距最大的颜色作为
文字。位于照片或渐变之上的视图不做判断，因为没有单一的背景。禁用的控件
不要求足够大，也不要求有名称。

### 色盲

```bash
emulabzip a11y --simulate deuteranopia -o deuteranopia.png    # protanopia, deuteranopia, tritanopia or achromatopsia
```

显示具有该类色盲的人所看到的屏幕（Machado、Oliveira 和 Fernandes 的矩阵，
最大严重程度，作用于线性光），写出图片，并列出**原本可读、现在不再可读的
文字**（`contrast-<kind>`）。本来就未通过对比度规则的文字不会重复列出。通过 [MCP](mcp.md)，
`simulate_color_blindness` 会返回图片和发现的问题。**Audit** 视图在 *Colour vision* 下提供同样的功能。

### 文字大小、语言和放不下的文字

见[语言、文字大小和放不下的文字](localization.md)：`i18n check` 和 `i18n sweep` 查找
被截断、跑出屏幕或没有镜像翻转的文字，扫描还会报告在大号文字下变得更糟的触摸目标和焦点顺序。

这只是仅基于布局的初步检查。它不能取代用 TalkBack 或 VoiceOver 实际试用应用，也无法
知道某个标签是*错误的*。

## 命令行和助手

```bash
emulabzip layout                       # the tree as text
emulabzip layout --json --png screen.png
emulabzip a11y                         # the findings
emulabzip a11y --json
emulabzip a11y --fail-on warning       # exit 1 when something at least this serious is found (note, warning, error)
```

`a11y --fail-on` 可以在屏幕出现退化时让 CI 步骤失败。[MCP 服务器](mcp.md)提供 `get_layout` 和
`audit_accessibility`，因此助手可以查看屏幕并修复它发现的问题。
