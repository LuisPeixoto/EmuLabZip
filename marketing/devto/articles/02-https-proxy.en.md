---
title: See, mock and slow down your app's HTTPS traffic on the Android emulator
description: Capture decrypted HTTPS from an emulator, answer requests with mocks, hold them at breakpoints and test Slow 3G, with a free open-source proxy built into EmuLabZip.
tags: android, networking, testing, opensource
series: EmuLabZip
cover: docs/images/network.png
publish_after: 2026-11-17
pair: 02-https-proxy
lang: en
---

"It works on my machine" usually means "the backend answered quickly with a nice 200". To see how your app behaves when the
API is slow, returns a 500 or an empty list, you normally change the server, or set up a proxy, install a certificate and
click through its rules UI.

EmuLabZip has a debugging proxy built in. This post walks through using it on an Android emulator, from capturing HTTPS to
mocking responses and testing a slow network, without touching your backend.

![Live requests of an Android emulator in EmuLabZip's Network view](docs/images/network.png)

## 1. Capture HTTPS

Start an emulator, then make it trust EmuLabZip's certificate authority once:

```bash
emulabzip ca install
emulabzip net --har capture.har
```

`emulabzip net` runs the proxy on your computer, points the device at it, prints one line per request and, when you press
`Ctrl+C`, restores the device's proxy setting. `--har` also writes the traffic to a HAR file you can open elsewhere. With
`--json` every request is a line of JSON, so you can filter:

```bash
emulabzip net --json | jq 'select(.status >= 400)'
```

The desktop app does the same in the **Network** view: every request with its headers, decoded bodies and timing, and which
HTTP version it used.

### Which apps trust the certificate

How far the trust goes depends on the device:

| Device | Result |
|---|---|
| iOS Simulator | All apps trust it (restart apps that were already running) |
| Android emulator whose image allows `adb root` (Google APIs, AOSP) | Added to the system store: every app trusts it until the device restarts |
| Android image from Google Play, or a physical device | Installed as a user certificate: Chrome and apps that opt in |

Since Android 7 apps ignore user certificates unless they ask for them. For your own **debug** build, add a network security
config that trusts them. Release builds are not affected, because `debug-overrides` only applies to debuggable apps:

```xml
<network-security-config>
    <debug-overrides>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </debug-overrides>
</network-security-config>
```

Apps that pin their certificate are passed through unchanged, and show up as **Connections** because EmuLabZip cannot read
them.

## 2. Answer requests yourself

Rules live in the proxy. A rule has a matcher (which requests) and one action. The first enabled rule that matches wins.
Save this as `rules.json`:

```json
[
  {
    "name": "stub the profile",
    "matcher": { "host": "api.example.com", "path": "/v1/profile" },
    "action": { "type": "mock", "status": 200,
                "headers": [["content-type", "application/json"]],
                "body": "{\"name\": \"Ada\"}" }
  },
  { "name": "slow uploads", "matcher": { "methods": ["POST"] }, "action": { "type": "delay", "ms": 3000 } },
  { "name": "no analytics", "matcher": { "host": "*.analytics.example.com" }, "action": { "type": "block" } }
]
```

```bash
emulabzip net --rules rules.json
```

Now the profile request never reaches the server, every `POST` waits three seconds, and analytics calls fail like a network
error. Flows a rule touched carry a **MOCK** or **RULE** badge in the list. In the app you can also select a captured
request and press **Mock** to get a rule that answers the way the server did, ready to edit.

There are more actions: answering with a local file, forwarding to another server (to point the app at your laptop), checking
responses against an OpenAPI file, and sandboxed WebAssembly scripts. They are covered in the
[network rules guide](https://emulabzip.luispeixoto.dev/docs/network-rules/).

## 3. Hold a request at a breakpoint

A rule can also **hold** a request, its response, or both. The app shows it in a drawer at the bottom of **Network**, where you
can edit the headers, the body (when it is text) and the status of a response. **Continue** sends it on with your edits,
**Abort** fails it. If nobody answers, a held message goes on unchanged after two minutes, so your app never hangs for good.

## 4. Try a bad network

The proxy sits in the middle of everything the device sends, so it can shape the link. The presets are Fast 3G, Slow 3G, Slow
Wi-Fi, High latency, Packet loss and Offline:

```bash
emulabzip net --conditions slow-3g
emulabzip net --down 400 --latency 100
emulabzip net --conditions offline
```

The change applies at once, also to connections that are already open. One caveat: HTTP/3 uses QUIC over UDP, which does not
go through this link, so switch HTTP/3 off to see an app slowed down.

## Limits worth knowing

- A mock body can be up to 5 MB, and a delay up to 60 seconds.
- Hosts that are passed through (pinned apps) cannot be changed.
- For WebSocket upgrades only the handshake can be mocked or blocked.

## Try it

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Network guide: [emulabzip.luispeixoto.dev/docs/network/](https://emulabzip.luispeixoto.dev/docs/network/)
- Mocks and breakpoints: [emulabzip.luispeixoto.dev/docs/mocks/](https://emulabzip.luispeixoto.dev/docs/mocks/)
- Website and all the documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
