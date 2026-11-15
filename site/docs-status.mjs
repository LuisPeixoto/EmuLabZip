#!/usr/bin/env node
// What is left to translate, and whether a translation is sound, without building the site.
//   node site/docs-status.mjs <code>              missing, outdated and unsound pages of a language
//   node site/docs-status.mjs <code> --stamp      write the current English hash into each page's `source:`
//   node site/docs-status.mjs <code> <slug>       the problems of one page
// Exits 1 when there is anything to fix.

import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildDocs, frontMatter } from "./lib/docs.mjs";

const root = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(root, "..");
const site = JSON.parse(readFileSync(join(root, "site.json"), "utf8"));
const [code, arg] = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const stamp = process.argv.includes("--stamp");
if (!code || !site.locales.some((l) => l.code === code)) {
  console.error(`usage: node site/docs-status.mjs <${site.locales.map((l) => l.code).join("|")}> [slug] [--stamp]`);
  process.exit(2);
}
const config = JSON.parse(readFileSync(join(root, "docs.json"), "utf8"));
const entries = config.sections.flatMap((s) => s.pages).filter((p) => !(config.englishOnly ?? []).includes(basename(p)));
const hash12 = (t) => createHash("sha256").update(t).digest("hex").slice(0, 12);

if (stamp) {
  let n = 0;
  for (const entry of entries) {
    const file = join(root, "docs", code, `${basename(entry)}.md`);
    if (!existsSync(file)) continue;
    const { meta, body } = frontMatter(readFileSync(file, "utf8"));
    const source = hash12(readFileSync(join(repoRoot, "docs", `${entry}.md`), "utf8"));
    const head = Object.entries({ source, ...Object.fromEntries(Object.entries(meta).filter(([k]) => k !== "source")) })
      .map(([k, v]) => `${k}: ${/[:#"]/.test(v) && !/^".*"$/.test(v) ? JSON.stringify(v) : v}`)
      .join("\n");
    writeFileSync(file, `---\n${head}\n---\n${body.replace(/^\n+/, "")}`);
    n++;
  }
  console.log(`Stamped ${n} page(s).`);
}

const strings = Object.fromEntries(site.locales.map((l) => [l.code, JSON.parse(readFileSync(join(root, "i18n", `${l.code}.json`), "utf8"))]));
const assetNames = Object.fromEntries(
  (await import("node:fs")).readdirSync(join(root, "assets", "img")).map((n) => [`img/${n}`, `/assets/img/${n}`]),
);
const result = buildDocs({
  root, repoRoot, dist: join(root, "dist"), site, strings, dry: true, only: [code],
  asset: (rel) => `/assets/${rel}`, assetNames, lastModified: () => "2026-01-01", baseScope: () => ({}), loadTemplate: () => "", renderTemplate: () => "", minifyHtml: (h) => h,
});
const mine = result.problems.filter((p) => p.startsWith(`${code}:`) && (!arg || p.includes(`docs/${arg}`) || p.includes(` ${arg} `)));
const done = entries.filter((e) => existsSync(join(root, "docs", code, `${basename(e)}.md`))).length;
console.log(`${code}: ${done} of ${entries.length} pages written`);
for (const p of mine) console.log(`  - ${p.slice(code.length + 2)}`);
process.exit(mine.length ? 1 : 0);
