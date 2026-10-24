import { api, errorText, type CrashKind, type CrashReport, type CrashSummary, type Target } from "../api";
import { app } from "./app.svelte";

/** How many new crashes get a toast each; the rest only count in the sidebar. */
const MAX_TOASTS = 3;

class CrashesStore {
  list = $state.raw<CrashSummary[]>([]);
  selectedId = $state<string | null>(null);
  report = $state.raw<CrashReport | null>(null);
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
    this.error = null;
    const ticket = ++this.#ticket;
    this.loading = true;
    try {
      const report = await api.crashGet(target, id);
      if (ticket === this.#ticket) this.report = report;
    } catch (e) {
      if (ticket === this.#ticket) this.error = errorText(e);
    } finally {
      if (ticket === this.#ticket) this.loading = false;
    }
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
