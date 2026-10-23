import {
  api,
  errorText,
  type CpuView,
  type DownloadProgress,
  type Opened,
  type PerfettoPreset,
  type ProcessorStatus,
  type Recording,
  type Rows,
  type TraceRequest,
} from "../api";
import { load, save } from "../prefs";
import { app } from "./app.svelte";

export type RecorderKind = "perfetto" | "cpu" | "instruments";

export interface RecorderSettings {
  kind: RecorderKind;
  preset: PerfettoPreset;
  durationS: number;
  /// Profile every process, not only the app picked in the toolbar (needs root on Android).
  systemWide: boolean;
  customConfig: string;
  template: string;
}

const defaults: RecorderSettings = {
  kind: "perfetto",
  preset: "cpu",
  durationS: 10,
  systemWide: false,
  customConfig: "",
  template: "Time Profiler",
};

export const MAX_DURATION = 300;

export const SAMPLE_CONFIG = `buffers { size_kb: 65536 }
data_sources {
  config {
    name: "linux.ftrace"
    ftrace_config {
      ftrace_events: "sched/sched_switch"
      ftrace_events: "sched/sched_wakeup"
      atrace_categories: "view"
    }
  }
}
`;

export type Tab = "summary" | "flame" | "sql";

class TracesStore {
  recordings = $state.raw<Recording[]>([]);
  selectedId = $state<string | null>(null);
  opened = $state.raw<Opened | null>(null);
  opening = $state(false);
  openError = $state<string | null>(null);
  tab = $state<Tab>("summary");

  settings = $state<RecorderSettings>({ ...defaults, ...load<Partial<RecorderSettings>>("traceRecorder", {}) });
  templates = $state<string[]>([]);

  /** A recording in progress. */
  session = $state<number | null>(null);
  elapsedMs = $state(0);
  stopping = $state(false);
  recordError = $state<string | null>(null);

  processor = $state<ProcessorStatus | null>(null);
  download = $state<DownloadProgress | null>(null);
  downloading = $state(false);

  /** The flame graph on screen: another process or thread than the first. */
  cpu = $state.raw<CpuView | null>(null);
  processKey = $state<string | null>(null);
  threadKey = $state<string | null>(null);
  search = $state("");

  sql = $state("select name, count(*) as n from slice group by name order by n desc limit 20");
  sqlResult = $state.raw<Rows | null>(null);
  sqlError = $state<string | null>(null);
  sqlRunning = $state(false);

  recording = $derived(this.recordings.find((r) => r.id === this.selectedId) ?? null);
  recordingNow = $derived(this.session != null);
  /** What the toolbar's device can record. */
  kinds = $derived<RecorderKind[]>(app.device?.platform === "ios" ? ["instruments"] : ["perfetto", "cpu"]);

  #ticket = 0;

  async init() {
    await Promise.all([this.refresh(), this.refreshProcessor()]);
  }

  async refresh() {
    try {
      this.recordings = await api.traceList();
    } catch (e) {
      app.toast(errorText(e), "err");
    }
  }

  async refreshProcessor() {
    try {
      this.processor = await api.traceProcessorStatus();
    } catch {
      /* running outside Tauri */
    }
  }

  /// Keeps the recorder valid for the selected device: the kind it can record, its templates.
  async syncDevice() {
    const target = app.target;
    if (!this.kinds.includes(this.settings.kind)) this.settings.kind = this.kinds[0];
    this.templates = [];
    if (target && app.online && app.device?.platform === "ios") {
      try {
        const list = await api.traceTemplates(target);
        if (app.target?.id === target.id) {
          this.templates = list;
          if (!list.includes(this.settings.template)) this.settings.template = list.includes("Time Profiler") ? "Time Profiler" : (list[0] ?? "");
        }
      } catch {
        /* templates are a convenience */
      }
    }
  }

  persist() {
    save("traceRecorder", $state.snapshot(this.settings));
  }

  request(): TraceRequest | string {
    const s = this.settings;
    const packageName = app.selectedApp?.id ?? null;
    if (s.kind === "perfetto") {
      if (s.preset === "custom" && !s.customConfig.trim()) return "Paste a Perfetto text configuration first.";
      return { kind: "perfetto", preset: s.preset, config: s.preset === "custom" ? s.customConfig : null, durationS: s.durationS, app: packageName };
    }
    if (s.kind === "cpu") {
      if (!s.systemWide && !packageName) return "Pick an app in the toolbar, or profile the whole system.";
      return { kind: "cpuProfile", app: s.systemWide ? null : packageName, durationS: s.durationS, systemWide: s.systemWide };
    }
    if (!s.template) return "Pick an Instruments template.";
    return { kind: "instruments", template: s.template, app: packageName, durationS: s.durationS };
  }

  async start() {
    const target = app.target;
    const device = app.device;
    if (!target || !device || this.session != null) return;
    const request = this.request();
    if (typeof request === "string") {
      this.recordError = request;
      return;
    }
    this.persist();
    this.recordError = null;
    this.elapsedMs = 0;
    this.stopping = false;
    try {
      this.session = await api.traceStart(target, device.name, request, (e) => {
        if (e.type === "recording") {
          this.elapsedMs = e.elapsedMs;
          this.stopping = e.stopping;
        } else if (e.type === "done") {
          this.session = null;
          this.recordings = [e.recording, ...this.recordings.filter((r) => r.id !== e.recording.id)];
          app.toast("Recording saved", "ok");
          this.open(e.recording.id);
        } else {
          this.session = null;
          this.recordError = e.message;
        }
      });
    } catch (e) {
      this.session = null;
      this.recordError = errorText(e);
    }
  }

  async stop() {
    if (this.session == null) return;
    this.stopping = true;
    await api.traceStop(this.session).catch(() => {});
  }

  async open(id: string) {
    this.selectedId = id;
    this.opened = null;
    this.cpu = null;
    this.openError = null;
    this.processKey = this.threadKey = null;
    this.search = "";
    this.sqlResult = null;
    this.sqlError = null;
    const rec = this.recordings.find((r) => r.id === id);
    if (!rec) return;
    this.tab = rec.format === "perfetto" ? "summary" : "flame";
    if (rec.format === "perfetto") {
      await this.refreshProcessor();
      if (!this.processor?.installed) return; // the view offers the download first
    }
    const ticket = ++this.#ticket;
    this.opening = true;
    try {
      const opened = await api.traceOpen(id);
      if (ticket !== this.#ticket) return;
      this.opened = opened;
      if (opened.view === "cpu") {
        this.cpu = opened;
        this.processKey = opened.processes[0]?.key ?? null;
      }
    } catch (e) {
      if (ticket === this.#ticket) this.openError = errorText(e);
    } finally {
      if (ticket === this.#ticket) this.opening = false;
    }
  }

  async installProcessor() {
    this.downloading = true;
    this.download = { done: 0, total: this.processor?.sizeBytes ?? null };
    try {
      await api.traceInstallProcessor((p) => (this.download = p));
      await this.refreshProcessor();
      app.toast("Perfetto trace processor installed", "ok");
      if (this.selectedId) await this.open(this.selectedId);
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.downloading = false;
      this.download = null;
    }
  }

  async pickProcess(key: string | null, thread: string | null = null) {
    if (!this.selectedId) return;
    this.processKey = key;
    this.threadKey = thread;
    try {
      this.cpu = await api.traceFlame(this.selectedId, key, thread);
    } catch (e) {
      app.toast(errorText(e), "err");
    }
  }

  async runSql() {
    if (!this.selectedId || this.sqlRunning || !this.sql.trim()) return;
    this.sqlRunning = true;
    this.sqlError = null;
    try {
      this.sqlResult = await api.traceQuery(this.selectedId, this.sql);
    } catch (e) {
      this.sqlError = errorText(e);
    } finally {
      this.sqlRunning = false;
    }
  }

  async remove(id: string) {
    try {
      await api.traceDelete(id);
      this.recordings = this.recordings.filter((r) => r.id !== id);
      if (this.selectedId === id) {
        this.selectedId = null;
        this.opened = null;
        this.cpu = null;
      }
    } catch (e) {
      app.toast(errorText(e), "err");
    }
  }

  openInPerfetto() {
    if (this.selectedId) app.attempt(() => api.traceOpenPerfettoUi(this.selectedId!));
  }

  showExternally() {
    if (this.selectedId) app.attempt(() => api.traceOpenExternal(this.selectedId!));
  }

  async saveCopy() {
    if (!this.selectedId) return;
    const path = await app.attempt(() => api.traceSave(this.selectedId!));
    if (path) app.toast(`Saved to ${path}`, "ok");
  }

  async saveFolded() {
    if (!this.selectedId) return;
    const id = this.selectedId;
    const text = await app.attempt(() => api.traceFolded(id, this.processKey, this.threadKey));
    if (text == null) return;
    const path = await app.attempt(() => api.saveText(`${id}.folded.txt`, text));
    if (path) app.toast(`Saved to ${path}`, "ok");
  }
}

export const traces = new TracesStore();
