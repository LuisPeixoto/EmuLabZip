// Builds the documentation pages: every docs/*.md (English, the source of truth) and its translations in site/docs/<code>/,
// rendered with the site's chrome, a sidebar, a table of contents, search data and structured data.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join, posix } from "node:path";
import { plain, render as markdown } from "./markdown.mjs";

const escapeHtml = (v) => String(v).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** Width as search results count it: CJK characters take two. */
export const weight = (s) => [...s].reduce((n, c) => n + (/[⺀-鿿＀-￯]/.test(c) ? 2 : 1), 0);

const hash12 = (text) => createHash("sha256").update(text).digest("hex").slice(0, 12);

/** `---\nkey: value\n---\nbody` → { meta, body }. */
export function frontMatter(text) {
  const m = /^---\n([\s\S]*?)\n---\n?/.exec(text);
  if (!m) return { meta: {}, body: text };
  const meta = {};
  for (const line of m[1].split("\n")) {
    const kv = /^([\w-]+):\s*(.*)$/.exec(line);
    if (kv) meta[kv[1]] = kv[2].replace(/^"(.*)"$/, "$1");
  }
  return { meta, body: text.slice(m[0].length) };
}

/** The first of `candidates` whose width is in [min, max]. */
const fit = (candidates, min, max) => candidates.find((c) => weight(c) >= min && weight(c) <= max);

/** `text` cut at a word (or character, for CJK) so it is at most `max` wide, with an ellipsis when cut. */
function cut(text, max) {
  if (weight(text) <= max) return text;
  let out = "";
  for (const word of text.split(/(?<=\s)|(?=[⺀-鿿＀-￯])/)) {
    if (weight(out + word) > max - 1) break;
    out += word;
  }
  return out.trimEnd().replace(/[,;:.\s–-]+$/, "") + "…";
}

function webpSize(buf) {
  const kind = buf.toString("ascii", 12, 16);
  if (kind === "VP8 ") return { width: buf.readUInt16LE(26) & 0x3fff, height: buf.readUInt16LE(28) & 0x3fff };
  if (kind === "VP8L") {
    const b = buf.readUInt32LE(21);
    return { width: (b & 0x3fff) + 1, height: ((b >> 14) & 0x3fff) + 1 };
  }
  if (kind === "VP8X") return { width: buf.readUIntLE(24, 3) + 1, height: buf.readUIntLE(27, 3) + 1 };
  return null;
}

export const docsBase = (l) => `/${l.path}docs/`;

/**
 * env: { root, repoRoot, dist, site, strings, asset, assetNames, renderTemplate, minifyHtml, lastModified, baseScope, pageUrl }
 * Returns { pages: [{ loc, lastmod, alternates, file, html, locale, slug }], problems }.
 */
export function buildDocs(env) {
  const { root, repoRoot, dist, site, strings, asset, assetNames, renderTemplate, minifyHtml, lastModified, baseScope, loadTemplate } = env;
  const config = JSON.parse(readFileSync(join(root, "docs.json"), "utf8"));
  const problems = [];
  const entries = config.sections.flatMap((s) => s.pages.map((entry) => ({ entry, slug: basename(entry), section: s.id })));
  const englishOnly = new Set(config.englishOnly ?? []);
  const locales = site.locales.filter((l) => (env.only ? env.only.includes(l.code) || l.code === site.locales[0].code : l.docs));
  const template = env.dry ? "" : loadTemplate("docs.html");

  // Every Markdown file in docs/ is listed once, and nothing listed is missing.
  const listed = new Set(entries.map((e) => e.entry));
  if (listed.size !== entries.length) problems.push("docs.json lists a page twice");
  for (const e of entries) if (!existsSync(join(repoRoot, "docs", `${e.entry}.md`))) problems.push(`docs.json lists ${e.entry}, which does not exist`);
  for (const f of walkMd(join(repoRoot, "docs"))) if (!listed.has(f)) problems.push(`docs/${f}.md is not in docs.json`);
  for (const [i, slug] of config.features.entries()) if (!entries.some((e) => e.slug === slug)) problems.push(`docs.json feature ${i} points at ${slug}, which is not a page`);
  for (const [tab, slug] of Object.entries(config.tabs)) if (!entries.some((e) => e.slug === slug)) problems.push(`docs.json tab ${tab} points at ${slug}, which is not a page`);

  // ---- sources ---------------------------------------------------------------------------------
  const sources = {}; // code → slug → { text, meta, rel, abs, enText }
  for (const l of locales) {
    sources[l.code] = {};
    for (const e of entries) {
      const enAbs = join(repoRoot, "docs", `${e.entry}.md`);
      const enText = readFileSync(enAbs, "utf8");
      if (l.code === site.locales[0].code) {
        sources[l.code][e.slug] = { text: enText, meta: {}, rel: `docs/${e.entry}.md`, abs: enAbs, enText };
        continue;
      }
      if (englishOnly.has(e.slug)) continue;
      const abs = join(root, "docs", l.code, `${e.slug}.md`);
      if (!existsSync(abs)) {
        problems.push(`${l.code}: docs/${e.slug} is not translated (site/docs/${l.code}/${e.slug}.md)`);
        continue;
      }
      const { meta, body } = frontMatter(readFileSync(abs, "utf8"));
      if (meta.source !== hash12(enText)) problems.push(`${l.code}: docs/${e.slug} is out of date: its source is ${meta.source}, the English page is ${hash12(enText)}`);
      sources[l.code][e.slug] = { text: body, meta, rel: `site/docs/${l.code}/${e.slug}.md`, abs, enText };
    }
  }

  // ---- first pass: headings, so links to anchors can be checked and carried across languages --------
  const heads = {}; // code → slug → headings
  for (const l of locales) {
    heads[l.code] = {};
    for (const e of entries) {
      const src = sources[l.code][e.slug];
      if (src) heads[l.code][e.slug] = markdown(src.text, { link: (h) => h, image: () => null }).headings;
    }
  }
  const en = site.locales[0].code;
  /** The id of an English heading in `code`'s version of the page. */
  const anchorMaps = {};
  for (const l of locales) {
    anchorMaps[l.code] = {};
    if (l.code === en) continue;
    for (const e of entries) {
      const own = heads[l.code][e.slug];
      const ref = heads[en][e.slug];
      if (!own) continue;
      if (own.length !== ref.length || own.some((h, i) => h.level !== ref[i].level)) {
        problems.push(`${l.code}: docs/${e.slug} has a different outline from the English page (${own.length} headings, English has ${ref.length}; levels must match)`);
        continue;
      }
      anchorMaps[l.code][e.slug] = new Map(ref.map((h, i) => [h.id, own[i].id]));
    }
  }
  const anchorIn = (l, slug, id, from) => {
    const map = anchorMaps[l.code]?.[slug];
    const mapped = l.code === en ? id : map?.get(id);
    if (!mapped || !heads[l.code][slug]?.some((h) => h.id === mapped)) {
      if (heads[l.code][slug]) problems.push(`${l.code}: ${from} links to ${slug}#${id}, which is not a heading`);
      return id;
    }
    return mapped;
  };

  // ---- second pass: render ---------------------------------------------------------------------
  const pages = [];
  const searchData = {};
  const enRendered = {};
  const imageCache = new Map();
  const imageFor = (name) => {
    if (imageCache.has(name)) return imageCache.get(name);
    const main = `img/${name}.webp`;
    let made = null;
    if (assetNames[main]) {
      const size = webpSize(readFileSync(join(root, "assets", main)));
      const sm = `img/${name}-sm.webp`;
      made = { src: asset(main), srcset: assetNames[sm] ? `${asset(sm)} 800w, ${asset(main)} ${size.width}w` : null, width: size.width, height: size.height };
    }
    imageCache.set(name, made);
    return made;
  };

  for (const l of locales) {
    const t = strings[l.code];
    const base = docsBase(l);
    const rendered = {};
    for (const e of entries) {
      const src = sources[l.code][e.slug];
      if (!src) continue;
      const from = `${e.slug}`;
      const result = markdown(src.text, {
        link(href) {
          if (/^(https?:|mailto:)/.test(href)) return href;
          const [pathPart, anchor = ""] = href.split("#");
          if (!pathPart) return `#${anchorIn(l, e.slug, anchor, from)}`;
          const resolved = posix.normalize(posix.join("docs", posix.dirname(e.entry), pathPart));
          if (resolved.startsWith("docs/") && resolved.endsWith(".md")) {
            const target = entries.find((x) => `docs/${x.entry}.md` === resolved);
            if (!target) {
              problems.push(`${l.code}: ${from} links to ${href}, which is not a page`);
              return href;
            }
            if (l.code !== en && englishOnly.has(target.slug)) return `${docsBase(site.locales[0])}${target.slug}/${anchor ? `#${anchorIn(site.locales[0], target.slug, anchor, from)}` : ""}`;
            return `${base}${target.slug}/${anchor ? `#${anchorIn(l, target.slug, anchor, from)}` : ""}`;
          }
          if (resolved.startsWith("..")) {
            problems.push(`${l.code}: ${from} links outside the repository: ${href}`);
            return href;
          }
          return `${site.repo}/blob/main/${resolved}${anchor ? `#${anchor}` : ""}`;
        },
        image(srcPath, alt) {
          const name = basename(srcPath).replace(/\.\w+$/, "");
          const made = imageFor(name);
          if (!made) problems.push(`${l.code}: ${from} uses ${srcPath}, which has no webp in site/assets/img`);
          return made;
        },
      });
      rendered[e.slug] = { ...result, e, src };
    }

    // A translation keeps the English page's structure: the same code, links, images, rows and items.
    if (l.code !== en) for (const r of Object.values(rendered)) for (const p of sameStructure(enRendered[r.e.slug], r, l)) problems.push(`${l.code}: docs/${r.e.slug}: ${p}`);
    if (l.code === en) Object.assign(enRendered, rendered);

    // Titles and descriptions: from the page, with front matter or docs.json to override.
    const sectionTitle = (id) => t.docs.sections[id];
    for (const r of Object.values(rendered)) {
      const override = l.code === en ? config.overrides[r.e.slug] ?? {} : r.src.meta;
      const h1 = r.title;
      const cands = [...t.docs.titleSuffixes.map((s) => `${h1} – ${s}`), h1, `${h1} – ${sectionTitle(r.e.section)} – EmuLabZip`];
      r.metaTitle = override.title ?? fit(cands, 30, 62);
      if (!r.metaTitle) {
        problems.push(`${l.code}: docs/${r.e.slug} has no title between 30 and 62 wide ("${h1}"): set "title" in ${l.code === en ? "docs.json overrides" : "its front matter"}`);
        r.metaTitle = h1;
      }
      let desc = "";
      for (const p of r.paragraphs) {
        desc = desc ? `${desc} ${p}` : p;
        if (weight(desc) >= 120) break;
      }
      r.description = override.description ?? cut(desc, 160);
      if (weight(r.description) < 110 || weight(r.description) > 165) {
        problems.push(`${l.code}: docs/${r.e.slug} has a description ${weight(r.description)} wide (110 to 165): set "description"`);
      }
      r.lastmod = lastModified([r.src.abs]);
      if (!r.title) problems.push(`${l.code}: docs/${r.e.slug} has no h1`);
    }

    if (env.dry) continue;
    const order = entries.filter((e) => rendered[e.slug]);
    const navModel = config.sections.map((s) => ({
      id: s.id,
      title: sectionTitle(s.id),
      text: t.docs.sectionText[s.id],
      pages: s.pages
        .map((p) => basename(p))
        .filter((slug) => rendered[slug] || (l.code !== en && englishOnly.has(slug) && enRendered[slug]))
        .map((slug) =>
          rendered[slug]
            ? { slug, title: rendered[slug].title, href: `${base}${slug}/`, summary: rendered[slug].description }
            : { slug, title: enRendered[slug].title, href: `${docsBase(site.locales[0])}${slug}/`, summary: enRendered[slug].description },
        ),
    }));

    const switcherFor = (slug) =>
      site.locales.map((o) => ({
        label: o.label,
        href: o.docs && (slug === null || sources[o.code][slug]) ? `${docsBase(o)}${slug ? `${slug}/` : ""}` : `/${o.path}`,
        hreflang: o.hreflang,
        lang: o.lang,
        current: o.code === l.code ? ' aria-current="true"' : "",
      }));

    const alternatesFor = (slug) => {
      const have = locales.filter((o) => (slug === null ? true : sources[o.code][slug]));
      return {
        list: have.map((o) => ({ hreflang: o.hreflang, href: `${site.url}${docsBase(o)}${slug ? `${slug}/` : ""}` })),
        xdefault: `${site.url}${docsBase(site.locales[0])}${slug ? `${slug}/` : ""}`,
      };
    };

    const emit = ({ slug, path, title, description, body, toc, current, h1, kind, crumbs, prev, next, updated, edit }) => {
      const url = `${site.url}${path}`;
      const alts = alternatesFor(slug);
      const alternates = alts.list
        .map((a) => `<link rel="alternate" hreflang="${a.hreflang}" href="${a.href}">`)
        .concat(`<link rel="alternate" hreflang="x-default" href="${alts.xdefault}">`)
        .join("\n");
      const ogAlt = site.locales
        .filter((o) => o.code !== l.code)
        .map((o) => `<meta property="og:locale:alternate" content="${o.og}">`)
        .join("\n");
      const graph = [
        { "@type": kind === "article" ? "TechArticle" : "CollectionPage", "@id": `${url}#page`, headline: h1, description, url, inLanguage: l.lang, dateModified: updated, isPartOf: { "@id": `${site.url}/#website` }, author: { "@id": `${site.url}/#author` }, publisher: { "@id": `${site.url}/#author` }, image: `${site.url}${asset(`img/og-${l.code}.png`)}` },
        { "@type": "BreadcrumbList", itemListElement: crumbs.map((c, i) => ({ "@type": "ListItem", position: i + 1, name: c.name, item: `${site.url}${c.href}` })) },
        { "@type": "Person", "@id": `${site.url}/#author`, name: site.author.name, url: site.author.github },
        { "@type": "WebSite", "@id": `${site.url}/#website`, url: `${site.url}/`, name: site.name },
      ];
      const scope = {
        ...baseScope(l),
        meta: { title, description, ogAlt: t.meta.ogAlt },
        switcher: switcherFor(slug ?? null),
        page: {
          ...baseScope(l).page,
          canonical: url,
          alternates,
          ogAlt,
          jsonld: JSON.stringify({ "@context": "https://schema.org", "@graph": graph }).replace(/</g, "\\u003c"),
          docsJs: asset("docs.js"),
          searchIndex: `${base}search.json`,
          h1,
          body,
          updated,
          edit,
        },
        sidebar: navModel.map((s) => ({ ...s, pages: s.pages.map((p) => ({ ...p, current: p.slug === current ? ' aria-current="page"' : "" })) })),
        toc,
        hasToc: toc.length ? [{}] : [],
        crumbs,
        prev: prev ? [prev] : [],
        next: next ? [next] : [],
        isArticle: kind === "article" ? [{}] : [],
        isIndex: kind === "article" ? [] : [{}],
        sections: navModel,
      };
      const html = minifyHtml(renderTemplate(template, [scope]));
      const file = join(dist, path.slice(1), "index.html");
      mkdirSync(join(dist, path.slice(1)), { recursive: true });
      writeFileSync(file, html);
      pages.push({ loc: url, lastmod: updated, alternates: alts, file, html, locale: l, slug: slug ?? "", title, description, h1 });
    };

    // The index
    const indexLastmod = lastModified([join(root, "docs.json"), join(root, "docs.html"), join(root, "i18n", `${l.code}.json`)]);
    emit({
      slug: null,
      path: base,
      title: t.docs.indexTitle,
      description: t.docs.indexDescription,
      body: "",
      toc: [],
      current: "",
      h1: t.docs.title,
      kind: "index",
      crumbs: [{ name: "EmuLabZip", href: `/${l.path}` }, { name: t.docs.title, href: base }],
      updated: indexLastmod,
      edit: `${site.repo}/tree/main/docs`,
    });

    // The pages
    order.forEach((e, i) => {
      const r = rendered[e.slug];
      const prevE = order[i - 1];
      const nextE = order[i + 1];
      const link = (x) => (x ? { title: rendered[x.slug].title, href: `${base}${x.slug}/` } : null);
      emit({
        slug: e.slug,
        path: `${base}${e.slug}/`,
        title: r.metaTitle,
        description: r.description,
        body: r.html,
        toc: r.headings.filter((h) => h.level === 2 || h.level === 3),
        current: e.slug,
        h1: r.title,
        kind: "article",
        crumbs: [{ name: "EmuLabZip", href: `/${l.path}` }, { name: t.docs.title, href: base }, { name: sectionTitle(e.section), href: `${base}#${e.section}` }, { name: r.title, href: `${base}${e.slug}/` }],
        prev: link(prevE),
        next: link(nextE),
        updated: r.lastmod,
        edit: `${site.repo}/edit/main/${r.src.rel}`,
      });
    });

    // Search data: a title, its headings and the start of the text, per page.
    searchData[l.code] = order.map((e) => {
      const r = rendered[e.slug];
      const text = r.html
        .replace(/<pre[\s\S]*?<\/pre>/g, " ")
        .replace(/<[^>]+>/g, " ")
        .replace(/&nbsp;/g, " ")
        .replace(/&lt;/g, "<")
        .replace(/&gt;/g, ">")
        .replace(/&quot;/g, '"')
        .replace(/&amp;/g, "&")
        .replace(/\s+/g, " ")
        .trim();
      return { t: r.title, u: `${base}${e.slug}/`, s: sectionTitle(e.section), h: r.headings.filter((h) => h.level > 1).map((h) => h.text), x: text.slice(0, 1400) };
    });
    mkdirSync(join(dist, base.slice(1)), { recursive: true });
    writeFileSync(join(dist, base.slice(1), "search.json"), JSON.stringify(searchData[l.code]));
  }

  return { pages, problems, config, entries, locales };
}

/** What a translation must keep from the English page. Returns the differences. */
function sameStructure(en, tr, l) {
  const out = [];
  const blocks = (h) => [...h.matchAll(/<pre class="code"[^>]*>[\s\S]*?<\/pre>/g)].map((m) => m[0]);
  const a = blocks(en.html);
  const b = blocks(tr.html);
  if (a.length !== b.length) out.push(`has ${b.length} code blocks, English has ${a.length}`);
  else a.forEach((x, i) => x !== b[i] && out.push(`code block ${i + 1} was changed: code stays exactly as in English`));
  const prose = (h) => h.replace(/<pre[\s\S]*?<\/pre>/g, "");
  const multiset = (h, re) => [...prose(h).matchAll(re)].map((m) => m[1]).sort();
  for (const [name, re] of [["inline code", /<code>([\s\S]*?)<\/code>/g], ["key", /<kbd>([\s\S]*?)<\/kbd>/g]]) {
    const x = multiset(en.html, re);
    const y = multiset(tr.html, re);
    if (x.join("\u0000") !== y.join("\u0000")) {
      const missing = x.filter((v) => !y.includes(v)).slice(0, 3);
      const extra = y.filter((v) => !x.includes(v)).slice(0, 3);
      out.push(`${name} differs (${missing.length ? `missing ${JSON.stringify(missing)}` : ""}${extra.length ? ` extra ${JSON.stringify(extra)}` : ""}${!missing.length && !extra.length ? "counts differ" : ""})`);
    }
  }
  const count = (h, re) => (h.match(re) || []).length;
  for (const [name, re] of [["list items", /<li>/g], ["table rows", /<tr>/g], ["tables", /<table>/g], ["images", /<img /g], ["block quotes", /<blockquote>/g], ["lists", /<(ul|ol)[ >]/g]]) {
    if (count(en.html, re) !== count(tr.html, re)) out.push(`has ${count(tr.html, re)} ${name}, English has ${count(en.html, re)}`);
  }
  const hrefs = (h) => [...prose(h).matchAll(/<a href="([^"#]*)[^"]*"/g)].map((m) => m[1].replace(/^\/(?:[a-z]{2}\/)?docs\//, "/docs/")).sort();
  const x = hrefs(en.html);
  const y = hrefs(tr.html);
  if (x.join("\n") !== y.join("\n")) {
    const missing = x.filter((v) => !y.includes(v)).slice(0, 3);
    const extra = y.filter((v) => !x.includes(v)).slice(0, 3);
    out.push(`links differ (missing ${JSON.stringify(missing)}, extra ${JSON.stringify(extra)})`);
  }
  const srcs = (h) => [...h.matchAll(/<img src="([^"]*)"/g)].map((m) => m[1]);
  if (srcs(en.html).join() !== srcs(tr.html).join()) out.push("images differ");
  return out;
}

function walkMd(dir, prefix = "") {
  const out = [];
  for (const n of existsSync(dir) ? readdirSync(dir, { withFileTypes: true }) : []) {
    const p = join(dir, n.name);
    if (n.isDirectory()) {
      if (n.name !== "images") out.push(...walkMd(p, `${prefix}${n.name}/`));
    } else if (n.name.endsWith(".md")) out.push(`${prefix}${n.name.slice(0, -3)}`);
  }
  return out;
}

/** Checks on the built pages: structure, links, images, widths and uniqueness. Returns problems. */
export function checkDocs({ pages, dist, site }) {
  const problems = [];
  const fail = (m) => problems.push(m);
  const idsCache = new Map();
  const idsOf = (file) => {
    if (!idsCache.has(file)) idsCache.set(file, new Set([...readFileSync(file, "utf8").matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])));
    return idsCache.get(file);
  };
  const seenTitles = new Map();
  const seenDescriptions = new Map();
  for (const p of pages) {
    const at = (m) => fail(`${p.locale.code} ${p.loc.replace(site.url, "")}: ${m}`);
    const html = p.html;
    const count = (re) => (html.match(re) || []).length;
    if (count(/<h1[\s>]/g) !== 1) at(`needs exactly one h1, has ${count(/<h1[\s>]/g)}`);
    if (!new RegExp(`<html[^>]*lang="${p.locale.lang}"`).test(html)) at("html lang is wrong");
    if (!html.includes(`<link rel="canonical" href="${p.loc}">`)) at("canonical is missing");
    if (!html.includes('hreflang="x-default"')) at("x-default is missing");
    const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]);
    for (const id of new Set(ids.filter((v, i) => ids.indexOf(v) !== i))) at(`duplicate id ${id}`);
    for (const img of html.match(/<img\b[^>]*>/g) || []) for (const attr of ["alt", "width", "height", "src"]) if (!new RegExp(`\\s${attr}="`).test(img)) at(`an <img> has no ${attr}: ${img.slice(0, 80)}`);
    if (/\sstyle="/.test(html)) at("inline style attributes are not allowed");
    if (/\{\{|\}\}/.test(html.replace(/<script[\s\S]*?<\/script>/g, "").replace(/<pre[\s\S]*?<\/pre>/g, "").replace(/<code>[\s\S]*?<\/code>/g, ""))) at("an unresolved placeholder is left");
    const ld = html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/);
    try {
      JSON.parse(ld[1]);
    } catch (e) {
      at(`JSON-LD is not valid: ${e.message}`);
    }
    // Links: none to .md, all internal ones resolve (page and anchor).
    for (const m of html.matchAll(/<a\b[^>]*\shref="([^"]*)"/g)) {
      const href = m[1].replace(/&amp;/g, "&");
      if (/\.md(#|$)/.test(href) && !href.startsWith("http")) at(`a link to a Markdown file is left: ${href}`);
      if (href.startsWith("#")) {
        if (href.length > 1 && !ids.includes(href.slice(1))) at(`link to ${href} has no target`);
      } else if (href.startsWith("/") && !href.startsWith("//")) {
        const [path, hash] = href.split("#");
        const file = path.endsWith("/") ? join(dist, path, "index.html") : join(dist, path);
        if (!existsSync(file)) at(`link to ${href} does not lead anywhere`);
        else if (hash && file.endsWith(".html") && !idsOf(file).has(hash)) at(`link to ${href} has no such heading`);
      }
    }
    // Unique titles and descriptions per language.
    const tk = `${p.locale.code}:${p.title}`;
    if (seenTitles.has(tk)) at(`the title is also the title of ${seenTitles.get(tk)}`);
    seenTitles.set(tk, p.loc);
    const dk = `${p.locale.code}:${p.description}`;
    if (seenDescriptions.has(dk)) at(`the description is also the description of ${seenDescriptions.get(dk)}`);
    seenDescriptions.set(dk, p.loc);
    if (weight(p.title) < 30 || weight(p.title) > 62) at(`title is ${weight(p.title)} wide (30 to 62): ${p.title}`);
    if (weight(p.description) < 110 || weight(p.description) > 165) at(`description is ${weight(p.description)} wide (110 to 165)`);
  }
  return problems;
}
