import { applyTheme } from "../theme/apply.ts";
import { BUILTIN } from "../theme/builtin.ts";
import { type Appearance, type Mode, activeBase, merge, migrate, pick, sanitize } from "../theme/select.ts";
import { validate } from "../theme/theme.ts";
import type { Base, Theme } from "../theme/tokens.ts";
import { setWebviewZoom, setWindowBackground } from "../theme/window.ts";
import { loadValue, save } from "../prefs";

const KEY = "appearance";
const INSTALLED = "themes";

/** Themes people installed, read again through the same checks as any theme from outside. */
function loadInstalled(): Theme[] {
  const raw = loadValue<unknown[]>(INSTALLED, []);
  if (!Array.isArray(raw)) return [];
  return raw.map((t) => validate(t)).flatMap((r) => (r.ok ? [r.theme] : []));
}

class ThemeStore {
  appearance = $state<Appearance>(
    (() => {
      const saved = loadValue<unknown>(KEY, null);
      // The first run after this setting existed: carry over the old dark or light choice.
      return saved === null ? migrate(loadValue<unknown>("theme", "dark")) : sanitize(saved);
    })(),
  );
  installed = $state<Theme[]>(loadInstalled());
  systemDark = $state(typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)").matches : true);
  /** A theme being edited, shown live and not yet kept. */
  preview = $state<Theme | null>(null);

  all = $derived(merge(BUILTIN, this.installed));
  base = $derived<Base>(this.preview ? this.preview.base : activeBase(this.appearance, this.systemDark));
  active = $derived<Theme>(this.preview ?? pick(this.appearance, this.all, this.systemDark));

  /** Follows the computer's own dark or light, and paints the theme whenever anything it depends on changes. */
  start(): () => void {
    let stop = () => {};
    if (typeof matchMedia === "function") {
      const query = matchMedia("(prefers-color-scheme: dark)");
      const onChange = (e: MediaQueryListEvent) => (this.systemDark = e.matches);
      query.addEventListener("change", onChange);
      stop = () => query.removeEventListener("change", onChange);
    }
    return stop;
  }

  /** Paints the theme in use. Called from an effect, so it runs again when the theme or a tuning changes. */
  paint() {
    const a = this.appearance;
    const theme = this.active;
    applyTheme(theme, { roundness: a.roundness, fontSans: a.fontSans, fontMono: a.fontMono });
    const root = document.documentElement.style;
    root.setProperty("--font-scale", String(a.fontScale));
    root.setProperty("--density", String(a.density));
    // The window's traffic lights do not zoom, so the toolbar needs the factor to keep its height in real pixels.
    root.setProperty("--zoom", String(a.zoom));
    void setWebviewZoom(a.zoom);
    void setWindowBackground(theme.colors.bg);
  }

  #save() {
    save(KEY, $state.snapshot(this.appearance));
  }

  update(change: Partial<Appearance>) {
    this.appearance = sanitize({ ...this.appearance, ...change });
    this.#save();
  }

  setMode(mode: Mode) {
    this.update({ mode });
  }

  /** Flips between dark and light, using the theme chosen for each. */
  toggle() {
    this.update({ mode: this.base === "dark" ? "light" : "dark" });
  }

  /** Uses a theme: it becomes the dark or the light one by its base, and the interface switches to that base unless it follows the computer. */
  choose(theme: Theme) {
    const mode = this.appearance.mode === "system" ? "system" : theme.base;
    this.update(theme.base === "dark" ? { darkId: theme.id, mode } : { lightId: theme.id, mode });
  }

  /** Keeps a theme. An installed theme with the same id is replaced; a built-in one never is. */
  install(theme: Theme): boolean {
    if (BUILTIN.some((t) => t.id === theme.id)) return false;
    this.installed = [...this.installed.filter((t) => t.id !== theme.id), theme];
    save(INSTALLED, $state.snapshot(this.installed));
    return true;
  }

  remove(id: string) {
    this.installed = this.installed.filter((t) => t.id !== id);
    save(INSTALLED, $state.snapshot(this.installed));
    const a = this.appearance;
    if (a.darkId === id || a.lightId === id) this.update({ darkId: a.darkId === id ? "midnight" : a.darkId, lightId: a.lightId === id ? "daylight" : a.lightId });
  }

  /** Zoom in or out a step, or back to 100% with `null`. */
  zoomBy(step: number | null) {
    this.update({ zoom: step === null ? 1 : Math.round((this.appearance.zoom + step) * 100) / 100 });
  }

  /** The editor's theme, painted live. `null` goes back to the one in use. */
  setPreview(theme: Theme | null) {
    this.preview = theme;
  }
}

export const theme = new ThemeStore();
