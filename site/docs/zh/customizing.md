---
source: 239678ac99f8
description: 了解如何自定义 EmuLabZip 的布局：拖动调整窗格大小、折叠和重排侧边栏、设置页面宽度，以及在 Settings 中调节文字大小、密度和界面缩放。
---
# 自定义布局

EmuLabZip 中所有一分为二的部分都可以调整大小，其余的可以在 **Settings**（⌘,）中调节。

![侧边栏已折叠、详情窗格已调整大小的 Logs 视图](images/layout-custom.png)

## 调整窗格大小

拖动两个窗格之间的分隔线来改变它们的大小。**双击**分隔线可恢复其大小，也可以用 Tab 键聚焦它，再使用方向键（按住 Shift 可加大步长）。每个窗格的大小都会被记住，并且在窗口变小时保持在窗口之内。

这适用于侧边栏、Storage、Network、Logs、Layout、Sessions、Crashes、Traces、Audit、Plugins、Memory 和 Visual 的列表与详情窗格，以及数据库和行查看器的表格列表。

**Settings → Layout → Reset all pane sizes** 会恢复所有窗格的大小。

## 侧边栏

- 使用 ⌘\ 或底部的按钮将其**折叠**为图标。将鼠标悬停在图标上可看到名称。
- **Customize**（同样在底部）可以让你在所属分区内拖动条目调整顺序，点击某个条目可隐藏或显示它。**Reset** 会恢复原来的顺序和条目。

## 页面宽度

Overview、Performance、Controls、Frameworks、Compare、Devices 和 Audit 是居中的页面。**Settings → Layout → Page width** 可将它们设为窄、宽（默认）或全宽。

## 文字、密度和缩放

在 **Settings → Appearance → Fine-tune** 中：

- **Text size**，85% 到 140%，缩放所有文字。
- **Density**（compact、normal、roomy）改变内边距和间距所占的空间。
- **Interface scale**，80% 到 140%，缩放所有内容。⌘+ 和 ⌘- 每次调整一档，⌘0 恢复。
- **Roundness** 和**字体**可以一次性设置，覆盖每个主题自带的设置。

主题本身的颜色和字体见[主题](themes.md)。

## 窗口

在 macOS 上，无论缩放比例如何，窗口按钮都在工具栏中居中。
