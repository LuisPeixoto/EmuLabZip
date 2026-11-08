import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SIDEBAR, mergeOrder, move, sanitizeSidebar } from "./sidebar.ts";

test("a saved order is kept, and new entries come last", () => {
  assert.deepEqual(mergeOrder(["a", "b", "c"], ["c", "a", "b"]), ["c", "a", "b"]);
  assert.deepEqual(mergeOrder(["a", "b", "c", "d"], ["c", "a"]), ["c", "a", "b", "d"]);
  assert.deepEqual(mergeOrder(["a", "b"], undefined), ["a", "b"]);
});

test("entries that are gone, or listed twice, do not come back", () => {
  assert.deepEqual(mergeOrder(["a", "b"], ["gone", "b", "b", "a"]), ["b", "a"]);
});

test("moving an entry puts it where the other one was", () => {
  assert.deepEqual(move(["a", "b", "c", "d"], 0, 2), ["b", "c", "a", "d"]);
  assert.deepEqual(move(["a", "b", "c", "d"], 3, 1), ["a", "d", "b", "c"]);
  assert.deepEqual(move(["a", "b"], 1, 1), ["a", "b"]);
  assert.deepEqual(move(["a", "b"], 5, 0), ["a", "b"]);
});

test("what was saved about the sidebar is made safe", () => {
  assert.deepEqual(sanitizeSidebar(undefined), DEFAULT_SIDEBAR);
  assert.deepEqual(sanitizeSidebar("x"), DEFAULT_SIDEBAR);
  const s = sanitizeSidebar({ collapsed: true, hidden: ["logs", 5, "Bad Id", "x".repeat(40)], order: { inspect: ["logs", "layout", null], "Bad Section": ["a"] } });
  assert.equal(s.collapsed, true);
  assert.deepEqual(s.hidden, ["logs"]);
  assert.deepEqual(s.order, { inspect: ["logs", "layout"] });
});
