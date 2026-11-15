#!/usr/bin/env node
// Publishes the dev.to articles in articles/ that are due. No dependencies (Node 20 or newer).
//   node marketing/devto/publish.mjs --dry-run   check every article, link and image; call nothing that writes
//   node marketing/devto/publish.mjs             publish what is due (needs DEVTO_API_KEY)
//   node marketing/devto/publish.mjs --max 2     publish at most 2 articles in this run (default 2)
// An article is due when its `publish_after` date has come. dev.to has no scheduling in its API, so a workflow runs this on
// a cron. It is safe to run again: an article whose title already exists on the account is never published twice.

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { reflow } from "./reflow.mjs";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "..", "..");
const dryRun = process.argv.includes("--dry-run");
const maxIdx = process.argv.indexOf("--max");
const max = maxIdx > 0 ? Number(process.argv[maxIdx + 1]) : 2;
const key = process.env.DEVTO_API_KEY;
const ref = process.env.DEVTO_ASSET_REF || process.env.GITHUB_SHA || "main";
const API = "https://dev.to/api";
const SITE = "https://emulabzip.luispeixoto.dev";
const RAW = `https://raw.githubusercontent.com/LuisPeixoto/EmuLabZip/${ref}/`;

const problems = [];
const fail = (m) => problems.push(m);

/** `---\nkey: value\n---\nbody` */
function parse(text, file) {
  const m = /^---\n([\s\S]*?)\n---\n/.exec(text);
  if (!m) {
    fail(`${file}: no front matter`);
    return { meta: {}, body: text };
  }
  const meta = {};
  for (const line of m[1].split("\n")) {
    const kv = /^([\w_]+):\s*(.*)$/.exec(line);
    if (kv) meta[kv[1]] = kv[2].trim();
  }
  return { meta, body: text.slice(m[0].length) };
}

const articles = readdirSync(join(here, "articles"))
  .filter((f) => f.endsWith(".md"))
  .sort()
  .map((file) => {
    const { meta, body } = parse(readFileSync(join(here, "articles", file), "utf8"), file);
    return { file, meta, body };
  });

// ---- checks that need no network ---------------------------------------------------------------
const weight = (s) => [...s].length;
for (const a of articles) {
  const at = (m) => fail(`${a.file}: ${m}`);
  for (const k of ["title", "description", "tags", "series", "cover", "publish_after", "pair", "lang"]) if (!a.meta[k]) at(`missing ${k}`);
  if (a.meta.tags && a.meta.tags.split(",").length > 4) at("more than 4 tags");
  if (a.meta.tags && /[^a-z0-9, ]/.test(a.meta.tags)) at("tags may only have lowercase letters and digits");
  if (a.meta.description && (weight(a.meta.description) < 100 || weight(a.meta.description) > 170)) at(`description is ${weight(a.meta.description)} characters`);
  if (a.meta.title && a.meta.title.length > 128) at("title is too long");
  if (!a.file.endsWith(`.${a.meta.lang}.md`)) at("file name does not end with its language");
  if (!a.body.includes(`${SITE}/`)) at("does not link to the site");
  if (!a.body.includes("github.com/LuisPeixoto/EmuLabZip")) at("does not link to the repository");
  if (a.meta.lang === "pt" && /\]\(https:\/\/emulabzip\.luispeixoto\.dev\/docs\//.test(a.body.replace(/\(https:\/\/emulabzip\.luispeixoto\.dev\/docs\/mcp-tools\/\)/g, ""))) at("a Portuguese article links to an English docs page");
  if (a.meta.lang === "en" && /emulabzip\.luispeixoto\.dev\/pt\//.test(a.body)) at("an English article links to a Portuguese page");
  const last = a.body.trimEnd().split("\n").slice(-6).join("\n");
  if (!last.includes(`${SITE}/`)) at("the site link is not at the end of the article");
  if (!/^\d{4}-\d{2}-\d{2}$/.test(a.meta.publish_after || "")) at("publish_after is not a date");
}
const pairs = new Map();
for (const a of articles) pairs.set(a.meta.pair, [...(pairs.get(a.meta.pair) ?? []), a.meta.lang]);
for (const [pair, langs] of pairs) if (langs.sort().join() !== "en,pt") fail(`pair ${pair} has ${langs.join(",") || "nothing"}, not en and pt`);
const titles = articles.map((a) => a.meta.title);
for (const t of new Set(titles.filter((v, i) => titles.indexOf(v) !== i))) fail(`two articles have the title "${t}"`);

/** The line at the end of a post that points to the same post in the other language. */
const pairNote = (lang, other) =>
  `\n\n---\n\n${lang === "pt" ? "Também em inglês" : "Also in Portuguese"}: [${other.title}](${other.url})\n`;

/** The body with images and cover made absolute, so dev.to can show them. */
const absolute = (body) => reflow(body).replace(/\]\((docs\/images\/[^)\s]+)\)/g, (_, p) => `](${RAW}${p})`);
const coverUrl = (a) => `${RAW}${a.meta.cover}`;

// ---- links and images must answer ---------------------------------------------------------------
async function check(url) {
  try {
    const r = await fetch(url, { method: "HEAD", redirect: "follow" });
    return r.status;
  } catch {
    return 0;
  }
}
async function checkLinks(list) {
  const urls = new Map();
  for (const a of list) {
    for (const m of absolute(a.body).matchAll(/\]\((https?:\/\/[^)\s]+)\)/g)) urls.set(m[1], a.file);
    urls.set(coverUrl(a), a.file);
  }
  await Promise.all([...urls].map(async ([url, file]) => {
    const status = await check(url);
    if (status !== 200) fail(`${file}: ${url} answers ${status}`);
  }));
  return urls.size;
}

// ---- dev.to ---------------------------------------------------------------------------------------
async function api(method, path, body, attempt = 0) {
  const r = await fetch(`${API}${path}`, {
    method,
    headers: { "api-key": key, accept: "application/vnd.forem.api-v1+json", "content-type": "application/json", "user-agent": "emulabzip-devto-publisher" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await r.text();
  // dev.to limits how fast articles are made: wait and try again instead of giving up.
  if (r.status === 429 && attempt < 6) {
    const wait = Number(r.headers.get("retry-after")) || 30 * (attempt + 1);
    console.log(`  dev.to asked to wait (${r.status}); trying again in ${wait} s`);
    await new Promise((done) => setTimeout(done, wait * 1000));
    return api(method, path, body, attempt + 1);
  }
  if (!r.ok) throw new Error(`${method} ${path} answered ${r.status}: ${text.slice(0, 300)}`);
  return text ? JSON.parse(text) : null;
}

const today = new Date().toISOString().slice(0, 10);
const due = articles.filter((a) => a.meta.publish_after <= today);

const urlCount = await checkLinks(dryRun ? articles : due);
console.log(`${articles.length} articles, ${due.length} due on ${today}, ${urlCount} links and images checked.`);
if (problems.length) {
  console.error(`\n${problems.length} problem(s):`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}

if (dryRun) {
  for (const a of articles) console.log(`  ${a.meta.publish_after}  ${a.meta.lang}  ${a.meta.title}`);
  console.log("Dry run: nothing was sent.");
  process.exit(0);
}

if (!key) {
  console.error("DEVTO_API_KEY is not set, so nothing can be published.");
  process.exit(1);
}

const mine = await api("GET", "/articles/me/all?per_page=1000");
const byTitle = new Map(mine.map((x) => [x.title, x]));
if (process.argv.includes("--update")) {
  // Re-send the text of posts that are already live (after a fix to how they are made), keeping the link to their pair.
  for (const a of articles) {
    const live = byTitle.get(a.meta.title);
    if (!live) continue;
    const other = articles.find((b) => b.meta.pair === a.meta.pair && b.meta.lang !== a.meta.lang);
    const otherPost = other && byTitle.get(other.meta.title);
    const note = otherPost ? pairNote(a.meta.lang, otherPost) : "";
    await api("PUT", `/articles/${live.id}`, { article: { body_markdown: absolute(a.body.trimEnd()) + note + "\n" } });
    console.log(`Updated ${a.file}`);
  }
  process.exit(0);
}
let made = 0;
// English before Portuguese within a pair, in file order.
for (const a of due) {
  if (made >= max) break;
  if (byTitle.has(a.meta.title)) continue;
  const other = articles.find((b) => b.meta.pair === a.meta.pair && b.meta.lang !== a.meta.lang);
  const otherPost = other && byTitle.get(other.meta.title);
  const note = otherPost ? pairNote(a.meta.lang, otherPost) : "";
  if (made > 0) await new Promise((done) => setTimeout(done, 45_000));
  const created = await api("POST", "/articles", {
    article: {
      title: a.meta.title,
      body_markdown: absolute(a.body.trimEnd()) + note + "\n",
      published: true,
      description: a.meta.description,
      tags: a.meta.tags,
      series: a.meta.series,
      main_image: coverUrl(a),
    },
  });
  byTitle.set(a.meta.title, created);
  made++;
  console.log(`Published ${a.file}: ${created.url}`);
  // The first of the pair gets a link to the second.
  if (otherPost) {
    const src = articles.find((b) => b.meta.title === otherPost.title);
    const full = absolute(src.body.trimEnd());
    const backNote = pairNote(src.meta.lang, created);
    await api("PUT", `/articles/${otherPost.id}`, { article: { body_markdown: full + backNote + "\n" } });
    console.log(`  linked ${src.file} to it`);
  }
}
console.log(made ? `Published ${made}.` : "Nothing to publish.");
