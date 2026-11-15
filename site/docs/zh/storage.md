---
source: 236e4411c357
description: 介绍 Storage 工具如何浏览所选应用的文件，并就地打开 SQLite、偏好设置、Realm、Core Data 等常见格式，以及对应的命令行用法。
---
# Storage

Storage 工具浏览工具栏中所选应用的文件，并就地打开常见格式。

## 可以查看的位置

| 根目录 | Android | iOS Simulator |
|------|---------|---------------|
| 应用数据 | `/data/data/<package>`，通过 `run-as` 读取。**仅限可调试的应用。** | 数据容器（`Documents`、`Library`、`tmp`） |
| 共享存储 | `/sdcard/Android/data/<package>`，通过 ADB sync 协议 | — |
| 应用包 | — | 已安装的 `.app` |
| 应用组 | — | 该应用使用的每个应用组容器 |

你的应用的调试构建默认是可调试的。发布构建和大多数预装应用则不是，Android
不允许任何工具读取它们的私有数据。EmuLabZip 会如实说明，而不是显示一个空文件夹。

要在没有自己应用的情况下试用这些工具，请安装示例应用：`tools/sample-app/install.sh`。

## 查看器

| 文件 | 查看器 |
|------|--------|
| SQLite 数据库 | 表和视图，含行数、列类型和主键，每页 100 行，以及一个只读 SQL 输入框 |
| SharedPreferences（`shared_prefs/*.xml`） | 键、类型和值；集合会显示其条目 |
| Preferences DataStore（`datastore/*.preferences_pb`） | 键、类型和值，从 protobuf 解码 |
| Proto DataStore（`datastore/*.pb`） | 不借助 schema 解码的 protobuf：字段编号和值 |
| MMKV（`mmkv/` 中与其 `.crc` 相邻的文件） | 键和值。MMKV 不存储类型，因此每个值按其看起来的类型显示（字符串、数字、字节、集合），并标明这是猜测；原始十六进制只需点击一下即可查看。加密文件会被识别并加以说明 |
| Realm（`*.realm`） | 类及其行数、列类型和行，使用 Realm 文件格式（版本 20 到 24）的读取器按页读取。列表、链接和时间戳会显示；集合、字典、mixed、decimal 和 UUID 列尚未读取，并会如实说明 |
| AsyncStorage（`RKStorage`，iOS 上为 `manifest.json`） | 键和值，JSON 值会美化输出 |
| Core Data（SQLite 存储） | 实体及其属性和行（去掉 `Z` 前缀），与原始表并列 |
| 属性列表（二进制或 XML） | 以 JSON 美化输出 |
| JSON | 美化输出 |
| 图片（PNG、JPEG、GIF、WebP） | 在棋盘格背景上预览 |
| 文本 | 原样显示，最大 2 MB |
| 其他任何文件 | 前 4 KB 的十六进制转储 |

数据库会连同其 `-wal` 和 `-shm` 文件一起复制到私有临时文件夹，因此应用尚未做检查点的更改也可见。
该副本以开启 `query_only` 的方式打开：查询只能读取，绝不会写入，
应用真正的数据库也绝不会被触碰。临时副本会在 EmuLabZip 退出时删除。

**Save…** 会将原始文件复制到你选择的位置。

![从设备读取的 Realm 文件：它的类，以及 Person 的行](images/storage-realm.png)

## 更改和保存

在你打开 **Edit** 之前，Storage 是只读的：见[更改应用的数据](storage-editing.md)。**Save state…**、
**Open state…** 和 schema 差异见[应用状态](app-state.md)。

## 通过命令行

```bash
emulabzip files com.example.app                   # the first readable root
emulabzip files com.example.app databases --root data
emulabzip pull com.example.app shared_prefs/settings.xml
emulabzip db com.example.app databases/app.db                     # tables
emulabzip db com.example.app databases/app.db --table users       # rows
emulabzip db com.example.app databases/app.db -q "SELECT count(*) FROM users"
emulabzip edit cell com.example.app databases/app.db -t users --rowid 1 --text name=Ana --yes   # see Changing an app's data
```
