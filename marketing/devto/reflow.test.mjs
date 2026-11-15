import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { reflow } from "./reflow.mjs";

test("the lines of a paragraph and of a list item are joined", () => {
  assert.equal(reflow("one two\nthree four\n\nnext"), "one two three four\n\nnext");
  assert.equal(reflow("- first item\n  goes on\n- second\n\ntext"), "- first item goes on\n- second\n\ntext");
  assert.equal(reflow("1. one\n   more\n2. two"), "1. one more\n2. two");
});

test("code, tables, headings, quotes and rules stay as they are", () => {
  const md = "# Title\ntext\nmore\n\n```bash\na\n  b\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> quote\n> more\n\n---\nafter\nit";
  assert.equal(reflow(md), "# Title\ntext more\n\n```bash\na\n  b\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> quote\n> more\n\n---\nafter it");
});

test("in every article only the line breaks inside paragraphs change", () => {
  const dir = join(dirname(fileURLToPath(import.meta.url)), "articles");
  for (const f of readdirSync(dir).filter((n) => n.endsWith(".md"))) {
    const text = readFileSync(join(dir, f), "utf8");
    const out = reflow(text);
    const fences = (t) => t.match(/```[\s\S]*?```/g) ?? [];
    assert.deepEqual(fences(out), fences(text), `${f}: a code block changed`);
    const rows = (t) => t.split("\n").filter((l) => l.startsWith("|"));
    assert.deepEqual(rows(out), rows(text), `${f}: a table row changed`);
    const words = (t) => t.replace(/\s+/g, " ").trim();
    assert.equal(words(out), words(text), `${f}: words were lost or changed`);
    const outside = out.replace(/```[\s\S]*?```/g, "").split("\n").filter((l) => l.trim() && !/^(#|\||>|---|<|\s*([-*+]|\d+[.)])\s|!\[)/.test(l));
    assert.ok(outside.every((l) => !/\S\n\S/.test(l)), `${f}: a paragraph is still wrapped`);
    assert.ok(Math.max(...outside.map((l) => l.length)) > 130, `${f}: paragraphs were not joined`);
  }
});
