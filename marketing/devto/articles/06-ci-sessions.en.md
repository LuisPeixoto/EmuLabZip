---
title: Catch performance regressions in CI with a recorded session
description: Wrap your instrumented tests with emulabzip ci to record logs, requests and crashes, hold the run to budgets and compare it with the last good run in GitHub Actions.
tags: android, ci, testing, performance
series: EmuLabZip
cover: docs/images/sessions.png
publish_after: 2026-12-01
pair: 06-ci-sessions
lang: en
---

A UI test that passes tells you the app did not crash on that run. It does not tell you the start time grew by a second, that
memory peaked 40% higher, or that three requests started failing and the app swallowed the errors. By the time someone notices,
the change that caused it is buried under a dozen commits.

EmuLabZip can record what happened on the device during any command, and hold the run to limits you write down. This post
puts it in a GitHub Actions job.

![A recorded session: a crash, repeated errors and slow requests](docs/images/sessions.png)

## Record a run

`emulabzip ci` records a **session** around any command, usually a test run, and then exits with the command's own exit
code. Your pipeline behaves as it did, and you get a file to look at when something goes wrong:

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

It starts recording, runs the command (its output goes through unchanged), stops, writes the session and prints a one-line
result. A session holds the log, CPU and memory samples, requests (with `--network`), crashes, screenshots every N seconds
(`--screens`), and optionally the screen video and touches. `--fail-on-crash` fails the run when the app crashed while it
ran, even if the command passed.

Read the file later:

```bash
emulabzip session show run.ezsession
emulabzip session show run.ezsession --markdown
emulabzip session timeline run.ezsession
```

`--markdown` is made for a pull request comment, and `emulabzip ci --summary report.md` writes it for you. In GitHub Actions,
if the `GITHUB_STEP_SUMMARY` variable is set, the report is appended to it and shows on the run's page without extra steps.

## Write the limits down

Put an `emulabzip.toml` in the project:

```toml
[budget]
start_time_ms = 800        # the median start time; needs [startup]
memory_peak_mb = 300
cpu_avg_percent = 40
janky_frames_percent = 5
failed_requests = 0
log_errors = 10
crashes = 0

[startup]                  # measure the start time before the command runs
runs = 5
kind = "cold"

[baseline]
tolerance_percent = 10
```

After the run the limits are checked and listed under **Budgets** in the report. If one is broken the exit code is 1, unless
the command itself failed, in which case its own code wins. A key that does not exist is an error, so a typo does not leave a
limit switched off, and a limit that could not be measured counts as broken instead of passing silently.

## Compare with the last good run

A budget catches an absolute problem. A baseline catches "worse than before":

```bash
emulabzip ci -o run.ezsession --app com.example.app \
  --baseline last-good.ezsession --fail-on-regression -- ./test.sh
emulabzip session compare last-good.ezsession run.ezsession
```

A number is worse when it grew by more than the tolerance (10% by default). The report lists each number as better, worse or
the same. Without `--fail-on-regression` a regression is only reported, which is a good way to start.

## The GitHub Actions job

```yaml
name: UI tests
on: [pull_request]

jobs:
  android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17 }
      - name: Install EmuLabZip
        run: cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
      - uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: 34
          arch: x86_64
          script: >
            emulabzip ci -o run.ezsession --app com.example.app --fail-on-crash --screens 10
            -- ./gradlew connectedCheck
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: session
          path: run.ezsession
```

Download the artifact and open it in the desktop app, or read it with `emulabzip session show`. To keep the last good session
for the baseline, store it as a cache or an artifact of your default branch and download it before the run.

Two more checks fit the same job: `emulabzip a11y --fail-on error` gates on accessibility, and `--fail-on-security warning`
audits the app's security and privacy after the command.

## Notes

- The emulator must be running before `ci` starts, and with more than one device you pass `--device`.
- A session file that already exists is never overwritten.
- With `--network` the device must trust EmuLabZip's certificate: on CI, run `emulabzip ca install` once, on a rootable image.

## Try it

- CI mode: [emulabzip.luispeixoto.dev/docs/ci/](https://emulabzip.luispeixoto.dev/docs/ci/)
- Budgets and baselines: [emulabzip.luispeixoto.dev/docs/budgets/](https://emulabzip.luispeixoto.dev/docs/budgets/)
- App start time: [emulabzip.luispeixoto.dev/docs/startup/](https://emulabzip.luispeixoto.dev/docs/startup/)
- Website and all the documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
