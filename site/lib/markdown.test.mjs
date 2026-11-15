import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { highlight } from "./highlight.mjs";
import { plain, render, slugify } from "./markdown.mjs";

const html = (md, options) => render(md, options).html;

test("headings get GitHub-style ids, the first h1 is the title and is not repeated", () => {
  const r = render("# The Title\n\nIntro text.\n\n## Set up `adb`\n\n## Set up `adb`\n");
  assert.equal(r.title, "The Title");
  assert.equal(r.summary, "Intro text.");
  assert.deepEqual(r.headings.map((h) => h.id), ["the-title", "set-up-adb", "set-up-adb-1"]);
  assert.ok(!r.html.includes("<h1"));
  assert.match(r.html, /<h2 id="set-up-adb">Set up <code>adb<\/code><a class="h-anchor" href="#set-up-adb"/);
  assert.equal(slugify("Réseau & Données: 3 étapes"), "réseau--données-3-étapes");
});

test("inline markup: emphasis, code, escapes, entities and snake_case", () => {
  assert.equal(html("a **b** *c* _d_ `e<f>` g_h_i \\* &amp; &"), "<p>a <strong>b</strong> <em>c</em> <em>d</em> <code>e&lt;f&gt;</code> g_h_i * &amp; &amp;</p>");
  assert.equal(html("``a ` b``"), "<p><code>a ` b</code></p>");
  assert.equal(html("1 < 2 and <unknown>"), "<p>1 &lt; 2 and &lt;unknown&gt;</p>");
});

test("links go through the hook, and can be dropped", () => {
  const r = html("[x](a.md#b) and [y](https://e.com/p_(1)) and <https://z.dev>", { link: (h) => (h.endsWith(".md#b") ? "/docs/a/#b" : h) });
  assert.match(r, /<a href="\/docs\/a\/#b">x<\/a>/);
  assert.match(r, /<a href="https:\/\/e.com\/p_\(1\)" rel="noopener">y<\/a>/);
  assert.match(r, /<a href="https:\/\/z.dev" rel="noopener">https:\/\/z.dev<\/a>/);
  assert.equal(html("[gone](x.md)", { link: () => null }), "<p>gone</p>");
});

test("images use the hook and carry size, lazy loading and alt", () => {
  const r = html("![A shot](images/a.png)", { image: (src) => ({ src: "/assets/a.webp", srcset: "/assets/a-sm.webp 800w, /assets/a.webp 1600w", width: 1600, height: 1000 }) });
  assert.match(r, /<img src="\/assets\/a.webp" srcset="[^"]+" sizes="[^"]+" alt="A shot" width="1600" height="1000" loading="lazy"/);
});

test("lists: bullets, numbers, nesting, loose items and wrapped lines", () => {
  assert.equal(html("- a\n- b\n  wrapped\n  - c\n- d"), "<ul><li>a</li><li>b\nwrapped\n<ul><li>c</li></ul></li><li>d</li></ul>");
  assert.equal(html("3. x\n4. y"), '<ol start="3"><li>x</li><li>y</li></ol>');
  assert.match(html("- a\n\n  second para\n\n- b"), /<li><p>a<\/p>\n<p>second para<\/p><\/li><li><p>b<\/p><\/li>/);
  assert.equal(html("1. one\n2. two\n\ntext"), "<ol><li>one</li><li>two</li></ol>\n<p>text</p>");
});

test("tables: alignment, inline code with pipes, escaped pipes and ragged rows", () => {
  const r = html("| A | B |\n|:-:|--:|\n| `x|y` | a\\|b |\n| only |");
  assert.match(r, /<th class="al-c">A<\/th><th class="al-r">B<\/th>/);
  assert.match(r, /<td class="al-c"><code>x\|y<\/code><\/td><td class="al-r">a\|b<\/td>/);
  assert.match(r, /<td class="al-c">only<\/td><td class="al-r"><\/td>/);
});

test("code fences keep their text and highlight by language", () => {
  const r = html("```bash\n$ emulabzip net --rules r.json # go\nexport A=\"x\"\n```");
  assert.match(r, /<pre class="code" data-lang="bash"><code><span class="t-p">\$ <\/span>emulabzip net <span class="t-f">--rules<\/span> r.json<span class="t-c"> # go<\/span>/);
  assert.match(html("```\n<b>&</b>\n```"), /<code>&lt;b&gt;&amp;&lt;\/b&gt;<\/code>/);
  assert.match(html("````text\n```\n````"), /<code>```<\/code>/);
});

test("block quotes nest, and the raw HTML allowlist drops everything else", () => {
  assert.equal(html("> a\n> - b"), "<blockquote><p>a</p>\n<ul><li>b</li></ul></blockquote>");
  const r = html('<p align="center">\n  <img src="images/x.png" alt="X" />\n</p>\n\n<script>alert(1)</script>', { image: () => ({ src: "/x.webp" }) });
  assert.match(r, /<p class="center">\s*<img src="\/x.webp" alt="X" loading="lazy" decoding="async">\s*<\/p>/);
  assert.ok(!r.includes("<script"));
  assert.match(html("Press <kbd>⌘F</kbd> now <br> <b onclick=x>b</b>"), /<kbd>⌘F<\/kbd> now <br> <b>b<\/b>/);
});

test("plain text drops markup", () => {
  assert.equal(plain("A **b** [c](d) `e` ![f](g)"), "A b c e f");
});

test("highlighting escapes what it does not know and handles each language", () => {
  assert.equal(highlight("<a & b>", "nope"), "&lt;a &amp; b&gt;");
  assert.match(highlight('{"a": [1, true]}', "json"), /<span class="t-t">"a"<\/span>: \[<span class="t-n">1<\/span>, <span class="t-k">true<\/span>\]/);
  assert.match(highlight("[net]\nport = 8080 # x", "toml"), /<span class="t-h">\[net\]<\/span>\n<span class="t-t">port<\/span> = <span class="t-n">8080<\/span>/);
  assert.match(highlight("key: value # c", "yaml"), /<span class="t-t">key<\/span>: value<span class="t-c"> # c<\/span>/);
  assert.match(highlight('<a href="x">', "xml"), /<span class="t-t">&lt;a<\/span> <span class="t-a">href<\/span>=<span class="t-s">"x"<\/span><span class="t-t">&gt;<\/span>/);
  assert.match(highlight("fn main() { let x = 1; // hi\n}", "rust"), /<span class="t-k">fn<\/span> main\(\) \{ <span class="t-k">let<\/span> x = <span class="t-n">1<\/span>; <span class="t-c">\/\/ hi<\/span>/);
});

test("every documentation page renders, with no Markdown left over and no raw tags", () => {
  const docs = join(dirname(fileURLToPath(import.meta.url)), "../../docs");
  const files = readdirSync(docs).filter((f) => f.endsWith(".md"));
  assert.ok(files.length >= 50, `found ${files.length} docs`);
  for (const f of files) {
    const r = render(readFileSync(join(docs, f), "utf8"), { image: (src) => ({ src: `/${src}` }), link: (h) => h });
    assert.ok(r.title, `${f} has a title`);
    assert.ok(r.summary.length > 20, `${f} has a summary`);
    // Take out code, then look for what Markdown should have consumed.
    const text = r.html.replace(/<pre[\s\S]*?<\/pre>/g, "").replace(/<code>[\s\S]*?<\/code>/g, "");
    assert.ok(!/\*\*[^*\s]/.test(text), `${f}: a ** is left: ${text.match(/.{20}\*\*.{20}/)?.[0]}`);
    assert.ok(!/\]\([^)]*\)/.test(text), `${f}: a link is left: ${text.match(/.{20}\]\(.{20}/)?.[0]}`);
    assert.ok(!/^\s*#{1,6} /m.test(text), `${f}: a heading marker is left`);
    assert.ok(!/<(script|iframe|style)/i.test(r.html), `${f}: unsafe tag`);
    assert.ok(!/^\|.*\|$/m.test(text), `${f}: a table row is left`);
    // Balanced structure.
    for (const tag of ["ul", "ol", "li", "table", "tr", "blockquote", "pre", "p", "strong", "em"]) {
      const opens = (r.html.match(new RegExp(`<${tag}[ >]`, "g")) || []).length;
      const closes = (r.html.match(new RegExp(`</${tag}>`, "g")) || []).length;
      assert.equal(opens, closes, `${f}: <${tag}> opens ${opens}, closes ${closes}`);
    }
  }
});
