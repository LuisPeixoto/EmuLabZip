# CI mode

`emulabzip ci` records a [session](sessions.md) around any command, usually a test run, and then exits with the
command's own exit code, so your pipeline behaves as it did, with a recording to look at when something goes wrong.

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

It starts recording, runs the command (its output goes through unchanged), stops, writes the session, and prints a
one-line result.

| Option | |
|--------|---|
| `-o, --output FILE` | Where to write the session (required) |
| `-a, --app ID` | The app: samples are taken for it and crashes are limited to it |
| `--network` | Record the HTTP(S) traffic: the device is pointed at the proxy for the run |
| `--screens N` | A screenshot every N seconds |
| `--fail-on-crash` | Fail the run (exit 1) when the app crashed while it ran, even if the command passed |
| `--summary FILE` | Also write the report, in Markdown, to this file |

The exit code is the command's. If the command passed but `--fail-on-crash` found a crash, it is 1; if you press Ctrl+C,
130. When the `GITHUB_STEP_SUMMARY` environment variable is set (GitHub Actions), the Markdown report is appended to it
and shows on the run's page without extra steps.

## GitHub Actions

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

Download the artifact and open it in **Sessions → Open…**, or read it with `emulabzip session show`.

You can also gate on accessibility in the same job, after the app is on the screen it should be checked on:

```bash
emulabzip a11y --fail-on error
```

## Notes

- The emulator must be running before `ci` starts; with more than one device, pass `--device`.
- A session file that already exists is never overwritten.
- If `--network` is used, the device must trust the EmuLabZip certificate (see [Network](network.md#the-certificate)).
  On CI run `emulabzip ca install` once, on a rootable image.
