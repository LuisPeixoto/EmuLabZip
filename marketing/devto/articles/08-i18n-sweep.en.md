---
title: Test your app in German, Arabic and 200% text, automatically
description: Find text that is cut off, off screen or not mirrored across languages and text sizes with one command, then lock the screen in with visual regression, using EmuLabZip.
tags: android, i18n, testing, accessibility
series: EmuLabZip
cover: docs/images/audit-text.png
publish_after: 2026-12-08
pair: 08-i18n-sweep
lang: en
---

Text is where layouts break. A German word is longer than the English one, Arabic runs right to left, and a person who needs
big text sets it to 200%. Most teams find these bugs from a screenshot in a bug report, because trying every screen in every
language by hand is nobody's idea of a good afternoon.

EmuLabZip can change a device's language and text size, check the screen for text that does not fit, and sweep a screen across
several languages and sizes with one command. This post shows how, and how to keep the result from regressing.

![The Audit view finding a label that is too long for its line](docs/images/audit-text.png)

## Change the device

Pseudo-locales are the cheapest test: `en-XA` is long, accented text and `ar-XB` is right to left. Android changes the language
of an app, not the whole device, so you name the app:

```bash
emulabzip control locale en-XA --app com.example.shop
emulabzip control locale ar-XB --app com.example.shop
emulabzip control reset-locale --app com.example.shop
emulabzip control font-scale 130        # percent of normal; 100 puts it back
```

Open the app in each and see what breaks. It is a good test even before any translation exists, as long as the app's strings
are in resources: hard-coded text stays as it is.

## Check the screen

```bash
emulabzip i18n check
emulabzip i18n check --rtl
```

`i18n check` reads the layout and the screenshot of the screen as it is now, and reports:

| Finding | Meaning |
|---|---|
| `text-off-screen` | Text that reaches past the edge of the screen (unless it is in something that scrolls) |
| `text-clipped-by-parent` | Text that does not fit inside its container, so the container cuts it |
| `text-cut-off` | A single line of text whose ink reaches the end of its box: it is probably cut off or ends in an ellipsis |
| `rtl-not-mirrored` | In a right-to-left language (`--rtl`), most of the screen's text is still on the left |

`text-cut-off` is a heuristic: the layout says where each view is, and the screenshot says whether its text runs to the end of
the box. The message says "may be", and a label that merely hugs its text is not reported.

## Sweep a screen

```bash
emulabzip i18n sweep --app com.example.shop --locales en-XA,ar-XB --scales 100,130,200
```

The sweep tries each language at each text size, reads the layout, and prints a table: how many findings, which ones, and
**which are worse than the device as it is now**. The device is put back (text size and language) when it finishes, and also
when a step fails. `--fail-on-worse` exits with 1 when any setting is worse, which makes it usable in CI. At 200% it also
reports touch targets and focus order that got worse. On the iOS Simulator only text sizes are swept, because an app only takes
a new language when it starts again.

## Make it stick with visual regression

Once a screen looks right, keep a picture of it:

```bash
emulabzip visual capture home --status-bar
emulabzip visual compare home --status-bar
emulabzip visual update home --yes --status-bar
```

`capture` saves the screen as a baseline PNG, `compare` exits with 1 when more than the limit changed and draws the difference
in red, and `update` accepts the new screen. A baseline is never replaced by accident: `capture` refuses a name that exists, and
`update` needs `--yes`. `--status-bar` shows a clean status bar (9:41, full battery) for the shot and puts the real one back, so
the clock does not change every minute. A pixel counts as changed when a channel differs by more than a tolerance (8 of 255 by
default), and anything else that moves, such as an ad or a date, can be left out with `ignore` regions in `emulabzip.toml`.

In CI it is one more flag: `emulabzip ci ... --visual home --fail-on-visual -- ./gradlew connectedCheck`.

## And accessibility, on the same screen

```bash
emulabzip a11y --fail-on error
emulabzip a11y --simulate deuteranopia -o deuteranopia.png
```

`a11y` finds missing labels, small touch targets and low-contrast text, each with the view and the WCAG criterion, and
`--simulate` shows the screen as someone with colour blindness sees it.

## Limits

- The heuristics see views, not glyphs: an icon font, a custom-drawn label or text in a game is not judged.
- Android changes the language per app, not per device, and device-wide languages need root.

## Try it

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Language, text size and text that does not fit: [emulabzip.luispeixoto.dev/docs/localization/](https://emulabzip.luispeixoto.dev/docs/localization/)
- Visual regression: [emulabzip.luispeixoto.dev/docs/visual-regression/](https://emulabzip.luispeixoto.dev/docs/visual-regression/)
- Layout and accessibility: [emulabzip.luispeixoto.dev/docs/layout/](https://emulabzip.luispeixoto.dev/docs/layout/)
- Website and all the documentation: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/)
- Source code (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
