# Security policy

EmuLabZip reads logs, files, databases and process information from the emulators and simulators you connect to it,
so we take security reports seriously.

## Supported versions

Only the latest release receives security fixes while the project is below 1.0.

## Reporting a vulnerability

**Please do not open a public issue.** Use GitHub's private reporting instead:

<https://github.com/LuisPeixoto/EmuLabZip/security/advisories/new>

Include, where possible:

- A description of the problem and its impact
- Steps to reproduce, or a proof of concept
- Affected versions and operating systems

You will get an acknowledgement as soon as the maintainer can review it. We will keep you informed while the issue is
fixed, and credit you in the release notes unless you prefer to stay anonymous.

## What is in scope

- Command injection through device names, package names, file paths or other values that reach `adb` or `simctl`
- Injection or script execution in the desktop interface through log lines, file contents or database values
- Reading or writing files outside the location the user picked when pulling, exporting or saving
- Memory safety or parsing flaws in the ADB protocol client, log parsers or database inspector

## Design notes

- EmuLabZip only talks to the local ADB server (`127.0.0.1:5037` by default) and to the `xcrun simctl` tool.
- Shell arguments sent to devices are quoted; package names and paths are validated before use.
- Data stays on your machine. EmuLabZip has no telemetry and makes no network connections of its own.
- Files are only written to disk after an explicit user action (export, pull, screenshot, heap dump).

## Responsible use

EmuLabZip is a development tool for apps you build or are authorized to test.
