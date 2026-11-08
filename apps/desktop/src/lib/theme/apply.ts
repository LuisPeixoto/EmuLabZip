import { SHADOWS, type Base, type Theme } from "./tokens.ts";

/** Where the variables of the last theme are kept, so the next start can paint them before anything else runs. */
export const CACHE_KEY = "emulabzip.themeCache";

/** What the person changed on top of a theme. `null` or nothing means the theme's own. */
export interface Tuning {
  roundness?: number | null;
  fontSans?: string | null;
  fontMono?: string | null;
}

/** The CSS variables of a theme: every colour, the shadow, the fonts and the roundness. */
export function cssVars(theme: Theme, tuning: Tuning = {}): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const [key, value] of Object.entries(theme.colors)) vars[`--${key}`] = value;
  vars["--shadow"] = SHADOWS[theme.base];
  vars["--ui-font-sans"] = tuning.fontSans ?? theme.fonts.sans;
  vars["--ui-font-mono"] = tuning.fontMono ?? theme.fonts.mono;
  vars["--roundness"] = String(tuning.roundness ?? theme.shape.radius);
  return vars;
}

/** Puts variables on the page. */
export function applyVars(vars: Record<string, string>, base: Base): void {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
  root.dataset.theme = base;
  root.style.colorScheme = base;
}

/** Paints a theme and keeps it for the next start. */
export function applyTheme(theme: Theme, tuning: Tuning = {}): void {
  const vars = cssVars(theme, tuning);
  applyVars(vars, theme.base);
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ base: theme.base, vars }));
  } catch {
    // Without storage the theme still applies; only the first paint of the next start is plainer.
  }
}

/** Paints what the last run kept. Called before the interface is built, so there is no flash of the wrong colours. */
export function applyCached(): void {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return;
    const cached = JSON.parse(raw) as { base?: Base; vars?: Record<string, string> };
    if ((cached.base === "dark" || cached.base === "light") && cached.vars) applyVars(cached.vars, cached.base);
  } catch {
    // A damaged cache is ignored; the theme is applied again once the interface starts.
  }
}
