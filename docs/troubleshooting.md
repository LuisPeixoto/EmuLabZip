# Troubleshooting

## My Android emulator does not show up

1. Run `adb devices`. If the emulator is not listed there, EmuLabZip cannot see it either. Restart it, or run
   `adb kill-server && adb start-server`.
2. The device picker footer shows **adb unavailable** when `adb` cannot be found. Install the Android SDK platform
   tools, or point `ANDROID_HOME` or `EMULABZIP_ADB` at your SDK.
3. A device listed as **unauthorized** is waiting for you to accept the USB debugging prompt.
4. A custom ADB server port must be set with `ANDROID_ADB_SERVER_PORT` or `ADB_SERVER_SOCKET` for EmuLabZip too.

## My simulator does not show up

- Only **booted iOS** simulators are listed. Check with `xcrun simctl list devices booted`.
- The footer shows **simctl unavailable** when Xcode's command line tools are missing. Run `xcode-select --install`,
  or `sudo xcode-select -s /Applications/Xcode.app` if several Xcode versions are installed.

## The log is empty

- On Android, check that `adb logcat` prints lines.
- Look at the minimum level and the filters; **Reset filters** appears when lines exist but none match.
- The app filter needs the app to be running. Start the app; the filter picks up the new process id automatically.

## The log is too busy

See [The unified log is busy](ios-simulator.md#the-unified-log-is-busy). On Android, raise the minimum level or filter
by tag.

## Screenshots fail

- On Android, `adb exec-out screencap -p > shot.png` must work. Some secure screens (DRM video, banking apps) are
  black by design.
- On iOS, `xcrun simctl io booted screenshot shot.png` must work.

## Storage says "only debuggable apps can be browsed"

Android only lets `run-as` read the private data of debuggable apps. Install a debug build of your app, or try the
tools with the sample app (`tools/sample-app/install.sh`). Shared storage works for every app.

## Heap dumps fail on Android

`am dumpheap` needs a debuggable app, or an emulator image of the `userdebug`/`eng` type (images without Google Play).
The error shows the reason Android gave, for example `Process not debuggable`.

## Performance shows no frame rate

Frames are only counted while the app draws. Interact with the app; a static screen renders nothing. On the iOS
Simulator, frame metrics arrive with Instruments traces in Phase 4.

## Network: nothing shows up

- Check that the device says *sending traffic here*. On Android, `adb shell settings get global http_proxy` should
  print `127.0.0.1:<port>`.
- Some apps ignore the system proxy (those that open sockets themselves, or use HTTP/3). They cannot be captured.
- Apps that cache heavily may not ask the network at all. Clear their data or use a fresh URL.

## Network: HTTPS requests fail or show as "Connections"

The device does not trust the certificate, or the app pins its server's. Press **Certificate** and follow what it
says. On Android images from Google Play only Chrome and apps that allow user certificates will trust it; see
[the certificate](network.md#the-certificate). A banner names the hosts that are being passed through; use **Decrypt
again** after fixing it.

## Network: the device or the Mac is still pointing at the proxy

Press **Disconnect**, or run `adb shell settings put global http_proxy :0` for Android. For the Mac, open EmuLabZip
once: it restores the proxy settings it saved if an earlier run was killed. Otherwise turn the proxies off in
*System Settings › Network › Details › Proxies*.

## Network: macOS asks for a password

Changing the Mac's proxy is a system setting. EmuLabZip tries as you first and asks for administrator rights only if
macOS refuses.

## Network: a rule does nothing

Rules see decrypted requests, so HTTPS needs the certificate installed, and a host that rejected it is passed through
and cannot be changed. The first enabled rule that matches wins; move the one you expect above the others. A host
pattern matches the host only (`api.example.com`, not `https://api.example.com/`).

## Network: HTTP/3 shows no `h3`

The feature is off by default (**HTTP/3 → Use HTTP/3 to servers**). A host is only tried after one of its responses said
it supports HTTP/3, so the first request is always TCP. A network that blocks UDP makes QUIC fail; the host then stays on
TCP for 10 minutes.

## Layout: nothing to read on iOS

Simulator.app has to be open on that device, and the program running EmuLabZip has to be allowed in System Settings,
Privacy & Security, Accessibility (the app, or your terminal for `emulabzip layout`). Wake the device if it shows a
black screen.

## Layout: Android says uiautomator produced nothing

Some windows forbid it: secure screens, and a few players. Go to another screen and try again. If the device is busy
(a boot animation, an ANR dialog) wait a moment.

## Sessions: a recording is left as a `.partial` folder

The recording was killed before it could pack itself. Nothing is lost: `emulabzip session show run.ezsession.partial`
opens it. Delete the folder, or move it, before you record to the same name again.

## Traces: "Perfetto did not record a trace"

Perfetto needs Android 9 or newer. If the message names a permission, restart the emulator, or run
`adb shell setprop persist.traced.enable 1` and record again. A custom configuration with a syntax error is reported
by Perfetto itself; check it with `adb shell perfetto --txt -c - -o /data/misc/perfetto-traces/test <config.txt`.

## Traces: a CPU profile fails

simpleperf only samples apps that are *debuggable* or *profileable*, and the app must be running. For the whole system
you need an emulator image that allows `adb root` (not Google Play). "No samples were recorded" means the app did
nothing while recording: use it, then record again.

## Traces: Instruments says Developer Mode is off

Run `sudo DevToolsSecurity -enable` in a terminal, enter your password, and record again. `DevToolsSecurity -status`
shows the current state.

## Traces: the download of the trace processor fails

It comes from `github.com/google/perfetto/releases`. Behind a proxy, set `HTTPS_PROXY`. A file that does not match the
pinned SHA-256 is discarded and never run. You can still open any Perfetto trace in the Perfetto UI.

## Crashes: nothing is listed

On Android, the system keeps a crash in DropBox and rate-limits repeats, so a looping app is listed less often than it
crashes. On the iOS Simulator, macOS writes the `.ips` report a few seconds after the crash, into
`~/Library/Logs/DiagnosticReports`; reports of apps that are not in a simulator's folder are not listed. Press
**Refresh**, or run `emulabzip crashes`.

## Controls: the location does nothing on a device

Only emulators accept a new location, through the emulator console. It needs the token in
`~/.emulator_console_auth_token`, which the emulator creates the first time it runs from your account. Physical devices
and simulators of other kinds are not supported for location on Android.

## Controls: a notification does not appear on Android

Notifications are posted from the shell with `cmd notification post` (tried on Android 14). Turn on
notifications for "Android Shell" if the device hides them. The app argument is ignored on Android.

## Plugins: a plugin cannot be run

It has to be enabled first, and editing its `plugin.json` disables it again. A plugin that exits with an error shows the
end of its error output. Check that the program in `command` exists (it is looked up in the plugin's folder, then on the
PATH) and that it is executable (`chmod +x run.sh`).

## MCP: the assistant sees no tools

Run `emulabzip mcp` by hand and send it an `initialize` request, as in [MCP server](mcp.md#trying-it). If the client
cannot find `emulabzip`, give it the full path. Standard output carries only the protocol; look at standard error
(`EMULABZIP_LOG=debug`) for what went wrong.

## Getting diagnostics

Start the app or the CLI with `EMULABZIP_LOG=debug` and include the output when you
[open an issue](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=bug_report.yml).
