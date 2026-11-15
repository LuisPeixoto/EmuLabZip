---
source: 2240302ecefc
description: 介绍如何把应用的数据库、偏好设置和文件保存为 .ezstate 文件、恢复它并比较两次保存之间的结构差异，包括保存内容、恢复的安全措施和限制。
---
# 应用状态：保存、恢复与比较

<p align="center">
  <img src="images/state-compare.png" alt="在 Storage 中打开的一个已保存状态，与另一个对比：新增了一列、一个索引和一张表" />
</p>

应用的状态就是它保存的内容：数据库、偏好设置、文件。**Save state** 会把这一切放进一个 `.ezstate` 文件；**Restore** 会把它放回去；**compare** 会说明两次保存之间其结构发生了什么变化。想每次都用相同的数据重现缺陷、测试迁移，以及让测试从已知的起点开始，都需要它。

```bash
emulabzip state save com.example.shop -o v1.ezstate
# …use the app, or install the new version…
emulabzip state save com.example.shop -o v2.ezstate
emulabzip state diff v1.ezstate v2.ezstate
emulabzip state restore v1.ezstate --yes
```

## 保存的内容

| | Android | iOS 模拟器 |
|---|---------|---------------|
| 内容 | 应用的私有文件夹（`/data/data/<package>`），不含 `cache`、`code_cache` 和 `lib`；使用 `--external` 时还包括它在共享存储上的文件夹 | 数据容器（`Documents`、`Library`、`tmp`） |
| 方式 | `run-as <package> tar`（应用必须是**可调试**的，与 [Storage](storage.md) 相同） | 由 EmuLabZip 写出的容器 tar |
| 应用 | 先被停止，因此没有写到一半的文件 | 先被终止 |

`.ezstate` 文件是一个 zip：`state.json`（应用、其版本、设备、时间）、tar，以及 `schemas.json`，后者描述了每个 SQLite 数据库（表、列、索引）以及每个偏好设置、DataStore 和属性列表文件（键和类型）。正是这份描述让比较变得很快：无需解压任何内容。

## 恢复

`state restore` 会用文件中的内容替换应用的数据。它会：

1. 拒绝在其他平台上生成的文件，并在应用版本与保存状态时的版本不同时给出警告；
2. 需要 `--yes`（桌面应用会先询问）；
3. 把被替换的数据**保存**到 EmuLabZip 数据文件夹的 `states/auto/` 中的一个文件里，并打印其名称。恢复那个文件即可撤销这次恢复。如果无法创建备份，则不会替换任何内容；
4. 停止应用，清空应用的文件夹（在 Android 上保留 `lib`），解开 tar，并在有任何失败时说明。

在写入任何内容之前，tar 的每个条目都会被检查：不允许绝对路径、`..`、硬链接、设备文件，以及指向文件夹之外的符号链接。别人提供的 `.ezstate` 文件无法写到应用之外。

Android 以应用的身份（`run-as`）执行解包，因此文件的所有者仍然是该应用。

## 结构差异

每处变化一行：`+` 表示新增，`-` 表示删除，`~` 表示变更，后面是文件、它是什么及其名称，以及详细信息。

```
$ emulabzip state diff v1.ezstate v2.ezstate
+ databases/shop.db: column cart.note  TEXT
+ databases/shop.db: index idx_cart_sku  CREATE INDEX idx_cart_sku ON cart (sku)
+ databases/shop.db: table orders  3 columns
```

它会列出表、列（及其类型、`NOT NULL` 和主键）、索引，以及偏好设置、DataStore 和属性列表文件的键和类型，包括新增、删除或变更的。行内容不会被比较：它用于检查迁移，而不是比较数据。不带第二个文件的 `state diff v1.ezstate` 会与应用当前的状态比较。`--json` 会以数据形式给出结果。

## 在桌面应用中

Storage 的工具栏中有 **Save state…** 和 **Open state…**。打开的状态会显示其中的内容，并提供 **Compare with the app now**、**Compare with another file…** 和 **Restore…**。Restore 会先询问，并说明旧数据保存在哪里。

## 面向 AI 助手

通过 [MCP](mcp.md)，提供 `save_state` 和 `diff_state`，`list_states` 会显示已保存的名称。**不**提供恢复：AI 助手可以看到发生了什么变化，但不能替换应用的数据。

## 限制

- Android 需要可调试的应用。任何工具都无法读取 release 构建的私有文件夹。
- 仅存在于内存中的数据、钥匙串，以及其他应用为本应用保存的内容（账户、content provider）不属于该文件夹，因此也不属于状态。
- iOS：不保存 app group 容器。
- 状态不包含应用本身：恢复之前请先安装相同的版本。
