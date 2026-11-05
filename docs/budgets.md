# Budgets and baselines

Write down what a run is allowed to cost, and `emulabzip ci` fails the run when it costs more.

## `emulabzip.toml`

Put it in the project folder (or pass `--config`). Every limit is optional; a run is held only to those written down.

```toml
[budget]
start_time_ms = 800        # the median start time; needs [startup]
memory_peak_mb = 300
cpu_avg_percent = 40
janky_frames_percent = 5
requests = 200
failed_requests = 0
log_errors = 10
crashes = 0

[startup]                  # optional: measure the start time before the command runs
runs = 5
kind = "cold"              # cold, warm or hot

[baseline]
tolerance_percent = 10     # how much worse than the baseline counts as a regression
```

A key that does not exist is an **error**, so a typo does not leave a limit switched off. A limit that could not be
measured counts as broken: `start_time_ms` without a `[startup]` table and `--app` fails, rather than passing silently.

## Budgets in `ci`

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

After the run, the limits are checked and listed in the Markdown report under **Budgets**. If one is broken the exit
code is 1, unless the command itself failed, in which case its own code wins.

## Baselines

Compare a run with an earlier session, usually the last good one:

```bash
emulabzip ci -o run.ezsession --app com.example.app --baseline last-good.ezsession --fail-on-regression -- ./test.sh
emulabzip session compare last-good.ezsession run.ezsession
```

A number is **worse** when it grew by more than the tolerance (10% by default, `--tolerance` on `session compare`). The
report lists each number as better, worse or the same. Without `--fail-on-regression` a regression is only reported.

In GitHub Actions, keep the last good session as a cache or an artifact of the default branch, and download it before the
run:

```yaml
- uses: actions/cache@v4
  with:
    path: baseline.ezsession
    key: baseline-${{ github.base_ref || github.ref_name }}
    restore-keys: baseline-
- run: >
    emulabzip ci -o run.ezsession --app com.example.app
    --baseline baseline.ezsession --fail-on-regression -- ./gradlew connectedCheck
- if: github.ref_name == 'main'
  run: cp run.ezsession baseline.ezsession
```

The start time is stored in the session (`startup.json`), so it can be compared too. Sessions made before this keep
working: the file is optional and the format version is still 1.
