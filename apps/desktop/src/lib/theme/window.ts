import { parseColor } from "./contrast.ts";

/** Paints the native window's own background, which shows while the page loads and when it is resized. */
export async function setWindowBackground(color: string): Promise<void> {
  const rgb = parseColor(color);
  if (!rgb) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setBackgroundColor([rgb[0], rgb[1], rgb[2], 255]);
  } catch {
    // In a plain browser (the dev server) there is no window to paint.
  }
}
