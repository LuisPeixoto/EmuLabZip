import { DEFAULT_DARK, DEFAULT_LIGHT, builtin } from "./builtin.ts";
import { isFontList } from "./theme.ts";
import type { Base, Theme } from "./tokens.ts";

/** Dark, light, or whichever the computer is using. */
export type Mode = "dark" | "light" | "system";

/** The choices about how the interface looks that are not part of a theme, and which theme is in use. */
export interface Appearance {
  mode: Mode;
  /** The theme used when the interface is dark, and the one used when it is light. */
  darkId: string;
  lightId: string;
  /** Overrides of the theme's own roundness (0 to 2) and fonts. `null` keeps the theme's. */
  roundness: number | null;
  fontSans: string | null;
  fontMono: string | null;
  /** Text size, as a factor of the normal one. */
  fontScale: number;
  /** How much room things take: tight, normal or roomy. */
  density: number;
  /** The zoom of the whole interface. */
  zoom: number;
}

export const DEFAULT_APPEARANCE: Appearance = {
  mode: "dark",
  darkId: DEFAULT_DARK,
  lightId: DEFAULT_LIGHT,
  roundness: null,
  fontSans: null,
  fontMono: null,
  fontScale: 1,
  density: 1,
  zoom: 1,
};

export const LIMITS = {
  fontScale: [0.85, 1.4],
  density: [0.8, 1.25],
  zoom: [0.8, 1.4],
  roundness: [0, 2],
} as const;

function clamp(value: unknown, [min, max]: readonly [number, number], fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;
}

const ID = /^[a-z0-9][a-z0-9-]{0,47}$/;

/** What was saved, made safe: unknown values go back to the defaults and numbers are kept in range. */
export function sanitize(raw: unknown): Appearance {
  const o = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const d = DEFAULT_APPEARANCE;
  return {
    mode: o.mode === "light" || o.mode === "system" || o.mode === "dark" ? o.mode : d.mode,
    darkId: typeof o.darkId === "string" && ID.test(o.darkId) ? o.darkId : d.darkId,
    lightId: typeof o.lightId === "string" && ID.test(o.lightId) ? o.lightId : d.lightId,
    roundness: o.roundness === null || o.roundness === undefined ? null : clamp(o.roundness, LIMITS.roundness, 1),
    fontSans: isFontList(o.fontSans) ? o.fontSans : null,
    fontMono: isFontList(o.fontMono) ? o.fontMono : null,
    fontScale: clamp(o.fontScale, LIMITS.fontScale, d.fontScale),
    density: clamp(o.density, LIMITS.density, d.density),
    zoom: clamp(o.zoom, LIMITS.zoom, d.zoom),
  };
}

/** The first run after the setting was only "dark" or "light": the same choice, now with themes behind it. */
export function migrate(oldTheme: unknown): Appearance {
  return { ...DEFAULT_APPEARANCE, mode: oldTheme === "light" ? "light" : "dark" };
}

export function activeBase(a: Appearance, systemDark: boolean): Base {
  return a.mode === "system" ? (systemDark ? "dark" : "light") : a.mode;
}

/** The theme in use: the one chosen for the current base, or the default of that base when it is gone. */
export function pick(a: Appearance, themes: Theme[], systemDark: boolean): Theme {
  const base = activeBase(a, systemDark);
  const wanted = base === "dark" ? a.darkId : a.lightId;
  const found = themes.find((t) => t.id === wanted && t.base === base);
  return found ?? builtin(base === "dark" ? DEFAULT_DARK : DEFAULT_LIGHT)!;
}

/** Installed themes with built-in ids are ignored, so a shared theme can never replace one that ships with the app. */
export function merge(builtins: Theme[], installed: Theme[]): Theme[] {
  const seen = new Set(builtins.map((t) => t.id));
  const out = [...builtins];
  for (const t of installed) {
    if (seen.has(t.id)) continue;
    seen.add(t.id);
    out.push(t);
  }
  return out;
}
