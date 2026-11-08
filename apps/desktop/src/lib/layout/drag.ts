/** Follows the pointer from a press on a handle until it is released, calling `set` with the new size. */
export function startDrag(
  e: PointerEvent,
  axis: "x" | "y",
  from: number,
  /** 1 when moving right or down makes the pane bigger, -1 when it makes it smaller. */
  sign: 1 | -1,
  set: (size: number) => void,
  end: () => void,
): void {
  if (e.button !== 0) return;
  const handle = e.currentTarget as HTMLElement;
  handle.setPointerCapture(e.pointerId);
  const start = axis === "x" ? e.clientX : e.clientY;
  const move = (ev: PointerEvent) => set(from + sign * ((axis === "x" ? ev.clientX : ev.clientY) - start));
  const up = () => {
    handle.removeEventListener("pointermove", move);
    handle.removeEventListener("pointerup", up);
    handle.removeEventListener("pointercancel", up);
    end();
  };
  handle.addEventListener("pointermove", move);
  handle.addEventListener("pointerup", up);
  handle.addEventListener("pointercancel", up);
}
