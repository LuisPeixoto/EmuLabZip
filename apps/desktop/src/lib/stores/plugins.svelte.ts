import { api, errorText, type PluginEvent, type PluginList, type PluginRow } from "../api";
import { app } from "./app.svelte";

/** How many points a streamed metric keeps. */
const WINDOW = 120;
/** How many log lines a run keeps. */
const MAX_LOG = 500;

export interface MetricSeries {
  name: string;
  unit: string | null;
  values: number[];
}

type TableEvent = Extract<PluginEvent, { type: "table" }>;
type TextEvent = Extract<PluginEvent, { type: "text" }>;

class PluginsStore {
  list = $state.raw<PluginList | null>(null);
  selected = $state<string | null>(null);
  loadError = $state<string | null>(null);

  session = $state<number | null>(null);
  metrics = $state.raw<MetricSeries[]>([]);
  tables = $state.raw<TableEvent[]>([]);
  texts = $state.raw<TextEvent[]>([]);
  logs = $state.raw<{ level: string; message: string }[]>([]);
  errors = $state.raw<string[]>([]);
  /** How the last run ended, once it has. */
  outcome = $state<string | null>(null);

  current = $derived<PluginRow | null>(this.list?.plugins.find((p) => p.manifest.name === this.selected) ?? null);
  running = $derived(this.session != null);

  async refresh() {
    try {
      this.list = await api.pluginList();
      this.loadError = null;
      if (!this.current) this.selected = this.list.plugins[0]?.manifest.name ?? null;
    } catch (e) {
      this.loadError = errorText(e);
    }
  }

  select(name: string) {
    if (name === this.selected) return;
    void this.stop();
    this.selected = name;
    this.clear();
  }

  clear() {
    this.metrics = [];
    this.tables = [];
    this.texts = [];
    this.logs = [];
    this.errors = [];
    this.outcome = null;
  }

  async setEnabled(name: string, enabled: boolean) {
    await app.attempt(() => api.pluginEnable(name, enabled), enabled ? `${name} enabled` : `${name} disabled`);
    if (!enabled) await this.stop();
    await this.refresh();
  }

  #add(event: PluginEvent) {
    switch (event.type) {
      case "metric": {
        // A new object for every point: the view keys its charts by name, and only redraws a chart
        // whose object it has not seen before.
        const at = this.metrics.findIndex((x) => x.name === event.name);
        const next: MetricSeries = at < 0 ? { name: event.name, unit: event.unit, values: [event.value] } : { ...this.metrics[at], values: [...this.metrics[at].values.slice(-(WINDOW - 1)), event.value] };
        this.metrics = at < 0 ? [...this.metrics, next] : this.metrics.map((x, i) => (i === at ? next : x));
        break;
      }
      case "table":
        this.tables = [...this.tables, event];
        break;
      case "text":
        this.texts = [...this.texts, event];
        break;
      case "log":
        this.logs = [...this.logs.slice(-(MAX_LOG - 1)), { level: event.level ?? "info", message: event.message }];
        break;
      case "error":
        this.errors = [...this.errors, event.message];
        break;
    }
  }

  async run() {
    const target = app.target;
    const plugin = this.current;
    if (!target || !plugin || this.session != null) return;
    this.clear();
    try {
      this.session = await api.pluginRun(plugin.manifest.name, target, app.appId, (m) => {
        if (m.type === "event") this.#add(m.event);
        else {
          this.session = null;
          if (m.type === "failed") this.errors = [...this.errors, m.message];
          this.outcome = m.type === "done" ? m.outcome : "Failed";
        }
      });
    } catch (e) {
      this.session = null;
      this.errors = [errorText(e)];
    }
  }

  async stop() {
    const session = this.session;
    if (session == null) return;
    await api.pluginStop(session).catch(() => {});
  }
}

export const plugins = new PluginsStore();
