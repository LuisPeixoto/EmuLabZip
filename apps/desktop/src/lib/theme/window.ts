import { parseColor } from "./contrast.ts";

/** Zooms the whole interface (text, spacing, everything) to a factor such as 1.1. */
export async function setWebviewZoom(zoom: number): Promise<void> {
  try {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    await getCurrentWebview().setZoom(zoom);
  } catch {
    // In a plain browser there is no webview to zoom.
  }
}

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
