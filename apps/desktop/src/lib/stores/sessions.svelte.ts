import { api, errorText, type RecordingStatus, type SessionFile, type SessionView } from "../api";
import { app } from "./app.svelte";

export type SessionTab = "overview" | "logs" | "requests" | "timeline" | "screens";

class SessionsStore {
  status = $state<RecordingStatus | null>(null);
  files = $state<SessionFile[]>([]);
  view = $state.raw<SessionView | null>(null);
  tab = $state<SessionTab>("overview");
  busy = $state<"start" | "stop" | "open" | null>(null);
  /** What the next recording will do. */
  network = $state(false);
  screensEvery = $state(0);
  /** The request open in the Requests tab, as HAR describes it. */
  request = $state.raw<Record<string, any> | null>(null);
  requestIndex = $state<number | null>(null);

  recording = $derived(this.status?.recording ?? false);

  #timer: ReturnType<typeof setInterval> | undefined;

  /** Starts watching a recording that is running, and lists what has been recorded. */
  async init() {
    await this.refresh();
    await this.listFiles();
  }

  async listFiles() {
    try {
      this.files = await api.sessionList();
    } catch {
      /* the folder may not exist yet */
    }
  }

  async refresh() {
    try {
      this.status = await api.sessionStatus();
    } catch {
      return;
    }
    if (this.status.recording && !this.#timer) this.#timer = setInterval(() => this.refresh(), 1000);
    if (!this.status.recording && this.#timer) {
      clearInterval(this.#timer);
      this.#timer = undefined;
    }
  }

  async start() {
    const target = app.target;
    if (!target) return;
    this.busy = "start";
    try {
      this.status = await api.sessionStart(target, app.appId, this.network, this.screensEvery || null);
      this.#timer ??= setInterval(() => this.refresh(), 1000);
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async stop() {
    this.busy = "stop";
    try {
      const { path } = await api.sessionStop();
      await this.refresh();
      await this.listFiles();
      await this.open(path);
    } catch (e) {
      app.toast(errorText(e), "err");
      await this.refresh();
    } finally {
      this.busy = null;
    }
  }

  async open(path: string | null) {
    this.busy = "open";
    try {
      const view = await api.sessionOpen(path);
      if (view) {
        this.view = view;
        this.tab = "overview";
        this.request = null;
        this.requestIndex = null;
      }
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }

  async openRequest(index: number) {
    this.requestIndex = index;
    try {
      this.request = await api.sessionRequest(index);
    } catch (e) {
      app.toast(errorText(e), "err");
    }
  }

  async remove(file: SessionFile) {
    try {
      await api.sessionDelete(file.path);
    } catch (e) {
      app.toast(errorText(e), "err");
      return;
    }
    if (this.view?.path === file.path) this.view = null;
    await this.listFiles();
  }

  close() {
    this.view = null;
    this.request = null;
  }
}

export const sessions = new SessionsStore();
