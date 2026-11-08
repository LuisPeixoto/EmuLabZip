import assert from "node:assert/strict";
import { test } from "node:test";

import { BUILTIN, builtin } from "./builtin.ts";
import { DEFAULT_APPEARANCE, LIMITS, activeBase, merge, migrate, pick, sanitize } from "./select.ts";

test("what was saved is made safe", () => {
  assert.deepEqual(sanitize(undefined), DEFAULT_APPEARANCE);
  assert.deepEqual(sanitize("nonsense"), DEFAULT_APPEARANCE);
  const s = sanitize({ mode: "system", darkId: "fjord", fontScale: 9, density: -3, zoom: 1.2, roundness: 5, fontSans: "url(x)", fontMono: "Menlo" });
  assert.equal(s.mode, "system");
  assert.equal(s.darkId, "fjord");
  assert.equal(s.fontScale, LIMITS.fontScale[1]);
  assert.equal(s.density, LIMITS.density[0]);
  assert.equal(s.zoom, 1.2);
  assert.equal(s.roundness, 2);
  assert.equal(s.fontSans, null, "a font that could carry CSS is dropped");
  assert.equal(s.fontMono, "Menlo");
  assert.equal(sanitize({ darkId: "../x" }).darkId, DEFAULT_APPEARANCE.darkId);
});

test("the old dark or light setting carries over", () => {
  assert.equal(migrate("light").mode, "light");
  assert.equal(migrate("dark").mode, "dark");
  assert.equal(migrate(undefined).mode, "dark");
  assert.equal(migrate("light").darkId, "midnight");
});

test("the theme in use follows the mode and the computer", () => {
  const a = { ...DEFAULT_APPEARANCE, darkId: "fjord", lightId: "paper" };
  assert.equal(pick({ ...a, mode: "dark" }, BUILTIN, false).id, "fjord");
  assert.equal(pick({ ...a, mode: "light" }, BUILTIN, true).id, "paper");
  assert.equal(pick({ ...a, mode: "system" }, BUILTIN, true).id, "fjord");
  assert.equal(pick({ ...a, mode: "system" }, BUILTIN, false).id, "paper");
  assert.equal(activeBase({ ...a, mode: "system" }, true), "dark");
});

test("a theme that is gone, or of the wrong base, falls back to the default", () => {
  assert.equal(pick({ ...DEFAULT_APPEARANCE, darkId: "deleted" }, BUILTIN, true).id, "midnight");
  assert.equal(pick({ ...DEFAULT_APPEARANCE, mode: "dark", darkId: "paper" }, BUILTIN, true).id, "midnight", "paper is light");
  assert.equal(pick({ ...DEFAULT_APPEARANCE, mode: "light", lightId: "gone" }, BUILTIN, true).id, "daylight");
});

test("a shared theme cannot replace one that ships with the app", () => {
  const evil = { ...builtin("midnight")!, name: "Not Midnight" };
  const merged = merge(BUILTIN, [evil, { ...builtin("fjord")!, id: "mine", name: "Mine" }, { ...builtin("fjord")!, id: "mine", name: "Again" }]);
  assert.equal(merged.find((t) => t.id === "midnight")?.name, "Midnight");
  assert.equal(merged.filter((t) => t.id === "mine").length, 1);
  assert.equal(merged.length, BUILTIN.length + 1);
});
