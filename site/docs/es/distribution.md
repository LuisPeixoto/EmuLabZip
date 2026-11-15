---
source: c6b502e6e414
description: Cómo se publica una versión de EmuLabZip con GitHub Actions a partir de una etiqueta, cómo firmar los instaladores y cómo activar las actualizaciones.
---
# Distribución

Las versiones se compilan con GitHub Actions a partir de una etiqueta de versión.

## Publicar una versión

1. Actualiza la versión en `Cargo.toml` (`workspace.package.version`), `apps/desktop/package.json` y
   `apps/desktop/src-tauri/tauri.conf.json`.
2. Añade la versión a `CHANGELOG.md`.
3. Haz merge a `main`, crea la etiqueta y súbela:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

Entonces el [workflow de Release](../.github/workflows/release.yml):

- compila los instaladores de escritorio con `tauri-action` para macOS (Apple silicon e Intel), Linux (`.deb`, `.rpm`,
  `.AppImage`) y Windows (`.msi`, `.exe`);
- compila la CLI `emulabzip` para macOS, Linux (x86_64 musl y arm64) y Windows;
- adjunta todo, junto con `SHA256SUMS`, a una versión en **borrador** (draft). Revísala y publícala.

## Firma de código (opcional)

Los instaladores se firman cuando existen estos secretos del repositorio; sin ellos la compilación sigue funcionando, sin firmar.

| Plataforma | Secretos |
|------------|----------|
| macOS | `APPLE_CERTIFICATE` (`.p12` en base64), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (específica de la app), `APPLE_TEAM_ID` |
| Windows | `WINDOWS_CERTIFICATE` (`.pfx` en base64), `WINDOWS_CERTIFICATE_PASSWORD` |

Las compilaciones de macOS sin firmar se deben abrir con clic derecho → **Open** la primera vez.

## Actualizaciones dentro de la app

Todavía no están activadas. Para activarlas, genera un par de claves del actualizador con `npx tauri signer generate`, guarda la clave privada
como `TAURI_SIGNING_PRIVATE_KEY` (y su contraseña como `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), añade el plugin actualizador con
la clave pública a `tauri.conf.json` y establece `includeUpdaterJson: true` en el workflow de release.
