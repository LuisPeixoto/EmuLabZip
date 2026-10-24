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

- Exposure or mishandling of the certificate authority's private key
- The proxy accepting connections from anything but the local machine
- A device or the Mac's proxy settings left pointing at EmuLabZip after it stops
- Verification of upstream certificates being skipped without the user asking for it

- Command injection through device names, package names, file paths or other values that reach `adb` or `simctl`
- Injection or script execution in the desktop interface through log lines, file contents or database values
- Reading or writing files outside the location the user picked when pulling, exporting or saving
- Memory safety or parsing flaws in the ADB protocol client, log parsers or database inspector

## Design notes

- EmuLabZip talks to the local ADB server (`127.0.0.1:5037` by default) and to the `xcrun simctl` tool. The network
  tool also runs a proxy, described below.
- The proxy listens on `127.0.0.1` only. Devices reach it through `adb reverse`, never over the network.
- The certificate authority is created on first use. Its private key is stored with owner-only permissions (Unix)
  next to the certificate, and never leaves the computer. Servers' certificates are verified against the system and
  built-in roots, as a browser would.
- Everything done to a device is undone: the Android proxy setting and `adb reverse` forward when you disconnect or
  quit, and the Mac's proxy settings, which are saved first and restored exactly, also at the next start if EmuLabZip
  was killed.
- Shell arguments sent to devices are quoted; package names and paths are validated before use.
- Data stays on your machine. EmuLabZip has no telemetry and makes no network connections of its own, except the
  one-time, opt-in download of the trace processor.
- Files are only written to disk after an explicit user action (export, pull, screenshot, heap dump, recording).
- The Perfetto trace processor is the one thing EmuLabZip downloads, after you agree: from the official Perfetto
  release, checked against a SHA-256 pinned in the source, and discarded on a mismatch. It runs with the trace you
  open, bound to `127.0.0.1`.
- "Open in Perfetto UI" starts a short-lived server on `127.0.0.1` that answers for one file, only with CORS for
  `https://ui.perfetto.dev`, and stops after the file was fetched or after two minutes. Nothing is uploaded.
- The MCP server (`emulabzip mcp`) speaks only on standard input and output, with no network port. Its tools that act
  on a device (`control_*`, `open_url`, `record_trace`) are always on, and are checked like the app's own: values are
  validated, links to `file:`, `javascript:` and similar are refused, and nothing reaches a shell unquoted. It will not
  run a plugin the user has not enabled, and it never downloads anything.
- A plugin is a program that runs as the user. It never runs until it is enabled; enabling shows the exact command, and
  the approval is tied to the contents of `plugin.json`, so editing the file withdraws it. Plugins are started without a
  shell, with a timeout and an output limit, and killed when stopped. EmuLabZip does not download plugins.
- Recordings leave nothing on the device: configurations, raw files and reports are removed when a recording ends.

## Responsible use

EmuLabZip is a development tool for apps you build or are authorized to test.
