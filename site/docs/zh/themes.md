---
source: ae0f1b688c5b
description: 了解 EmuLabZip 的十个内置主题，如何创建、导入、导出自己的主题，文件格式与安全检查，以及如何在社区图库中分享主题。
title: EmuLabZip 主题：使用、创建、导入与分享
---
# 主题

EmuLabZip 自带十个主题，你可以创建自己的主题、导入别人的主题，并在社区图库中分享你的主题。打开 **Settings**（⌘,），进入 **Appearance**。

![带有主题卡片网格的 Appearance 设置](images/themes.png)

## 使用主题

应用为深色和浅色各保留一个主题。选择某个主题会设置其所属基调的那一个，**Mode** 开关（dark、light 或 system）决定显示哪一个。工具栏中的太阳和月亮按钮可在两者之间切换。

内置主题：

| Theme | Base | |
| --- | --- | --- |
| **Midnight** | dark | 默认主题：更浅的深色，更易阅读。 |
| **Classic Dark** | dark | 早期版本中接近纯黑的主题。 |
| **Graphite**, **Fjord**, **Ember**, **Forest** | dark | 中性灰、冷蓝、暖色和绿色。 |
| **Daylight** | light | 默认的浅色主题。 |
| **Paper** | light | 更柔和、更温暖的浅色。 |
| **High Contrast Dark** and **Light** | dark, light | 文字达到 AAA 对比度。 |

## 创建主题

**New theme** 会打开编辑器，任何卡片上的 **Duplicate** 会以该主题为起点。编辑时整个应用都会显示你的更改；**Cancel** 会恢复所选的主题。

- **Start from one colour** 会根据一种强调色填充各个表面和文字，使用该基调所需的亮度。
- 每种颜色都有一个取色器和一个文本框。颜色可以是 `#rgb`、`#rrggbb`、`#rrggbbaa`、`rgb()` 或 `hsl()`。
- **Fonts** 是字体族名称，用逗号分隔。它们必须已安装在电脑上；不会下载任何内容。
- **Roundness** 从方形（0）到非常圆（2）。
- 当文字难以阅读时，编辑器会发出警告：它会检查主要颜色组合的 WCAG 对比度（窗口和面板上的文字、次要文字、提示、强调色、错误、警告）。

在卡片下方，**Fine-tune** 可更改属于你、而不属于任何主题的内容：文字大小（85% 到 140%）、界面缩放、密度、圆角和字体。**Reset the fine-tuning** 会将它们恢复。

![带有对比度警告的主题编辑器](images/theme-editor.png)

## 文件格式

主题是一个 JSON 文件，包含几百字节的颜色，最大 64 KB：

```json
{
  "schema": 1,
  "id": "aurora",
  "name": "Aurora",
  "author": "EmuLabZip",
  "description": "Violet accents on a deep, cool grey.",
  "base": "dark",
  "colors": { "bg": "#1d1a23", "panel": "#24212c", "fg": "#f1eff7", "accent": "#8b5cf6" },
  "fonts": { "sans": "Inter, system-ui, sans-serif", "mono": "Menlo, monospace" },
  "shape": { "radius": 1 }
}
```

颜色名称就是编辑器中的分组：表面（`bg`、`panel`、`panel-2`、`elevated`、`hover`、`line`、`line-strong`）、文字（`fg`、`muted`、`faint`）、强调色（`accent`、`accent-2`、`accent-soft`、`select`）、状态（`ok`、`warn`、`err`、`info`）、图表（`series-1`、`series-2`、`flame-app`、`flame-framework`、`flame-native`、`flame-kernel`）和日志级别（`lv-verbose`、`lv-debug`、`lv-info`、`lv-warn`、`lv-error`、`lv-fatal`）。省略的颜色取自该 `base` 的默认主题。

### 为什么打开主题是安全的

主题可能来自任何人，因此在使用之前会经过检查：

- 颜色必须是十六进制颜色，或带数字的 `rgb()`/`hsl()`，不能是其他任何形式；
- 字体是一个字体族名称列表。`url(`、花括号和分号会被拒绝，因此主题无法携带 CSS 或加载文件；
- 未知的键会被丢弃，文件不能大于 64 KB；
- 应用的内容安全策略不允许任何远程内容，图库自己的地址除外。

## 导入和导出

卡片上的 **Export** 会将主题保存为 `<id>.json`。**Import file** 读取一个文件，**Paste JSON** 接收文本。主题会先显示在卡片上，只有在你点击 Install 后才会安装。

## 社区图库

**Gallery** 标签页列出其他人分享的主题，数据读取自 `https://emulabzip.luispeixoto.dev/themes/index.json`。安装后它就归你使用和编辑。离线时，该标签页会说明这一点，其他一切照常工作。

![带有社区主题的 Gallery 标签页](images/theme-gallery.png)

### 分享你的主题

1. 在编辑器中为主题设置作者和描述。
2. 在 **Gallery** 中，于 **Share a theme** 下选择该主题，然后点击 **Open on GitHub**。
3. GitHub 会打开一个新文件 `site/static/themes/<id>.json`，其中包含你的主题。登录后点击 **Propose new file**；它会变成一个拉取请求。

你也可以在拉取请求中手动把文件添加到 `site/static/themes/`。站点构建会用与应用相同的规则检查每个文件（`node site/build.mjs --check`），检查失败的拉取请求无法合并：

- 文件以其 `id` 命名，该 id 必须是小写字母、数字和连字符，且不能是内置主题的 id；
- 它有 `author` 和 `description`；
- 每种颜色和字体都通过上述检查；
- 文字可读：窗口和面板上的文字及次要文字至少达到 4.5:1。

应用读取的索引由构建根据这些文件生成，因此无需再编辑其他内容。
