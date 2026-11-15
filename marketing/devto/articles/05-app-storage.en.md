---
title: Look inside your app's storage on an emulator: SQLite, SharedPreferences, MMKV and Realm
description: Browse an app's files and open SQLite, SharedPreferences, DataStore, MMKV and Realm in place, run read-only SQL and save app state with EmuLabZip.
tags: android, ios, sqlite, debugging
series: EmuLabZip
cover: docs/images/storage-realm.png
publish_after: 2026-11-26
pair: 05-app-storage
lang: en
---

Where did that value go? Is the migration right? Why is this user stuck in the onboarding? The answers are in the app's
private storage, and getting at them usually means `adb shell run-as`, pulling a database, finding a viewer for it, and
repeating all of that after the next run.

EmuLabZip's **Storage** tool browses an app's files on the emulator or simulator and opens the common formats in place. This
post shows what it reads, how to run SQL safely, and how to save and compare an app's whole state.

![A Realm file read from the device: its classes, and the rows of Person](docs/images/storage-realm.png)

## Where it can look

| Root | Android | iOS Simulator |
|---|---|---|
| App data | `/data/data/<package>`, read with `run-as`. Debuggable apps only | The data container (`Documents`, `Library`, `tmp`) |
| Shared storage | `/sdcard/Android/data/<package>` | n/a |
| App bundle | n/a | The installed `.app` |
| App groups | n/a | Every app group container the app uses |

Debug builds of your app are debuggable by default. Release builds and most preinstalled apps are not, and Android does not
let any tool read their private data. EmuLabZip says so instead of showing an empty folder.

## What it opens

Pick a file and the viewer follows the format:

- **SQLite**: tables and views with row counts, column types and primary keys, 100 rows per page, and a read-only SQL box.
- **SharedPreferences** and **DataStore** (Preferences and Proto): key, type and value. Proto DataStore is decoded without its
  schema, so you see field numbers and values.
- **MMKV**: key and value. MMKV does not store types, so each value is shown as the type it looks like and marked as a guess.
- **Realm**: classes with row counts, column types and rows.
- **AsyncStorage** (React Native) and **Core Data**: keys and values, or entities with their attributes.
- **Property lists**, JSON, images and text, shown as readable as they can be, and a hex dump for anything else.

Databases are copied to a private temporary folder together with their `-wal` and `-shm` files, so changes the app has not
checkpointed yet are visible. The copy is opened read-only: queries can read but never write, and the app's real database is
never touched.

## The same thing from the terminal

```bash
emulabzip files com.example.app
emulabzip files com.example.app databases --root data
emulabzip pull com.example.app shared_prefs/settings.xml
emulabzip db com.example.app databases/app.db
emulabzip db com.example.app databases/app.db --table users
emulabzip db com.example.app databases/app.db -q "SELECT count(*) FROM users"
```

`files` lists, `pull` copies a file to your computer, and `db` shows the tables, the rows of one table, or the result of a
read-only query.

## Changing data, carefully

Storage is read-only until you turn on **Edit**. Editing changes the app's real files, so it is built to be careful: every
change is checked before the device is touched, the app is stopped so it does not write over your change, the file it
replaces is kept, the new file is read back to check it, and the edit can be undone.

```bash
emulabzip edit cell com.example.shop databases/shop.db -t customers --rowid 1 --text name=Ana --set vip=1 --yes
emulabzip edit pref-set com.example.shop shared_prefs/settings.xml theme dark --yes
emulabzip edit undo <folder the edit printed> --yes
```

SQLite rows and SharedPreferences (and property lists on iOS) can be edited. DataStore, MMKV, AsyncStorage, Realm and Core
Data are read-only, because their formats are not written back.

## Save the whole state, and compare

An app's state is what it keeps: databases, preferences, files. You can save all of it into one file, put it back, and see what
changed between two versions:

```bash
emulabzip state save com.example.shop -o v1.ezstate
# use the app, or install the new version
emulabzip state save com.example.shop -o v2.ezstate
emulabzip state diff v1.ezstate v2.ezstate
emulabzip state restore v1.ezstate --yes
```

`state diff` compares the structure of the data (what a migration added or changed, such as a column, an index or a table),
which is exactly what you want to check after a schema change. Restoring puts you back at a known point to reproduce a bug
again.

![A saved state opened in Storage, compared with another](docs/images/state-compare.png)

## Try it

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Storage guide: [emulabzip.luispeixoto.dev/docs/storage/](https://emulabzip.luispeixoto.dev/docs/storage/)
- App state: [emulabzip.luispeixoto.dev/docs/app-state/](https://emulabzip.luispeixoto.dev/docs/app-state/)
- Website and all the documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
