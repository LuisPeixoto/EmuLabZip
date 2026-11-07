import { api, errorText, type CrashKind, type CrashReport, type CrashSummary, type Target } from "../api";
import { app } from "./app.svelte";

/** How many new crashes get a toast each; the rest only count in the sidebar. */
const MAX_TOASTS = 3;

class CrashesStore {
  list = $state.raw<CrashSummary[]>([]);
  selectedId = $state<string | null>(null);
  report = $state.raw<CrashReport | null>(null);
  /** What made the open crash readable, and what is missing for it. */
  applied = $state.raw<string[]>([]);
  missing = $state.raw<string[]>([]);
  /** Show the names as the device wrote them, even when a mapping is stored. */
  original = $state(false);
  loading = $state(false);
  error = $state<string | null>(null);
  kind = $state<CrashKind | "all">("all");
  /** Only the app picked in the toolbar. */
  onlyApp = $state(true);
  /** Crashes that arrived since the Crashes view was last open. */
  unseen = $state(0);

  visible = $derived.by(() => {
    const picked = app.selectedApp?.id;
    return this.list.filter((c) => (this.kind === "all" || c.kind === this.kind) && (!this.onlyApp || !picked || c.app === picked || c.process === picked));
  });
  selected = $derived(this.list.find((c) => c.id === this.selectedId) ?? null);

  #key = "";
  #seen = new Set<string>();
  #ticket = 0;

  /** Follows a device: forgets the last one, and lists the crashes of the new one. */
  attach(key: string, target: Target) {
    if (key === this.#key) return;
    this.#key = key;
    this.#seen = new Set();
    this.list = [];
    this.selectedId = null;
    this.report = null;
    this.unseen = 0;
    this.error = null;
    void this.refresh(target, true);
  }

  detach() {
    this.#key = "";
    this.list = [];
    this.selectedId = null;
    this.report = null;
    this.unseen = 0;
  }

  async refresh(target = app.target, quiet = false) {
    if (!target || !app.online) return;
    const key = this.#key;
    try {
      const list = await api.crashList(target, null);
      if (key !== this.#key) return;
      const fresh = list.filter((c) => !this.#seen.has(c.id));
      const first = this.#seen.size === 0 && quiet;
      list.forEach((c) => this.#seen.add(c.id));
      this.list = list;
      this.error = null;
      if (!first && fresh.length) {
        if (app.view !== "crashes") this.unseen += fresh.length;
        fresh.slice(0, MAX_TOASTS).forEach((c) => app.toast(`${label(c.kind)} in ${c.app ?? c.process}: ${c.title}`, "err"));
      }
    } catch (e) {
      if (!quiet) this.error = errorText(e);
    }
  }

  markSeen() {
    this.unseen = 0;
  }

  async open(id: string) {
    const target = app.target;
    if (!target) return;
    this.selectedId = id;
    this.report = null;
    this.applied = [];
    this.missing = [];
    this.error = null;
    const ticket = ++this.#ticket;
    this.loading = true;
    try {
      const readable = await api.crashGet(target, id, this.original);
      if (ticket === this.#ticket) {
        this.report = readable.report;
        this.applied = readable.applied;
        this.missing = readable.missing;
      }
    } catch (e) {
      if (ticket === this.#ticket) this.error = errorText(e);
    } finally {
      if (ticket === this.#ticket) this.loading = false;
    }
  }

  /** Keeps a mapping.txt, source map or dSYM for the open crash's app and version, and reads the crash again. */
  async addMapping(folder: boolean) {
    const report = this.report;
    const pkg = report?.summary.app;
    if (!report || !pkg) return;
    if (!report.appVersion) {
      app.toast("This crash does not say which version of the app it came from.", "err");
      return;
    }
    const stored = await app.attempt(() => api.mappingAdd(pkg, report.appVersion!, folder));
    if (!stored) return;
    app.toast(`Stored for ${stored.app} ${stored.version}`, "ok");
    if (this.selectedId) await this.open(this.selectedId);
  }

  setOriginal(original: boolean) {
    if (this.original === original) return;
    this.original = original;
    if (this.selectedId) void this.open(this.selectedId);
  }

  async save() {
    const target = app.target;
    if (!target || !this.selectedId) return;
    const path = await app.attempt(() => api.crashSave(target, this.selectedId!));
    if (path) app.toast(`Saved to ${path}`, "ok");
  }
}

export function label(kind: CrashKind): string {
  return { crash: "Crash", nativeCrash: "Native crash", anr: "ANR" }[kind];
}

export const crashes = new CrashesStore();
