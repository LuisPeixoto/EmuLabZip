#!/usr/bin/env node
// One-off: turns `text-[12px]` classes into the size tokens of src/app.css, so a theme and the text-size setting can scale them.
//   node scripts/codemod-text-sizes.mjs          # rewrites src/ in place
//   node scripts/codemod-text-sizes.mjs --check  # exits 1 when a pixel size is left (used by CI)
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const check = process.argv.includes("--check");

/** The size of each token in app.css. A pixel size between two tokens goes to the nearest one named here. */
const MAP = {
  9.5: "tiny", 10: "tiny", 10.5: "2xs", 11: "xs", 11.5: "sm", 12: "base", 12.5: "md", 13: "lg",
  13.5: "xl", 14: "xl", 15: "2xl", 16: "2xl", 17: "3xl", 18: "3xl", 20: "3xl",
  21: "4xl", 22: "4xl", 24: "4xl", 26: "5xl", 28: "5xl",
};

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* files(path);
    else if (/\.(svelte|ts)$/.test(name) && !name.endsWith(".test.ts")) yield path;
  }
}

let changed = 0;
let left = 0;
for (const file of files(root)) {
  const before = readFileSync(file, "utf8");
  const after = before.replace(/text-\[(\d+(?:\.\d+)?)px\]/g, (whole, px) => {
    const token = MAP[Number(px)];
    if (!token) {
      left++;
      console.error(`${file}: no token for ${px}px`);
      return whole;
    }
    changed++;
    return `text-${token}`;
  });
  if (!check && after !== before) writeFileSync(file, after);
  if (check) left += (before.match(/text-\[\d+(?:\.\d+)?px\]/g) ?? []).length;
}
console.log(check ? `${left} pixel text size(s) left` : `${changed} class(es) rewritten, ${left} left`);
if (left > 0) process.exit(1);
