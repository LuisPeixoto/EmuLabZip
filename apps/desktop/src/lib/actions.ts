// Svelte actions shared by components.

/** Calls `handler` when a pointer press or Escape happens outside `node`. */
export function clickOutside(node: HTMLElement, handler: () => void) {
  const onPointer = (e: PointerEvent) => {
    if (!node.contains(e.target as Node)) handler();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") handler();
  };
  window.addEventListener("pointerdown", onPointer, true);
  window.addEventListener("keydown", onKey);
  return {
    destroy() {
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("keydown", onKey);
    },
  };
}
