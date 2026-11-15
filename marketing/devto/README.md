# dev.to series

Eight tutorials about EmuLabZip, each in English and in Brazilian Portuguese (16 posts). Each solves one real problem, says plainly that
the author built the tool, and ends with a link to the website and to the docs page of its topic, so readers can go on from there.

| # | Topic | Docs pages it links to |
|---|---|---|
| 1 | What EmuLabZip is | getting started |
| 2 | HTTPS proxy, mocks, breakpoints, slow networks | network, mocks, network rules |
| 3 | AI assistants over MCP | MCP, MCP reference |
| 4 | Readable crashes and bug reports | crashes, readable stacks, bug reports |
| 5 | App storage and state | storage, app state |
| 6 | CI sessions, budgets, baselines | CI, budgets, start time |
| 7 | Memory leaks and traces | memory, leaks, traces |
| 8 | Language and text size sweep, visual regression | localization, visual regression, layout |

## Files

- `articles/NN-slug.en.md` and `articles/NN-slug.pt.md`: the posts. Front matter: `title`, `description`, `tags` (at most 4),
  `series`, `cover` (a file of `docs/images`), `publish_after` (a date), `pair` (shared by both languages) and `lang`.
  Images in the text use `docs/images/...`; the publisher turns them into `raw.githubusercontent.com` links at the commit that is
  published, so they never break.
- `publish.mjs`: publishes the posts that are due. No dependencies.
- `../../.github/workflows/devto.yml`: runs it on Tuesdays and Thursdays at 13:00 UTC, and by hand.

## How publishing works

dev.to has no scheduling in its API, so the workflow publishes the next pair (English first, then Portuguese, at most 2 posts a run)
when its `publish_after` date has come. Each post of a pair gets a link to the other. It is safe to run again: a post whose title
already exists on the account is never published twice.

```bash
node marketing/devto/publish.mjs --dry-run   # checks every post, link and image; sends nothing
DEVTO_API_KEY=… node marketing/devto/publish.mjs --max 2   # publishes what is due
```

## Setting it up

1. On dev.to, **Settings → Extensions → DEV Community API Keys**, generate a key.
2. Store it as a repository secret: `gh secret set DEVTO_API_KEY`.
3. Start the workflow once by hand (**Actions → dev.to articles → Run workflow**). The timer takes over.

To pause the series, disable the workflow. To change a post that is already live, edit it on dev.to (the script never edits an
existing post's text, only adds the link to its pair).
