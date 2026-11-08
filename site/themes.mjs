// The theme gallery: every file in site/static/themes/<id>.json is a theme anyone can submit by pull request.
// This reads them with the very same checks the app uses, so what passes here installs in the app, and a bad file fails the build.
// Needs Node 22.6 or newer (it reads the app's own TypeScript).

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const lib = join(here, "..", "apps", "desktop", "src", "lib", "theme");
const { MAX_THEME_BYTES, parse } = await import(join(lib, "theme.ts"));
const { BUILTIN } = await import(join(lib, "builtin.ts"));
const { checks } = await import(join(lib, "contrast.ts"));

export const THEMES_DIR = join(here, "static", "themes");

/** The themes in the gallery, validated, and the problems found. A theme with a problem is left out. */
export function loadGallery(dir = THEMES_DIR) {
  const themes = [];
  const problems = [];
  if (!existsSync(dir)) return { themes, problems };
  const taken = new Set(BUILTIN.map((t) => t.id));
  for (const file of readdirSync(dir).sort()) {
    if (file === "index.json") {
      problems.push("themes/index.json is made by the build; do not add it");
      continue;
    }
    if (!file.endsWith(".json")) {
      problems.push(`${file}: only .json files belong in themes/`);
      continue;
    }
    const path = join(dir, file);
    if (statSync(path).size > MAX_THEME_BYTES) {
      problems.push(`${file}: bigger than ${MAX_THEME_BYTES / 1024} KB`);
      continue;
    }
    const result = parse(readFileSync(path, "utf8"));
    if (!result.ok) {
      problems.push(`${file}: ${result.error}`);
      continue;
    }
    const theme = result.theme;
    if (file !== `${theme.id}.json`) problems.push(`${file}: the file name must be its id, ${theme.id}.json`);
    if (taken.has(theme.id)) problems.push(`${file}: ${theme.id} is the id of a built-in theme or of another one`);
    taken.add(theme.id);
    if (!theme.author) problems.push(`${file}: a theme in the gallery needs an author`);
    if (!theme.description) problems.push(`${file}: a theme in the gallery needs a description`);
    const unreadable = checks(theme).filter((c) => !c.ok && c.needed >= 4.5);
    for (const c of unreadable) problems.push(`${file}: ${c.what} is ${c.ratio?.toFixed(1)}:1, text needs ${c.needed}:1`);
    themes.push(theme);
  }
  return { themes, problems };
}
