# Translating the documentation

`docs/*.md` (English) is the source. A translation of `docs/<name>.md` is `site/docs/<lang>/<slug>.md`, where `<slug>` is the
file name without the folder (`research/how-the-official-tools-work.md` becomes `how-the-official-tools-work.md`).
`mcp-tools` is generated and stays English only.

## Format

```
---
description: One or two sentences that sum the page up, 110 to 165 characters wide (a CJK character counts as 2).
---
# Translated title

Body…
```

- `description` is what search engines show. Write it as natural prose, not as a cut of the first paragraph.
- Add `title: …` only when the checker says the page title does not fit (30 to 62 wide, with the site suffix added after your h1).
- Do not write `source:`; it is added by `node site/docs-status.mjs <lang> --stamp`.

## What stays exactly as in English

The checker compares your page with the English one and rejects the page when any of these differ.

- **Code blocks**, byte for byte: commands, flags, JSON, comments inside them, everything.
- **Inline code** (`like this`): the same set of spans. Translate the sentence around it, never what is inside the backticks.
- **Keys** (`<kbd>…</kbd>`), **links** (the same targets; the visible text is translated), **images** (same files, translated alt text).
- **Structure**: the same number of headings at the same levels, in the same order; the same lists, list items, tables and
  table rows, and block quotes. Do not merge, split, add or drop any of them. Translate every cell.

## What to translate

All prose: paragraphs, headings, list items, table cells, link text and image alt text.

## Not translated

- Product and tool names: EmuLabZip, Android Studio, Xcode, Instruments, Perfetto, Flutter, React Native, Maestro, GitHub…
- **Names of what the app shows on screen.** The app is in English, so a button or view is written as the app shows it
  (**Take snapshot**, **Network → Rules**, the **Audit** view). You may add a short gloss in the language after it the first time.
- Command names, flags, file names, environment variables, tool and MCP names.
- Numbers, versions, units.

## Style

Clear, direct, second person, the tone of the English. Keep sentences short. Do not add content, examples or warnings that are
not in the English, and do not drop any. Each language has a `GLOSSARY.md` next to its pages: use its terms every time.

## Checking

```
node site/docs-status.mjs <lang> <slug>   # problems of one page
node site/docs-status.mjs <lang>          # every page of the language
```

It is read-only and fast. Fix every line it prints for your pages.
