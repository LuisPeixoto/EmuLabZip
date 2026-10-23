# Contributing to EmuLabZip

Thank you for helping make EmuLabZip better! Contributions of every size are welcome: bug reports, documentation,
design polish, tests and new collectors.

By participating you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- **Report a bug** with the [bug report form](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=bug_report.yml).
- **Suggest a feature** with the [feature request form](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=feature_request.yml).
  The [roadmap](docs/roadmap.md) shows what is already planned.
- **Improve the docs.** Typos, clearer wording and missing examples are all valuable.
- **Pick up an issue** labelled
  [`good first issue`](https://github.com/LuisPeixoto/EmuLabZip/labels/good%20first%20issue) or
  [`help wanted`](https://github.com/LuisPeixoto/EmuLabZip/labels/help%20wanted).

For anything bigger than a small fix, please open an issue first so we can agree on the approach.

## Development setup

You need [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 20 or newer and, on Linux, the
[Tauri system dependencies](https://tauri.app/start/prerequisites/#linux). To try things against a real target you
also need the Android SDK platform tools (`adb`) with an emulator, and/or Xcode with an iOS Simulator on macOS.

```bash
git clone https://github.com/LuisPeixoto/EmuLabZip.git
cd EmuLabZip

# Collectors and CLI
cargo test --workspace --exclude emulabzip-desktop
cargo run -p emulabzip-cli -- devices

# Desktop app (hot reload for the UI)
cd apps/desktop
npm install
npm run tauri dev
```

Tips:

- Set `EMULABZIP_LOG=debug` for verbose logs from the collectors.
- Set `ADB_SERVER_SOCKET=tcp:127.0.0.1:5037` (or another port) to point EmuLabZip at a different ADB server.
- Most parsers are tested against fixtures in `crates/*/tests/fixtures`. When you find output that a parser gets
  wrong, add the raw output as a fixture first.

## Project layout

| Area | Where |
|------|-------|
| Device model, events, sampling buffers | `crates/emulabzip-core/src` |
| ADB host protocol client | `crates/emulabzip-adb/src` |
| Android collectors (logcat, info, perf, memory) | `crates/emulabzip-android/src` |
| iOS Simulator collectors (simctl, log stream) | `crates/emulabzip-apple/src` |
| Files, SQLite, preferences and plists | `crates/emulabzip-storage/src` |
| Debuggable sample app for storage tests | `tools/sample-app` |
| Command line | `crates/emulabzip-cli/src` |
| Desktop commands | `apps/desktop/src-tauri/src/commands.rs` |
| Interface | `apps/desktop/src/lib/components` |

Read [docs/architecture.md](docs/architecture.md) for how the pieces fit together.

## Workflow

1. Fork the repository and create a branch from `main`.
2. Make your change in small, focused commits.
3. Run the checks below.
4. Open a pull request and fill in the template.

### Branch names

Use a short prefix: `feat/…`, `fix/…`, `docs/…`, `refactor/…`, `test/…`, `ci/…`, `chore/…`.

### Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org):

```text
feat(android): sample frame times from gfxinfo
fix(adb): handle a device that disconnects mid-sync
docs: explain how to debug a release build
```

Common types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`. Keep the subject under
72 characters, in the imperative mood. Explain *why* in the body when it is not obvious.

## Checks before opening a pull request

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude emulabzip-desktop
cd apps/desktop && npm run check
```

- New parsers need unit tests with real captured output as fixtures.
- UI changes should include a screenshot in both the dark and the light theme.
- Update the docs and the [changelog](CHANGELOG.md) when behaviour changes.

## Code style

- Rust code is formatted with `rustfmt` (see `rustfmt.toml`) and must be clippy-clean.
- Prefer small modules and plain data types. Comments should explain *why*, not *what*.
- Never build shell commands by concatenating untrusted values. Use the quoting helpers in `emulabzip-adb`.
- Only use public, documented interfaces (`adb`, `dumpsys`, `simctl`, `xctrace`, Perfetto). We do not ship or depend on
  private Apple frameworks or vendor agents.
- The UI is Svelte 5 with TypeScript and Tailwind CSS. Reuse the colour tokens defined in `src/app.css`; do not hard-code colours.

## Security

Please report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md), not in public issues.

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
