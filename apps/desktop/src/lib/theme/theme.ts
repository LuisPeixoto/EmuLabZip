import { baseColors } from "./builtin.ts";
import { parseColor } from "./contrast.ts";
import { type Base, COLOR_KEYS, DEFAULT_FONTS, type Fonts, type Theme } from "./tokens.ts";

/** A theme file bigger than this is refused: a theme is a few hundred bytes of colours. */
export const MAX_THEME_BYTES = 64 * 1024;

export type Validated = { ok: true; theme: Theme } | { ok: false; error: string };

const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const FUNCTION = /^(?:rgba?|hsla?)\(\s*[0-9.\s,%/deg-]+\)$/i;

/** Whether a string is a plain colour: hex, or `rgb()` and `hsl()` with numbers and nothing else. */
export function isColor(text: unknown): text is string {
  return typeof text === "string" && text.length <= 64 && (HEX.test(text.trim()) || FUNCTION.test(text.trim())) && parseColor(text) !== null;
}

/** Family names separated by commas, in quotes or not. No `url(`, no braces, no semicolons: a font cannot carry CSS. */
const FONT = /^[A-Za-z0-9 _'",.-]{1,200}$/;

export function isFontList(text: unknown): text is string {
  return typeof text === "string" && FONT.test(text) && !/^\s*$/.test(text);
}

const ID = /^[a-z0-9][a-z0-9-]{0,47}$/;

/** A name made into an id: `Ocean Breeze!` becomes `ocean-breeze`. */
export function slug(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

function text(value: unknown, max: number): string | undefined {
  if (typeof value !== "string") return undefined;
  // Control characters are dropped; the text is only ever shown, never used as markup or CSS.
  const clean = [...value].filter((c) => c >= " " && c !== "\u007f").join("").trim();
  return clean ? clean.slice(0, max) : undefined;
}

/**
 * Reads a theme from untrusted JSON. This is the one door for themes from a file, the clipboard or the gallery: only colours
 * and font family names get through, unknown keys are dropped, and what is missing comes from the base's own colours.
 */
export function validate(input: unknown): Validated {
  if (typeof input !== "object" || input === null || Array.isArray(input)) return { ok: false, error: "A theme is a JSON object." };
  const o = input as Record<string, unknown>;
  if (o.schema !== undefined && o.schema !== 1) return { ok: false, error: `This theme is for another version of the format (schema ${String(o.schema)}).` };

  const name = text(o.name, 60);
  if (!name) return { ok: false, error: "A theme needs a name." };
  const id = o.id === undefined ? slug(name) : o.id;
  if (typeof id !== "string" || !ID.test(id)) return { ok: false, error: "The id may only hold lower-case letters, digits and hyphens (up to 48)." };
  if (o.base !== "dark" && o.base !== "light") return { ok: false, error: 'The base must be "dark" or "light".' };
  const base: Base = o.base;

  const colors = baseColors(base);
  if (o.colors !== undefined) {
    if (typeof o.colors !== "object" || o.colors === null || Array.isArray(o.colors)) return { ok: false, error: "The colours must be an object." };
    for (const [key, value] of Object.entries(o.colors as Record<string, unknown>)) {
      if (!COLOR_KEYS.includes(key)) continue;
      if (!isColor(value)) return { ok: false, error: `${key} is not a colour. Use #rrggbb, rgb() or hsl().` };
      colors[key] = value.trim();
    }
  }

  const fonts: Fonts = { ...DEFAULT_FONTS };
  if (o.fonts !== undefined) {
    if (typeof o.fonts !== "object" || o.fonts === null || Array.isArray(o.fonts)) return { ok: false, error: "The fonts must be an object." };
    const f = o.fonts as Record<string, unknown>;
    for (const key of ["sans", "mono"] as const) {
      if (f[key] === undefined) continue;
      if (!isFontList(f[key])) return { ok: false, error: `The ${key} font may only list family names, separated by commas.` };
      fonts[key] = (f[key] as string).trim();
    }
  }

  let radius = 1;
  if (o.shape !== undefined) {
    const s = o.shape as { radius?: unknown } | null;
    if (typeof s !== "object" || s === null) return { ok: false, error: "The shape must be an object." };
    if (s.radius !== undefined) {
      if (typeof s.radius !== "number" || !Number.isFinite(s.radius) || s.radius < 0 || s.radius > 2) return { ok: false, error: "The radius is a number from 0 to 2." };
      radius = s.radius;
    }
  }

  return {
    ok: true,
    theme: {
      schema: 1,
      id,
      name,
      author: text(o.author, 60),
      description: text(o.description, 300),
      base,
      colors,
      fonts,
      shape: { radius },
    },
  };
}

/** Reads a theme from JSON text, with the size limit. */
export function parse(textInput: string): Validated {
  if (textInput.length > MAX_THEME_BYTES) return { ok: false, error: "That file is too big to be a theme." };
  let json: unknown;
  try {
    json = JSON.parse(textInput);
  } catch {
    return { ok: false, error: "That is not JSON." };
  }
  return validate(json);
}

/** A theme as the text people share: only what differs from the base is not worth the saving, so it is all there. */
export function serialize(theme: Theme): string {
  return JSON.stringify({ ...theme, colors: theme.colors }, null, 2) + "\n";
}

// ---------------------------------------------------------------------------------------------

function hex(n: number): string {
  return Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, "0");
}

function hslHex(h: number, s: number, l: number): string {
  const a = (s / 100) * Math.min(l / 100, 1 - l / 100);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return hex((l / 100 - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))) * 255);
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

function hueOf([r, g, b]: [number, number, number]): number {
  const [x, y, z] = [r / 255, g / 255, b / 255];
  const max = Math.max(x, y, z);
  const d = max - Math.min(x, y, z);
  if (d === 0) return 220;
  const h = max === x ? ((y - z) / d) % 6 : max === y ? (z - x) / d + 2 : (x - y) / d + 4;
  return Math.round(((h * 60) + 360) % 360);
}

/** A whole theme from one accent colour: surfaces and text tinted by its hue, in the lightness a base needs. */
export function fromAccent(name: string, accentColor: string, base: Base): Theme {
  const rgb = parseColor(accentColor) ?? [106, 77, 255];
  const h = hueOf(rgb);
  const surfaces =
    base === "dark"
      ? {
          bg: hslHex(h, 14, 12),
          panel: hslHex(h, 14, 15),
          "panel-2": hslHex(h, 14, 17.5),
          elevated: hslHex(h, 14, 21),
          hover: hslHex(h, 14, 23),
          line: hslHex(h, 13, 27),
          "line-strong": hslHex(h, 13, 34),
          fg: hslHex(h, 18, 94),
          muted: hslHex(h, 12, 72),
          faint: hslHex(h, 9, 58),
        }
      : {
          bg: hslHex(h, 25, 96),
          panel: "#ffffff",
          "panel-2": hslHex(h, 25, 98),
          elevated: "#ffffff",
          hover: hslHex(h, 22, 93),
          line: hslHex(h, 20, 90),
          "line-strong": hslHex(h, 18, 82),
          fg: hslHex(h, 25, 12),
          muted: hslHex(h, 12, 36),
          faint: hslHex(h, 10, 50),
        };
  const [r, g, b] = rgb;
  return {
    schema: 1,
    id: slug(name) || "my-theme",
    name,
    base,
    colors: {
      ...baseColors(base),
      ...surfaces,
      accent: `#${hex(r)}${hex(g)}${hex(b)}`,
      "accent-soft": `rgb(${r} ${g} ${b} / 0.15)`,
      select: `rgb(${r} ${g} ${b} / 0.24)`,
      "series-1": `#${hex(r)}${hex(g)}${hex(b)}`,
    },
    fonts: { ...DEFAULT_FONTS },
    shape: { radius: 1 },
  };
}
