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
- Mapping files, source maps and dSYMs you add are copied into `mappings/` in EmuLabZip's data folder, and read only
  on your computer. They describe your app's code, so treat them as you would the source.
- A bug report holds a screenshot, device details, log lines, the addresses of failed requests and crash stacks. It is
  written only where you save it (or to the `reports` folder for an assistant). "Open issue" is a link that your
  browser opens: EmuLabZip does not talk to GitHub, GitLab or Jira and stores no token. Everything in the file is
  HTML-escaped and it contains no script. Look at it before you attach it to an issue others can see.
- **Writes.** Most of EmuLabZip reads. These change a device and each needs a confirmation (`--yes`, or a dialog):
  editing rows, preferences and property lists; restoring an app's state; wiping, erasing and deleting emulators and
  simulators; loading or deleting an emulator snapshot. Every file written to an app goes through `Lab`, which checks
  the path (no `..`, no absolute paths), and on iOS through a resolver that refuses a symbolic link out of the container.
  An edit keeps the file it replaces so it can be undone; a restore saves the data it replaces first.
- **State files** (`.ezstate`) hold everything an app stores: tokens, databases, preferences. Treat one like the data it
  came from, and do not attach it to an issue. Opening one checks every tar entry before anything is written: no
  absolute paths, `..`, hard links, devices or symbolic links that leave the app's folder.
- **Device names** given to `avdmanager`, `emulator` and `simctl` are letters, digits, `.`, `_` and `-` only.
- **Audits read what is sensitive, and never repeat it.** The security audit reads the log, preferences, databases and
  recorded requests of an app. Findings carry the first two and last two characters of a secret and its length, never the
  value. The audit changes nothing on the device. Visual baselines are screenshots and can hold anything that was on the
  screen; do not commit one of a screen with personal data.
- **Language, text size and display size** are changed on a device for a test and put back by the sweep, also when it
  fails. Names given to the device (language tags) are letters, digits and hyphens only.
- **Assistants** (MCP) can save states and compare them, start and stop devices, and save emulator snapshots. They
  cannot restore, edit, wipe, delete or load: those need a person.
- React Native state is read through Metro's debugger and can contain whatever the app keeps in memory. Watching Redux
  actions wraps `dispatch` for as long as you watch, and puts it back.
- A session can hold a **screen recording**, touches, and the requests and bodies of a run. A video can show anything
  that was on the screen: messages, email, a password being typed. Sessions and reports are written only where you
  save them (or to EmuLabZip's own folders), and nothing is sent anywhere. Watch a recording before you attach it to an
  issue others can see.
- **Replaying a recording** answers requests from a file on your computer. While it runs, what it cannot answer
  (WebSockets, gRPC, connections it cannot decode) is refused rather than let through, unless you ask for
  `--passthrough`. A HAR file holds what servers sent, tokens included.
- A heap dump holds everything the app had in memory: tokens, personal data, what was on screen. `emulabzip heap analyze`
  reads it on your computer and sends nothing anywhere, but keep the file as private as the app's data.
- The Perfetto trace processor is the one thing EmuLabZip downloads, after you agree: from the official Perfetto
  release, checked against a SHA-256 pinned in the source, and discarded on a mismatch. It runs with the trace you
  open, bound to `127.0.0.1`.
- "Open in Perfetto UI" starts a short-lived server on `127.0.0.1` that answers for one file, only with CORS for
  `https://ui.perfetto.dev`, and stops after the file was fetched or after two minutes. Nothing is uploaded.
- Rules can answer requests with content you wrote and change what an app receives. They are kept in
  `net-rules.json` in EmuLabZip's data folder, and are applied only by a proxy you started. `emulabzip net --rules`
  skips breakpoints, because nobody is there to continue them.
- A request held at a breakpoint is kept in memory, with its headers and body, until you continue it, or for at most
  two minutes. Edits are applied to that one message only.
- Packet captures are parsed with bounds on every length, a size limit (512 MB) and caps on reassembled streams, and
  nothing in them is executed. Captures and sessions can contain secrets: treat them like logs.
- Reading an iOS Simulator's screen uses the macOS Accessibility API, which needs the permission you grant to the program
  that runs EmuLabZip; EmuLabZip only reads the tree and never sends input.
- A session (`.ezsession`) is a zip of JSON, a HAR file and PNGs. Readers refuse names that leave the archive, files
  over 512 MB and formats newer than they know. The MCP tools read only the sessions folder.
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
