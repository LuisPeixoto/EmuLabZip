import {
  api,
  errorText,
  type CaInstall,
  type FlowDetail,
  type FlowSummary,
  type Imported,
  type NetBatch,
  type NetState,
  type NetworkStatus,
  type Paused,
  type Rule,
} from "../api";
import { app } from "./app.svelte";

/** The same bound as the proxy's store, so the list never outgrows it. */
const MAX_FLOWS = 10_000;

export type StatusFilter = "all" | "2xx" | "3xx" | "4xx" | "5xx" | "failed";

export interface FlowFilter {
  text: string;
  status: StatusFilter;
  method: string;
  /** Show connections the proxy could not decode (other protocols, apps that pin certificates). */
  tunnels: boolean;
}

const defaultFilter: FlowFilter = { text: "", status: "all", method: "all", tunnels: true };

function statusClass(f: FlowSummary): StatusFilter {
  if (f.state === "failed" && f.status == null) return "failed";
  const s = f.status ?? 0;
  if (s >= 500) return "5xx";
  if (s >= 400) return "4xx";
  if (s >= 300) return "3xx";
  if (s >= 200) return "2xx";
  return "all";
}

class NetStore {
  state = $state<NetState | null>(null);
  /** Oldest first. Replaced, never changed in place, so the view only reacts to whole updates. */
  flows = $state.raw<FlowSummary[]>([]);
  filter = $state<FlowFilter>({ ...defaultFilter });
  selectedId = $state<number | null>(null);
  detail = $state<FlowDetail | null>(null);
  detailError = $state<string | null>(null);
  /** What the selected device's own settings say. */
  device = $state<NetworkStatus | null>(null);
  busy = $state<"start" | "stop" | "connect" | "disconnect" | "certificate" | null>(null);
  certificate = $state<CaInstall | null>(null);
  /** The rules, as saved. */
  rules = $state<Rule[]>([]);
  /** The rules dialog is open; `draft` is a rule to start editing when it opens. */
  rulesOpen = $state(false);
  draft = $state<Rule | null>(null);
  /** Requests and responses held at breakpoints, oldest first. */
  paused = $state.raw<Paused[]>([]);
  /** What the last imported capture held. */
  imported = $state<Imported | null>(null);

  running = $derived(this.state?.running ?? false);
  methods = $derived([...new Set(this.flows.map((f) => f.method))].filter((m) => m !== "CONNECT").sort());
  visible = $derived.by(() => {
    const { text, status, method, tunnels } = this.filter;
    const needle = text.trim().toLowerCase();
    return this.flows.filter(
      (f) =>
        (tunnels || f.kind !== "tunnel") &&
        (status === "all" || statusClass(f) === status) &&
        (method === "all" || f.method === method) &&
        (!needle || f.url.toLowerCase().includes(needle) || f.method.toLowerCase() === needle || String(f.status) === needle),
    );
  });
  /** The device is sending its traffic to our proxy. */
  connected = $derived(this.device?.connected ?? false);

  #index = new Map<number, number>();
  #session: number | null = null;
  #selecting = 0;

  async init() {
    if (this.#session != null) return;
    try {
      this.state = await api.netState();
      this.#session = await api.netSubscribe((b) => this.#apply(b));
      this.rules = await api.netRulesGet();
      await api.netBreakpointsSubscribe((e) => {
        if (e.type === "paused") {
          if (!this.paused.some((p) => p.id === e.paused.id)) this.paused = [...this.paused, e.paused];
        } else {
          this.paused = this.paused.filter((p) => p.id !== e.id);
        }
      });
    } catch (e) {
      app.toast(errorText(e), "err");
    }
  }

  #apply(batch: NetBatch) {
    const next = batch.cleared ? [] : this.flows.slice();
    if (batch.cleared) this.#index.clear();
    let selectedChanged = false;
    for (const f of batch.flows) {
      const i = this.#index.get(f.id);
      if (i === undefined) {
        this.#index.set(f.id, next.length);
        next.push(f);
      } else {
        selectedChanged ||= f.id === this.selectedId && next[i].state !== f.state;
        next[i] = f;
      }
    }
    if (next.length > MAX_FLOWS) {
      next.splice(0, next.length - MAX_FLOWS);
      this.#index = new Map(next.map((f, i) => [f.id, i]));
    }
    this.flows = next;
    if (batch.cleared && this.selectedId != null && !this.#index.has(this.selectedId)) this.select(null);
    // A request that was open while it was still running gets its response when it finishes.
    if (selectedChanged && this.selectedId != null) this.select(this.selectedId);
  }

  async select(id: number | null) {
    this.selectedId = id;
    this.detailError = null;
    const ticket = ++this.#selecting;
    if (id == null) {
      this.detail = null;
      return;
    }
    try {
      const detail = await api.netFlow(id);
      if (ticket === this.#selecting) this.detail = detail;
    } catch (e) {
      if (ticket === this.#selecting) {
        this.detail = null;
        this.detailError = errorText(e);
      }
    }
  }

  async refreshState() {
    try {
      this.state = await api.netState();
    } catch {
      /* the proxy state is informational */
    }
  }

  /** Reads the selected device's proxy setting. */
  async refreshDevice() {
    const target = app.target;
    if (!target || !app.online) {
      this.device = null;
      return;
    }
    try {
      const status = await api.netStatus(target);
      if (app.target?.id === target.id) this.device = status;
    } catch {
      this.device = null;
    }
  }

  async start() {
    this.busy = "start";
    try {
      this.state = await api.netStart(app.settings.proxyPort);
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async stop() {
    this.busy = "stop";
    try {
      this.state = await api.netStop();
      await this.refreshDevice();
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async connect() {
    const target = app.target;
    if (!target) return;
    this.busy = "connect";
    try {
      this.device = await api.netConnect(target);
      await this.refreshState();
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async disconnect() {
    const target = app.target;
    if (!target) return;
    this.busy = "disconnect";
    try {
      this.device = await api.netDisconnect(target);
      await this.refreshState();
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async installCertificate() {
    const target = app.target;
    if (!target) return;
    this.busy = "certificate";
    try {
      this.certificate = await api.netInstallCa(target);
      await this.refreshState();
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async tryAgainOnPassedThrough() {
    this.state = await api.netClearBypass();
    app.toast("Those hosts will be decrypted again on their next request", "ok");
  }

  async clear() {
    await app.attempt(() => api.netClear());
  }

  async exportHar() {
    const path = await app.attempt(() => api.netExportHar());
    if (path) app.toast(`Saved to ${path}`, "ok");
  }

  /** Saves the whole list of rules. Returns whether it was accepted. */
  async saveRules(rules: Rule[]): Promise<boolean> {
    const saved = await app.attempt(() => api.netRulesSet(rules));
    if (!saved) return false;
    this.rules = saved;
    return true;
  }

  /** Opens the rules dialog, optionally with a rule ready to edit. */
  openRules(draft: Rule | null = null) {
    this.draft = draft;
    this.rulesOpen = true;
  }

  async resume(id: number, abort: boolean, edits = {}) {
    await app.attempt(() => api.netResume(id, abort, edits));
  }

  async resumeAll() {
    await app.attempt(() => api.netResumeAll());
  }

  async setHttp3(enabled: boolean) {
    const state = await app.attempt(() => api.netSetHttp3(enabled));
    if (state) this.state = state;
  }

  async setHideHttp3(hide: boolean) {
    const state = await app.attempt(() => api.netSetHideHttp3(hide));
    if (state) this.state = state;
  }

  async importPcap() {
    const result = await app.attempt(() => api.netImportPcap());
    if (!result) return;
    this.imported = result;
    const { httpRequests, tlsConnections, otherConnections } = result.summary;
    app.toast(`Imported ${httpRequests + tlsConnections + otherConnections} connections from ${result.file}`, "ok");
  }

  resetFilter() {
    this.filter = { ...defaultFilter };
  }
}

export const net = new NetStore();
