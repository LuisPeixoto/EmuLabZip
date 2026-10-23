# Distribution

Releases are built by GitHub Actions from a version tag.

## Making a release

1. Update the version in `Cargo.toml` (`workspace.package.version`), `apps/desktop/package.json` and
   `apps/desktop/src-tauri/tauri.conf.json`.
2. Add the release to `CHANGELOG.md`.
3. Merge to `main`, then tag and push:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

The [Release workflow](../.github/workflows/release.yml) then:

- builds the desktop installers with `tauri-action` for macOS (Apple silicon and Intel), Linux (`.deb`, `.rpm`,
  `.AppImage`) and Windows (`.msi`, `.exe`);
- builds the `emulabzip` CLI for macOS, Linux (x86_64 musl and arm64) and Windows;
- attaches everything, plus `SHA256SUMS`, to a **draft** release. Review it and publish it.

## Code signing (optional)

Installers are signed when these repository secrets exist; without them the build still succeeds, unsigned.

| Platform | Secrets |
|----------|---------|
| macOS | `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID` |
| Windows | `WINDOWS_CERTIFICATE` (base64 `.pfx`), `WINDOWS_CERTIFICATE_PASSWORD` |

Unsigned macOS builds must be opened with right-click → **Open** the first time.

## In-app updates

Not enabled yet. To turn them on, generate an updater key pair with `npx tauri signer generate`, store the private key
as `TAURI_SIGNING_PRIVATE_KEY` (and its password as `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), add the updater plugin with
the public key to `tauri.conf.json`, and set `includeUpdaterJson: true` in the release workflow.
