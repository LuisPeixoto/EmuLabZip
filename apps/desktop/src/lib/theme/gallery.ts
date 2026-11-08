import { validate, serialize } from "./theme.ts";
import type { Theme } from "./tokens.ts";

/** Where the community gallery is published: a static folder of the website, so there is no server and no account. */
export const GALLERY_ORIGIN = "https://emulabzip.luispeixoto.dev";
export const GALLERY_URL = `${GALLERY_ORIGIN}/themes/index.json`;

/** The index is a few KB for each theme; a reply bigger than this is not the gallery. */
const MAX_INDEX_CHARS = 2_000_000;

/** The themes in a gallery index. Each one goes through the same checks as any imported theme, and the bad ones are dropped. */
export function readIndex(json: unknown): Theme[] {
  const list = typeof json === "object" && json !== null ? (json as { themes?: unknown }).themes : null;
  if (!Array.isArray(list)) return [];
  return list.map((t) => validate(t)).flatMap((r) => (r.ok ? [r.theme] : []));
}

export async function fetchGallery(get: typeof fetch = fetch): Promise<Theme[]> {
  const response = await get(GALLERY_URL, { cache: "no-cache" });
  if (!response.ok) throw new Error(`The gallery answered ${response.status}.`);
  const text = await response.text();
  if (text.length > MAX_INDEX_CHARS) throw new Error("That is not the gallery.");
  return readIndex(JSON.parse(text));
}

/** GitHub's "create a new file" page, filled with the theme: it forks the repository and opens a pull request. */
export function shareUrl(theme: Theme): string {
  const q = new URLSearchParams({ filename: `${theme.id}.json`, value: serialize(theme), message: `Add the ${theme.name} theme` });
  return `https://github.com/LuisPeixoto/EmuLabZip/new/main/site/static/themes?${q.toString()}`;
}
