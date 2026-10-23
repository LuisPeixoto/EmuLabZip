import { api, errorText, targetOf, type App, type Device, type HostInfo, type PlatformStatus, type Target } from "../api";
import { load, loadValue, save } from "../prefs";

export type Theme = "dark" | "light";
export type View = "overview" | "logs" | "performance" | "memory" | "storage" | "network" | "traces";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "ok" | "err";
}

export interface Settings {
  /** Lines kept in memory by the log viewer. */
  logBuffer: number;
  /** Lines replayed when a log stream starts (Android). */
  logTail: number;
  /** Include verbose and debug lines. */
  logVerbose: boolean;
  /** How often the device list is refreshed, in seconds. */
  pollSeconds: number;
  /** The port the proxy listens on, on this computer. */
  proxyPort: number;
}

export const defaultSettings: Settings = { logBuffer: 50_000, logTail: 500, logVerbose: true, pollSeconds: 3, proxyPort: 9090 };

class AppStore {
  theme = $state<Theme>(loadValue<Theme>("theme", "dark"));
  host = $state<HostInfo | null>(null);
  settings = $state<Settings>(load("settings", defaultSettings));
  settingsOpen = $state(false);
  view = $state<View>(loadValue<View>("view", "overview"));

  devices = $state<Device[]>([]);
  platforms = $state<PlatformStatus[]>([]);
  devicesLoaded = $state(false);
  deviceId = $state<string | null>(loadValue<string | null>("device", null));
  device = $derived(this.devices.find((d) => d.id === this.deviceId) ?? null);
  /** Changes only when the selected device or its readiness changes, not on every poll. */
  deviceKey = $derived(this.device ? `${this.device.platform}:${this.device.id}:${this.device.state}` : "");
  target = $derived<Target | null>(this.device ? targetOf(this.device) : null);
  online = $derived(this.device?.state === "online");

  apps = $state<App[]>([]);
  appId = $state<string | null>(null);
  selectedApp = $derived(this.apps.find((a) => a.id === this.appId) ?? null);

  toasts = $state<Toast[]>([]);
  #toastId = 0;
  #pollTimer: ReturnType<typeof setInterval> | undefined;

  async init() {
    try {
      this.host = await api.hostInfo();
    } catch {
      /* running outside Tauri */
    }
    await this.refreshDevices();
    this.startPolling();
  }

  startPolling() {
    clearInterval(this.#pollTimer);
    this.#pollTimer = setInterval(() => this.refreshDevices(), Math.max(1, this.settings.pollSeconds) * 1000);
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    save("theme", theme);
  }

  setView(view: View) {
    this.view = view;
    save("view", view);
  }

  saveSettings(next: Settings) {
    this.settings = next;
    save("settings", next);
    this.startPolling();
  }

  selectDevice(id: string) {
    if (id === this.deviceId) return;
    this.deviceId = id;
    this.apps = [];
    this.appId = null;
    save("device", id);
  }

  selectApp(id: string | null) {
    this.appId = id;
  }

  async refreshDevices() {
    try {
      const report = await api.listDevices();
      this.devices = report.devices;
      this.platforms = report.platforms;
      // Keep the current choice while it exists; otherwise pick the first ready device.
      if (!this.device) {
        const next = report.devices.find((d) => d.state === "online") ?? report.devices[0];
        if (next) this.selectDevice(next.id);
      }
    } catch (e) {
      this.platforms = [];
      this.toast(errorText(e), "err");
    } finally {
      this.devicesLoaded = true;
    }
  }

  async refreshApps() {
    const target = this.target;
    if (!target || !this.online) return;
    try {
      const apps = await api.listApps(target);
      // Ignore answers for a device that is no longer selected.
      if (this.target?.id === target.id) this.apps = apps;
    } catch (e) {
      this.toast(errorText(e), "err");
    }
  }

  toast(text: string, kind: Toast["kind"] = "info") {
    const id = ++this.#toastId;
    this.toasts.push({ id, text, kind });
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === "err" ? 6000 : 3200);
  }

  /** Runs an action, showing errors (and optionally success) as toasts. */
  async attempt<T>(fn: () => Promise<T>, ok?: string): Promise<T | undefined> {
    try {
      const out = await fn();
      if (ok) this.toast(ok, "ok");
      return out;
    } catch (e) {
      this.toast(errorText(e), "err");
      return undefined;
    }
  }
}

export const app = new AppStore();
