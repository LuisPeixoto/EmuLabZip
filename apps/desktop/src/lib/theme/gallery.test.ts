import assert from "node:assert/strict";
import { test } from "node:test";
import { BUILTIN } from "./builtin.ts";
import { GALLERY_URL, fetchGallery, readIndex, shareUrl } from "./gallery.ts";

const theme = { ...BUILTIN[0], id: "mine", name: "Mine", author: "Me", description: "A test." };

test("an index keeps good themes and drops bad ones", () => {
  const bad = { ...theme, id: "evil", fonts: { sans: "x; } body { background: url(http://evil)", mono: "monospace" } };
  const got = readIndex({ schema: 1, themes: [theme, bad, 5, null] });
  assert.deepEqual(got.map((t) => t.id), ["mine"]);
  assert.deepEqual(readIndex(null), []);
  assert.deepEqual(readIndex({ themes: "no" }), []);
});

test("the gallery is fetched from the site and checked", async () => {
  let asked = "";
  const get = (async (url: string) => {
    asked = url;
    return new Response(JSON.stringify({ themes: [theme] }), { status: 200 });
  }) as unknown as typeof fetch;
  assert.equal((await fetchGallery(get)).length, 1);
  assert.equal(asked, GALLERY_URL);
  const down = (async () => new Response("", { status: 503 })) as unknown as typeof fetch;
  await assert.rejects(fetchGallery(down), /503/);
});

test("sharing opens a new file in the gallery folder, small enough for a link", () => {
  const url = new URL(shareUrl(theme));
  assert.equal(url.host, "github.com");
  assert.equal(url.pathname, "/LuisPeixoto/EmuLabZip/new/main/site/static/themes");
  assert.equal(url.searchParams.get("filename"), "mine.json");
  assert.equal(JSON.parse(url.searchParams.get("value")!).id, "mine");
  assert.ok(shareUrl(theme).length < 7000, "GitHub refuses very long links");
});
