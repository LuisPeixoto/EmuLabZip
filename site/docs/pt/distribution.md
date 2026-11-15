---
source: c6b502e6e414
description: Como publicar uma versão do EmuLabZip com o GitHub Actions a partir de uma tag, a assinatura de código opcional e como ativar as atualizações no app.
---
# Distribuição

As versões são geradas pelo GitHub Actions a partir de uma tag de versão.

## Publicando uma versão

1. Atualize a versão em `Cargo.toml` (`workspace.package.version`), `apps/desktop/package.json` e
   `apps/desktop/src-tauri/tauri.conf.json`.
2. Adicione a versão ao `CHANGELOG.md`.
3. Faça o merge em `main`, depois crie a tag e envie:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

O [workflow de Release](../.github/workflows/release.yml) então:

- gera os instaladores do desktop com `tauri-action` para macOS (Apple silicon e Intel), Linux (`.deb`, `.rpm`,
  `.AppImage`) e Windows (`.msi`, `.exe`);
- gera a CLI `emulabzip` para macOS, Linux (x86_64 musl e arm64) e Windows;
- anexa tudo, mais o `SHA256SUMS`, a uma versão em **rascunho**. Revise e publique.

## Assinatura de código (opcional)

Os instaladores são assinados quando estes secrets do repositório existem; sem eles o build ainda funciona, só que sem assinatura.

| Plataforma | Secrets |
|------------|---------|
| macOS | `APPLE_CERTIFICATE` (`.p12` em base64), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (específica do app), `APPLE_TEAM_ID` |
| Windows | `WINDOWS_CERTIFICATE` (`.pfx` em base64), `WINDOWS_CERTIFICATE_PASSWORD` |

Os builds de macOS sem assinatura precisam ser abertos com clique direito → **Open** na primeira vez.

## Atualizações no app

Ainda não estão ativadas. Para ligá-las, gere um par de chaves do updater com `npx tauri signer generate`, guarde a chave privada
como `TAURI_SIGNING_PRIVATE_KEY` (e a senha dela como `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), adicione o plugin do updater com
a chave pública ao `tauri.conf.json` e defina `includeUpdaterJson: true` no workflow de release.
