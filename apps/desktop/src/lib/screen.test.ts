import assert from "node:assert/strict";
import { test } from "node:test";
import { describeStep } from "./flow.ts";
import { gesture, toDevice } from "./screen.ts";

const box = { left: 100, top: 50, width: 210, height: 430 };

test("a click lands on the device pixel under it, inside the frame", () => {
  // The picture is 200 by 420 inside a 5 px frame, of a 1080 by 2268 screen.
  const natural = { width: 1080, height: 2268 };
  assert.deepEqual(toDevice({ x: 105, y: 55 }, box, 5, natural), { x: 0, y: 0 });
  assert.deepEqual(toDevice({ x: 205, y: 265 }, box, 5, natural), { x: 540, y: 1134 });
  assert.equal(toDevice({ x: 102, y: 100 }, box, 5, natural), null, "the frame is not the screen");
  assert.equal(toDevice({ x: 305, y: 100 }, box, 5, natural), null, "the far edge is outside");
  assert.equal(toDevice({ x: 150, y: 100 }, { ...box, width: 8 }, 5, natural), null, "a box with no room");
  assert.equal(toDevice({ x: 150, y: 100 }, box, 5, { width: 0, height: 0 }), null);
});

test("a press and a release are a tap, a swipe or a long press", () => {
  assert.deepEqual(gesture({ x: 10, y: 10 }, { x: 12, y: 11 }, 80), { kind: "tap", x: 10, y: 10 });
  assert.deepEqual(gesture({ x: 10, y: 10 }, { x: 10, y: 400 }, 240), { kind: "swipe", x1: 10, y1: 10, x2: 10, y2: 400, ms: 240 });
  assert.deepEqual(gesture({ x: 10, y: 10 }, { x: 10, y: 400 }, 5), { kind: "swipe", x1: 10, y1: 10, x2: 10, y2: 400, ms: 100 });
  assert.deepEqual(gesture({ x: 10, y: 10 }, { x: 11, y: 10 }, 900), { kind: "swipe", x1: 10, y1: 10, x2: 10, y2: 10, ms: 900 });
});

test("steps are described in words", () => {
  assert.equal(describeStep({ type: "tap", x: 1, y: 2 }), "Tap at 1, 2");
  assert.equal(describeStep({ type: "tap", x: 1, y: 2, label: { id: "go", text: "Go" } }), "Tap “Go”");
  assert.equal(describeStep({ type: "wait", ms: 2500 }), "Wait 2.5 s");
  assert.equal(describeStep({ type: "wait", ms: 3000 }), "Wait 3 s");
  assert.equal(describeStep({ type: "wait", ms: 700 }), "Wait 700 ms");
  assert.equal(describeStep({ type: "key", key: "volume_up" }), "Press volume up");
  assert.equal(describeStep({ type: "launch" }), "Start the app");
});
