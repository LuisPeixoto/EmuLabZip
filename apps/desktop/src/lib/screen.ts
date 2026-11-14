/** Where a click on a picture of the device screen lands on the device, in its own pixels. */
export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** A finger that moves further than this, in device pixels, is a swipe and not a tap. */
export const SWIPE_PX = 24;

/**
 * `box` is the picture's rectangle on the page, `border` the width of its frame (which is part of the box), and
 * `natural` the size of the picture itself, which is the device's own pixels. Returns `null` for a click on the frame.
 */
export function toDevice(point: { x: number; y: number }, box: Box, border: number, natural: { width: number; height: number }): { x: number; y: number } | null {
  const w = box.width - 2 * border;
  const h = box.height - 2 * border;
  if (w <= 0 || h <= 0 || natural.width <= 0 || natural.height <= 0) return null;
  const fx = (point.x - box.left - border) / w;
  const fy = (point.y - box.top - border) / h;
  if (fx < 0 || fx >= 1 || fy < 0 || fy >= 1) return null;
  return { x: Math.floor(fx * natural.width), y: Math.floor(fy * natural.height) };
}

export type Gesture = { kind: "tap"; x: number; y: number } | { kind: "swipe"; x1: number; y1: number; x2: number; y2: number; ms: number };

/** A press and a release on the picture, as a tap or a swipe. A press held for a while without moving is a long press: a swipe to itself. */
export function gesture(from: { x: number; y: number }, to: { x: number; y: number }, heldMs: number): Gesture {
  const moved = Math.hypot(to.x - from.x, to.y - from.y);
  if (moved > SWIPE_PX) return { kind: "swipe", x1: from.x, y1: from.y, x2: to.x, y2: to.y, ms: Math.min(Math.max(Math.round(heldMs), 100), 2000) };
  if (heldMs >= 500) return { kind: "swipe", x1: from.x, y1: from.y, x2: from.x, y2: from.y, ms: Math.min(Math.round(heldMs), 3000) };
  return { kind: "tap", x: from.x, y: from.y };
}
