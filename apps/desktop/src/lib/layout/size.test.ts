import assert from "node:assert/strict";
import { test } from "node:test";

import { clampSize, keyStep, sanitizeSizes } from "./size.ts";

test("a pane stays between its limits", () => {
  assert.equal(clampSize(300, 200, 500, 1000, 200), 300);
  assert.equal(clampSize(50, 200, 500, 1000, 200), 200);
  assert.equal(clampSize(900, 200, 500, 1000, 200), 500);
  assert.equal(clampSize(Number.NaN, 200, 500, 1000, 200), 200);
});

test("the other pane always keeps its room", () => {
  assert.equal(clampSize(500, 200, 500, 600, 200), 400, "only 400 of 600 may go to this pane");
  assert.equal(clampSize(500, 200, 500, 300, 200), 200, "the minimum wins when the window is tiny");
  assert.equal(clampSize(300, 200, 500, 0, 200), 300, "an unmeasured window imposes no limit");
});

test("saved sizes are kept only when they make sense", () => {
  assert.deepEqual(sanitizeSizes({ "a.b": 300.4, bad: "wide", tiny: 5, huge: 99999, "Bad Id": 300, nan: Number.NaN }), { "a.b": 300 });
  assert.deepEqual(sanitizeSizes(null), {});
  assert.deepEqual(sanitizeSizes("x"), {});
});

test("arrow keys move a handle along its axis", () => {
  assert.equal(keyStep("ArrowRight", false, "x"), 16);
  assert.equal(keyStep("ArrowLeft", true, "x"), -64);
  assert.equal(keyStep("ArrowDown", false, "y"), 16);
  assert.equal(keyStep("ArrowUp", false, "y"), -16);
  assert.equal(keyStep("ArrowUp", false, "x"), null);
  assert.equal(keyStep("a", false, "x"), null);
});
