/** The colour tokens a theme sets, in the groups the editor shows. Each is a CSS variable of the same name. */
export interface TokenGroup {
  id: string;
  label: string;
  keys: { key: string; label: string }[];
}

export const COLOR_GROUPS: TokenGroup[] = [
  {
    id: "surfaces",
    label: "Surfaces",
    keys: [
      { key: "bg", label: "Window background" },
      { key: "panel", label: "Panels" },
      { key: "panel-2", label: "Panel headers and wells" },
      { key: "elevated", label: "Menus and dialogs" },
      { key: "hover", label: "Hover" },
      { key: "line", label: "Borders" },
      { key: "line-strong", label: "Strong borders" },
    ],
  },
  {
    id: "text",
    label: "Text",
    keys: [
      { key: "fg", label: "Text" },
      { key: "muted", label: "Secondary text" },
      { key: "faint", label: "Hints and icons" },
    ],
  },
  {
    id: "accent",
    label: "Accent",
    keys: [
      { key: "accent", label: "Accent" },
      { key: "accent-2", label: "Second accent" },
      { key: "accent-soft", label: "Accent tint" },
      { key: "select", label: "Selection" },
    ],
  },
  {
    id: "status",
    label: "Status",
    keys: [
      { key: "ok", label: "Good" },
      { key: "warn", label: "Warning" },
      { key: "err", label: "Error" },
      { key: "info", label: "Information" },
    ],
  },
  {
    id: "charts",
    label: "Charts",
    keys: [
      { key: "series-1", label: "First series" },
      { key: "series-2", label: "Second series" },
      { key: "flame-app", label: "Flame graph: app" },
      { key: "flame-framework", label: "Flame graph: framework" },
      { key: "flame-native", label: "Flame graph: native" },
      { key: "flame-kernel", label: "Flame graph: kernel" },
    ],
  },
  {
    id: "levels",
    label: "Log levels",
    keys: [
      { key: "lv-verbose", label: "Verbose" },
      { key: "lv-debug", label: "Debug" },
      { key: "lv-info", label: "Info" },
      { key: "lv-warn", label: "Warning" },
      { key: "lv-error", label: "Error" },
      { key: "lv-fatal", label: "Fatal" },
    ],
  },
];

export const COLOR_KEYS: string[] = COLOR_GROUPS.flatMap((g) => g.keys.map((k) => k.key));

export type Base = "dark" | "light";

export interface Fonts {
  sans: string;
  mono: string;
}

export interface Theme {
  schema: 1;
  /** Lower-case letters, digits and hyphens. */
  id: string;
  name: string;
  author?: string;
  description?: string;
  /** Whether it is a dark or a light theme: decides the browser's own controls and the shadow. */
  base: Base;
  colors: Record<string, string>;
  fonts: Fonts;
  /** 0 is square, 1 is the usual roundness, 2 is very round. */
  shape: { radius: number };
}

export const DEFAULT_FONTS: Fonts = {
  sans: '"Inter Variable", Inter, -apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", system-ui, Roboto, "Helvetica Neue", sans-serif',
  mono: '"JetBrains Mono", "SF Mono", "Cascadia Code", Menlo, Consolas, "Liberation Mono", monospace',
};

export const SHADOWS: Record<Base, string> = {
  light: "0 10px 40px -10px rgb(20 24 36 / 0.25), 0 2px 8px rgb(20 24 36 / 0.06)",
  dark: "0 16px 50px -12px rgb(0 0 0 / 0.7), 0 2px 10px rgb(0 0 0 / 0.35)",
};
