---
source: 685bcdfe2f24
description: 介绍如何更改设备的语言和文字大小、检查屏幕上放不下的文字，以及用一条命令在多种语言和字号下扫描界面，包括 Android 与 iOS 模拟器的差异和限制。
---
# 语言、文字大小与放不下的文字

<p align="center">
  <img src="images/audit-text.png" alt="Audit 视图发现一个标签对于它所在的行来说太长" />
</p>

文字最容易让布局出问题：德语单词更长，阿拉伯语从右向左书写，需要大字体的人会把它设为 200%。EmuLabZip 可以**更改**设备的语言和文字大小，**检查**屏幕上放不下的文字，并用一条命令在多种语言和字号下**扫描**一个界面。

## 更改设备

```bash
emulabzip control locale en-XA --app com.example.shop   # long, accented text
emulabzip control locale ar-XB --app com.example.shop   # right to left
emulabzip control reset-locale --app com.example.shop
emulabzip control font-scale 130                        # percent of normal; 100 puts it back
emulabzip control display --width 720 --height 1600 --density 320     # Android
emulabzip control reset-display
```

| | Android | iOS 模拟器 |
|---|---------|---------------|
| 语言 | `cmd locale set-app-locales`：**一次一个应用**，Android 13 及更高版本。伪区域设置 `en-XA` 和 `ar-XB` 可用。应用会收到通知并重新创建其界面 | 对**整个模拟器**使用 `defaults write -g AppleLanguages` 和 `AppleLocale`；应用会在**重新启动时**采用它 |
| 文字大小 | `settings put system font_scale` | `simctl ui … content_size`：最接近该百分比的 Dynamic Type 大小（130% 是 `extra-extra-extra-large`，200% 是 `accessibility-extra-large`） |
| 显示尺寸和密度 | `wm size`、`wm density` | 不可用 |

语言标签由字母、数字和连字符组成（`de-DE`、`pt-BR`、`zh-Hans-CN`）。文字大小为 50% 到 300%。所有这些也都在 **Controls** 视图中，位于 *Text size and language* 和 *Display size* 下。

## 检查屏幕

```bash
emulabzip i18n check            # the screen as it is now
emulabzip i18n check --rtl      # it is shown in a right-to-left language: also check the layout was mirrored
```

| 规则 | 查找内容 |
|------|-----------|
| `text-off-screen` | 超出屏幕边缘的文字（除非它位于可滚动的内容中） |
| `text-clipped-by-parent` | 在其容器内放不下的文字，因此被容器裁掉 |
| `text-cut-off` | 被限制在所给空间内的单行文字，其笔画延伸到了框的末端：它很可能被截断或以省略号结尾 |
| `rtl-not-mirrored` | 在从右向左的语言中，屏幕上的大部分文字仍在左侧 |

`text-cut-off` 是一种启发式判断。布局说明每个视图在哪里，截图说明其文字是否一直延伸到框的末端。仅仅紧贴文字的标签不会被报告（它的框只和文字一样宽），只有当标签的框在其容器或同一行中的下一个视图没有留出空间的位置结束时，才会报告。消息中会写“may be”。没有截图时，只运行布局规则。

## 扫描

```bash
emulabzip i18n sweep --app com.example.shop --locales en-XA,ar-XB --scales 100,130,200
```

扫描会在每种字号下尝试每种语言，读取布局，并打印一张表：有多少发现、是哪些，以及**哪些比设备当前状态更糟**。完成后（步骤失败时也一样），设备会恢复原状（文字大小和语言）。当任何设置变得更糟时，`--fail-on-worse` 会以 1 退出。在 iOS 模拟器上只扫描文字大小，因为应用只有在重新启动时才会采用新语言。

文字缩放也会通过扫描来检查：在 200% 时，它会报告变得更糟的触摸目标和焦点顺序。

## 面向 AI 助手

通过 [MCP](mcp.md)：`control_locale`、`control_font_scale`、`control_display_size` 和 `check_text`。

## 限制

- Android 更改的是应用的语言，而不是设备的语言。更改整个设备的语言需要 root。
- 这些启发式判断看到的是视图，而不是字形。图标字体、自绘的标签或游戏中的文字不会被判断。
- 伪区域设置依赖于应用把字符串放在资源中；硬编码的文字保持不变。
