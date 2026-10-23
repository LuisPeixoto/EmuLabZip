## Summary

<!-- What does this change and why? Link the issue it closes, e.g. "Closes #123". -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Refactor / chore / CI

## How was it tested?

<!-- Commands you ran, targets you tried (emulator API level, simulator iOS version), host OS,
     and screenshots for UI changes (dark and light theme). -->

## Checklist

- [ ] `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] `cargo test --workspace --exclude emulabzip-desktop` passes
- [ ] `npm run check` passes in `apps/desktop` (for UI changes)
- [ ] Tests added or updated (new parsers need fixtures with real output)
- [ ] Docs and `CHANGELOG.md` updated when behaviour changes
- [ ] Commits follow [Conventional Commits](https://www.conventionalcommits.org)
