import type { Theme } from "./tokens.ts";

/** A colour as red, green and blue from 0 to 255. Transparency is ignored. */
export type Rgb = [number, number, number];

function hslToRgb(h: number, s: number, l: number): Rgb {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return Math.round((l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))) * 255);
  };
  return [f(0), f(8), f(4)];
}

/** Reads a colour the way a theme may write it: hex, or `rgb()` and `hsl()` with numbers. `null` when it is none of them. */
export function parseColor(text: string): Rgb | null {
  const t = text.trim().toLowerCase();
  const hex = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(t);
  if (hex) {
    let h = hex[1];
    if (h.length <= 4) h = [...h].map((c) => c + c).join("");
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  }
  const fn = /^(rgba?|hsla?)\(\s*([0-9.\s,%/deg-]+)\)$/.exec(t);
  if (!fn) return null;
  const nums = fn[2].split(/[\s,/]+/).filter(Boolean).map((n) => parseFloat(n));
  if (nums.length < 3 || nums.slice(0, 3).some((n) => Number.isNaN(n))) return null;
  if (fn[1].startsWith("rgb")) return [nums[0], nums[1], nums[2]].map((n) => Math.max(0, Math.min(255, Math.round(n)))) as Rgb;
  return hslToRgb(((nums[0] % 360) + 360) % 360, Math.max(0, Math.min(1, nums[1] / 100)), Math.max(0, Math.min(1, nums[2] / 100)));
}

function luminance([r, g, b]: Rgb): number {
  const c = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b);
}

/** The WCAG contrast ratio, from 1 to 21. */
export function ratio(a: Rgb, b: Rgb): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

export interface Check {
  /** What the pair is, in words. */
  what: string;
  foreground: string;
  background: string;
  /** The ratio, or `null` when a colour could not be read. */
  ratio: number | null;
  needed: number;
  ok: boolean;
}

/** The pairs that decide whether a theme can be read, and what each one needs (WCAG AA). */
const PAIRS: [string, string, string, number][] = [
  ["Text on the window", "fg", "bg", 4.5],
  ["Text on panels", "fg", "panel", 4.5],
  ["Secondary text on panels", "muted", "panel", 4.5],
  ["Secondary text on the window", "muted", "bg", 4.5],
  ["Hints on panels", "faint", "panel", 3],
  ["Accent on panels", "accent", "panel", 3],
  ["Errors on panels", "err", "panel", 3],
  ["Warnings on panels", "warn", "panel", 3],
  ["Good on panels", "ok", "panel", 3],
  ["Borders on panels", "line-strong", "panel", 1.3],
];

export function checks(theme: Theme): Check[] {
  return PAIRS.map(([what, fg, bg, needed]) => {
    const a = parseColor(theme.colors[fg] ?? "");
    const b = parseColor(theme.colors[bg] ?? "");
    const r = a && b ? ratio(a, b) : null;
    return { what, foreground: fg, background: bg, ratio: r, needed, ok: r !== null && r >= needed };
  });
}
