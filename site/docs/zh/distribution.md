---
source: c6b502e6e414
description: 说明如何发布 EmuLabZip：更新版本号并打标签后，由 GitHub Actions 构建桌面安装包和命令行工具，附带校验和并生成草稿版本，以及可选的代码签名和应用内更新。
title: EmuLabZip 发布分发：打标签构建、代码签名与应用内更新
---
# 发布分发

版本由 GitHub Actions 根据版本标签构建。

## 制作版本

1. 更新 `Cargo.toml`（`workspace.package.version`）、`apps/desktop/package.json` 和 `apps/desktop/src-tauri/tauri.conf.json` 中的版本号。
2. 把这个版本加入 `CHANGELOG.md`。
3. 合并到 `main`，然后打标签并推送：

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

随后，[Release 工作流](../.github/workflows/release.yml)会：

- 用 `tauri-action` 构建桌面安装包，覆盖 macOS（Apple 芯片和 Intel）、Linux（`.deb`、`.rpm`、`.AppImage`）和 Windows（`.msi`、`.exe`）；
- 为 macOS、Linux（x86_64 musl 和 arm64）和 Windows 构建 `emulabzip` CLI；
- 把所有产物连同 `SHA256SUMS` 附加到一个**草稿**版本。检查后再发布它。

## 代码签名（可选）

当存在这些仓库 secret 时，安装包会被签名；没有它们时构建仍会成功，只是不签名。

| 平台 | Secret |
|----------|---------|
| macOS | `APPLE_CERTIFICATE`（base64 `.p12`）、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`（应用专用密码）、`APPLE_TEAM_ID` |
| Windows | `WINDOWS_CERTIFICATE`（base64 `.pfx`）、`WINDOWS_CERTIFICATE_PASSWORD` |

未签名的 macOS 构建在第一次打开时必须右键单击并选择 **Open**。

## 应用内更新

尚未启用。要启用，请用 `npx tauri signer generate` 生成更新器密钥对，把私钥存为 `TAURI_SIGNING_PRIVATE_KEY`（其密码存为 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`），把带公钥的更新器插件加入 `tauri.conf.json`，并在发布工作流中设置 `includeUpdaterJson: true`。
