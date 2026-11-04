#!/usr/bin/env node
// Builds the website: one page per language from template.html and i18n/<code>.json, written to site/dist.
//   node site/build.mjs           build
//   node site/build.mjs --check   build, then verify translations, markup and SEO rules (exit 1 on a problem)
// No dependencies: Node 20 or newer.

import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const dist = join(root, "dist");
const site = JSON.parse(readFileSync(join(root, "site.json"), "utf8"));
const checkMode = process.argv.includes("--check");

// ---------------------------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------------------------

const escapeHtml = (v) =>
  String(v).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** Every dotted path of a JSON value, arrays as `list[]` with the paths of their first item. */
function keysOf(value, prefix = "") {
  if (Array.isArray(value)) {
    return [prefix + "[]", ...(value.length ? keysOf(value[0], prefix + "[]") : [])];
  }
  if (value && typeof value === "object") {
    return Object.entries(value).flatMap(([k, v]) => keysOf(v, prefix ? `${prefix}.${k}` : k));
  }
  return [prefix];
}

function lookup(scopes, path) {
  const parts = path.split(".");
  for (const scope of scopes) {
    let cur = scope;
    for (const p of parts) {
      if (cur == null) break;
      cur = cur[p];
    }
    if (cur !== undefined && cur !== null) return cur;
  }
  throw new Error(`missing value for {{${path}}}`);
}

/**
 * A tiny template engine.
 *   {{path}}   the value, HTML-escaped        {{{path}}}   the value as it is
 *   {{#each path}} … {{/each}}   once per item; inside, names are looked up in the item first, then outward.
 *                                {{@index}} is the position from 1, {{this}} the item itself.
 */
function render(template, scopes) {
  let out = template;
  // Loops are resolved from the outermost in; a body is rendered again for each item, which resolves its own loops.
  for (;;) {
    const m = outermostEach(out);
    if (!m) break;
    const list = lookup(scopes, m.path);
    const body = list
      .map((item, i) => {
        const scope = typeof item === "object" ? { ...item, "@index": i + 1 } : { this: item, "@index": i + 1 };
        return render(m.body, [scope, ...scopes]);
      })
      .join("");
    out = out.slice(0, m.start) + body + out.slice(m.end);
  }
  return out
    .replace(/\{\{\{([\w.@]+)\}\}\}/g, (_, p) => String(lookup(scopes, p)))
    .replace(/\{\{([\w.@]+)\}\}/g, (_, p) => escapeHtml(lookup(scopes, p)));
}

/** The first {{#each}} with the {{/each}} that closes it, counting the loops inside. */
function outermostEach(text) {
  const open = /\{\{#each ([\w.]+)\}\}/g;
  const m = open.exec(text);
  if (!m) return null;
  let depth = 1;
  const tokens = /\{\{#each [\w.]+\}\}|\{\{\/each\}\}/g;
  tokens.lastIndex = m.index + m[0].length;
  let t;
  while ((t = tokens.exec(text))) {
    depth += t[0].startsWith("{{#each") ? 1 : -1;
    if (depth === 0) {
      return { path: m[1], start: m.index, end: t.index + t[0].length, body: text.slice(m.index + m[0].length, t.index) };
    }
  }
  throw new Error(`{{#each ${m[1]}}} is never closed`);
}

const hash = (buf) => createHash("sha256").update(buf).digest("hex").slice(0, 10);

function minifyCss(css) {
  return css
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/\s+/g, " ")
    .replace(/\s*([{};,>])\s*/g, "$1")
    .replace(/:\s+/g, ":")
    .replace(/;}/g, "}")
    .trim();
}

function minifyJs(js) {
  // Only whole-line comments and indentation: strings and regular expressions are left alone.
  return js
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("//"))
    .join("\n");
}

function minifyHtml(html) {
  // Keeps <pre>, <script> and <style> as they are.
  const keep = [];
  const masked = html.replace(/<(pre|script|style|textarea)[\s\S]*?<\/\1>/g, (m) => {
    keep.push(m);
    return `\u0000${keep.length - 1}\u0000`;
  });
  return masked
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/>\s+</g, "> <")
    .replace(/\s{2,}/g, " ")
    .replace(/\u0000(\d+)\u0000/g, (_, i) => keep[Number(i)])
    .trim();
}

function walk(dir) {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

// ---------------------------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------------------------

rmSync(dist, { recursive: true, force: true });
mkdirSync(join(dist, "assets"), { recursive: true });

// Assets get a hash in their name, so they can be cached for ever.
const assetNames = {};
for (const file of walk(join(root, "assets"))) {
  const rel = relative(join(root, "assets"), file);
  let body = readFileSync(file);
  if (file.endsWith(".css")) body = Buffer.from(minifyCss(body.toString("utf8")));
  if (file.endsWith(".js")) body = Buffer.from(minifyJs(body.toString("utf8")));
  const dot = rel.lastIndexOf(".");
  const name = `${rel.slice(0, dot)}.${hash(body)}${rel.slice(dot)}`;
  assetNames[rel] = `/assets/${name}`;
  mkdirSync(dirname(join(dist, "assets", name)), { recursive: true });
  writeFileSync(join(dist, "assets", name), body);
}
const asset = (rel) => {
  if (!assetNames[rel]) throw new Error(`no such asset: ${rel}`);
  return assetNames[rel];
};

const template = readFileSync(join(root, "template.html"), "utf8");
const strings = {};
for (const l of site.locales) strings[l.code] = JSON.parse(readFileSync(join(root, "i18n", `${l.code}.json`), "utf8"));

const pageUrl = (l) => `${site.url}/${l.path}`;

function head(l, t) {
  const alternates = site.locales
    .map((o) => `<link rel="alternate" hreflang="${o.hreflang}" href="${pageUrl(o)}">`)
    .concat(`<link rel="alternate" hreflang="x-default" href="${pageUrl(site.locales[0])}">`)
    .join("\n");
  const ogAlt = site.locales
    .filter((o) => o.code !== l.code)
    .map((o) => `<meta property="og:locale:alternate" content="${o.og}">`)
    .join("\n");
  const verify = [
    site.verification.google && `<meta name="google-site-verification" content="${escapeHtml(site.verification.google)}">`,
    site.verification.bing && `<meta name="msvalidate.01" content="${escapeHtml(site.verification.bing)}">`,
  ]
    .filter(Boolean)
    .join("\n");
  return { alternates, ogAlt, verify };
}

function jsonLd(l, t) {
  const image = (n) => `${site.url}${asset(`img/${n}`)}`;
  const graph = [
    {
      "@type": "WebSite",
      "@id": `${site.url}/#website`,
      url: `${site.url}/`,
      name: site.name,
      inLanguage: l.lang,
      publisher: { "@id": `${site.url}/#author` },
    },
    {
      "@type": "SoftwareApplication",
      "@id": `${site.url}/#app`,
      name: site.name,
      url: pageUrl(l),
      description: t.meta.description,
      applicationCategory: "DeveloperApplication",
      operatingSystem: "macOS, Windows, Linux",
      softwareVersion: site.version,
      license: "https://opensource.org/licenses/MIT",
      isAccessibleForFree: true,
      downloadUrl: site.releases,
      codeRepository: site.repo,
      screenshot: [image("network.webp"), image("layout.webp"), image("sessions.webp")],
      offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
      author: { "@id": `${site.url}/#author` },
      inLanguage: l.lang,
    },
    {
      "@type": "Person",
      "@id": `${site.url}/#author`,
      name: site.author.name,
      url: site.author.web,
      sameAs: [site.author.github, site.author.web],
      jobTitle: t.author.role,
    },
    {
      "@type": "FAQPage",
      mainEntity: t.faq.items.map((q) => ({
        "@type": "Question",
        name: q.q,
        acceptedAnswer: { "@type": "Answer", text: q.a },
      })),
    },
  ];
  return JSON.stringify({ "@context": "https://schema.org", "@graph": graph }).replace(/</g, "\\u003c");
}

const pages = [];
for (const l of site.locales) {
  const t = strings[l.code];
  const { alternates, ogAlt, verify } = head(l, t);
  const switcher = site.locales.map((o) => ({
    label: o.label,
    href: `/${o.path}`,
    hreflang: o.hreflang,
    lang: o.lang,
    current: o.code === l.code ? ' aria-current="true"' : "",
  }));
  // What the language hint says, in each language, for a visitor whose browser speaks it.
  const hints = Object.fromEntries(
    site.locales
      .filter((o) => o.code !== l.code)
      .map((o) => [o.code, { text: strings[o.code].hint.text, go: strings[o.code].hint.go, close: strings[o.code].hint.close, href: `/${o.path}` }]),
  );
  const scope = {
    ...t,
    nav: { ...t.nav, current: l.label },
    showcase: {
      ...t.showcase,
      tabs: t.showcase.tabs.map((tab) => ({ ...tab, src: asset(`img/${tab.shot}.webp`), srcSm: asset(`img/${tab.shot}-sm.webp`) })),
    },
    site: { ...site, version: site.version },
    page: {
      lang: l.lang,
      code: l.code,
      canonical: pageUrl(l),
      alternates,
      ogLocale: l.og,
      ogAlt,
      verify,
      jsonld: jsonLd(l, t),
      hints: JSON.stringify(hints),
      og_image: `${site.url}${asset(`img/og-${l.code}.png`)}`,
      css: asset("styles.css"),
      js: asset("main.js"),
      font: asset("fonts/display.woff2"),
      home: `/${l.path}`,
    },
    switcher,
    img: Object.fromEntries(
      readdirSync(join(root, "assets", "img")).map((n) => [n.replace(/[.-]/g, "_"), asset(`img/${n}`)]),
    ),
  };
  const html = minifyHtml(render(template, [scope]));
  const outDir = join(dist, l.path);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, "index.html"), html);
  pages.push({ locale: l, html, file: join(outDir, "index.html") });
}

// 404 (English, not indexed)
const notFound = existsSync(join(root, "404.html")) ? readFileSync(join(root, "404.html"), "utf8") : null;
if (notFound) {
  const html = minifyHtml(render(notFound, [{ ...strings.en, page: { css: asset("styles.css"), home: "/" }, site }]));
  writeFileSync(join(dist, "404.html"), html);
}

// Sitemap with the language alternates of every page
const urls = site.locales
  .map((l) => {
    const alts = site.locales
      .map((o) => `<xhtml:link rel="alternate" hreflang="${o.hreflang}" href="${pageUrl(o)}"/>`)
      .concat(`<xhtml:link rel="alternate" hreflang="x-default" href="${pageUrl(site.locales[0])}"/>`)
      .join("");
    return `<url><loc>${pageUrl(l)}</loc><changefreq>weekly</changefreq><priority>${l.code === "en" ? "1.0" : "0.9"}</priority>${alts}</url>`;
  })
  .join("");
writeFileSync(
  join(dist, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${urls}</urlset>\n`,
);

// Static files (CNAME, robots.txt, manifest, .nojekyll) are copied as they are
if (existsSync(join(root, "static"))) cpSync(join(root, "static"), dist, { recursive: true });

console.log(`Built ${pages.length} pages into ${relative(process.cwd(), dist) || "."}`);

// ---------------------------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------------------------

if (checkMode) {
  const problems = [];
  const fail = (msg) => problems.push(msg);

  // 1. Every language has exactly the keys of English.
  const reference = new Set(keysOf(strings.en));
  for (const l of site.locales.slice(1)) {
    const own = new Set(keysOf(strings[l.code]));
    for (const k of reference) if (!own.has(k)) fail(`${l.code}: missing ${k}`);
    for (const k of own) if (!reference.has(k)) fail(`${l.code}: extra ${k}`);
  }

  for (const { locale: l, html } of pages) {
    const t = strings[l.code];
    const at = (msg) => fail(`${l.code}: ${msg}`);

    // 2. Title and description lengths (characters; CJK counts double in search results).
    const weight = (s) => [...s].reduce((n, c) => n + (/[⺀-鿿＀-￯]/.test(c) ? 2 : 1), 0);
    const title = t.meta.title;
    if (weight(title) < 30 || weight(title) > 62) at(`title is ${weight(title)} wide (30 to 62): ${title}`);
    const description = t.meta.description;
    if (weight(description) < 110 || weight(description) > 165) at(`description is ${weight(description)} wide (110 to 165)`);

    // 3. Markup rules.
    const count = (re) => (html.match(re) || []).length;
    if (count(/<h1[\s>]/g) !== 1) at(`needs exactly one h1, has ${count(/<h1[\s>]/g)}`);
    if (!new RegExp(`<html[^>]*lang="${l.lang}"`).test(html)) at("html lang is wrong");
    if (!html.includes(`<link rel="canonical" href="${pageUrl(l)}">`)) at("canonical is missing");
    for (const o of site.locales) if (!html.includes(`hreflang="${o.hreflang}" href="${pageUrl(o)}"`)) at(`hreflang ${o.hreflang} is missing`);
    if (!html.includes('hreflang="x-default"')) at("x-default is missing");
    for (const img of html.match(/<img\b[^>]*>/g) || []) {
      for (const attr of ["alt", "width", "height", "src"]) if (!new RegExp(`\\s${attr}="`).test(img)) at(`an <img> has no ${attr}: ${img.slice(0, 90)}`);
    }
    const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]);
    for (const id of new Set(ids.filter((v, i) => ids.indexOf(v) !== i))) at(`duplicate id ${id}`);
    for (const a of html.match(/<a\b[^>]*>/g) || []) if (!/\shref="/.test(a)) at(`an <a> has no href: ${a}`);
    if (/\sstyle="/.test(html)) at("inline style attributes are not allowed");
    // (JSON closes objects with "}}", so the structured data and the hint data are left out of this test.)
    if (/\{\{|\}\}/.test(html.replace(/<script[\s\S]*?<\/script>/g, "").replace(/data-hints="[^"]*"/g, ""))) at("an unresolved placeholder is left");
    // Targets of #links exist.
    for (const m of html.matchAll(/href="#([^"]+)"/g)) if (!ids.includes(m[1])) at(`link to #${m[1]} has no target`);
    // 4. JSON-LD parses.
    const ld = html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/);
    if (!ld) at("JSON-LD is missing");
    else
      try {
        JSON.parse(ld[1]);
      } catch (e) {
        at(`JSON-LD is not valid: ${e.message}`);
      }
  }

  // 5. Files that have to exist.
  for (const f of ["sitemap.xml", "robots.txt", "CNAME", ".nojekyll", "404.html"]) if (!existsSync(join(dist, f))) fail(`dist/${f} is missing`);
  const robots = existsSync(join(dist, "robots.txt")) ? readFileSync(join(dist, "robots.txt"), "utf8") : "";
  if (!robots.includes(`${site.url}/sitemap.xml`)) fail("robots.txt does not point to the sitemap");

  if (problems.length) {
    console.error(`\n${problems.length} problem(s):`);
    for (const p of problems) console.error(`  - ${p}`);
    process.exit(1);
  }
  console.log("All checks passed.");
}
