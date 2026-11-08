/** The size a pane may have: between its own limits, and never so big that the other pane has no room. */
export function clampSize(value: number, min: number, max: number, available: number, minOther: number): number {
  const room = Number.isFinite(available) && available > 0 ? available - minOther : Infinity;
  const top = Math.max(min, Math.min(max, room));
  return Math.round(Math.min(top, Math.max(min, Number.isFinite(value) ? value : min)));
}

/** What was saved for the panes: only sizes that make sense are kept. */
export function sanitizeSizes(raw: unknown): Record<string, number> {
  const out: Record<string, number> = {};
  if (typeof raw !== "object" || raw === null) return out;
  for (const [id, v] of Object.entries(raw as Record<string, unknown>)) {
    if (/^[a-z0-9._-]{1,48}$/.test(id) && typeof v === "number" && Number.isFinite(v) && v >= 40 && v <= 4000) out[id] = Math.round(v);
  }
  return out;
}

/** A key press on a handle: how far it moves the pane, or `null` when the key is not for it. */
export function keyStep(key: string, shift: boolean, axis: "x" | "y"): number | null {
  const big = shift ? 64 : 16;
  const forward = axis === "x" ? "ArrowRight" : "ArrowDown";
  const back = axis === "x" ? "ArrowLeft" : "ArrowUp";
  if (key === forward) return big;
  if (key === back) return -big;
  return null;
}
