import { api, errorText, type Control, type ControlInfo } from "../api";
import { load, save } from "../prefs";
import { app } from "./app.svelte";

export interface Place {
  name: string;
  lat: string;
  lon: string;
}

export const PLACES: Place[] = [
  { name: "São Paulo", lat: "-23.5505", lon: "-46.6333" },
  { name: "San Francisco", lat: "37.7749", lon: "-122.4194" },
  { name: "London", lat: "51.5072", lon: "-0.1276" },
  { name: "Tokyo", lat: "35.6762", lon: "139.6503" },
  { name: "Sydney", lat: "-33.8688", lon: "151.2093" },
];

/** The services of `simctl privacy`. */
export const SERVICES = [
  "photos", "photos-add", "contacts", "contacts-limited", "calendar", "reminders", "location", "location-always",
  "microphone", "motion", "media-library", "siri", "all",
];

/** What the forms remember between runs. */
interface Draft {
  lat: string;
  lon: string;
  time: string;
  battery: number;
  wifi: boolean;
  cellular: boolean;
  title: string;
  body: string;
  payload: string;
  url: string;
  level: number;
  charging: boolean;
  service: string;
}

const defaults: Draft = {
  lat: PLACES[0].lat,
  lon: PLACES[0].lon,
  time: "9:41",
  battery: 100,
  wifi: true,
  cellular: true,
  title: "EmuLabZip",
  body: "A notification from your computer",
  payload: "",
  url: "https://example.com",
  level: 15,
  charging: false,
  service: "photos",
};

class ControlsStore {
  info = $state.raw<ControlInfo | null>(null);
  draft = $state<Draft>(load("controls", defaults));
  /** The action being applied, to disable its button. */
  busy = $state<string | null>(null);
  error = $state<string | null>(null);

  #key = "";

  supports(action: string): boolean {
    return this.info?.supported.includes(action) ?? false;
  }

  /** Reads the device again, for the app picked in the toolbar. */
  async refresh() {
    const target = app.target;
    if (!target || !app.online) {
      this.info = null;
      return;
    }
    const key = `${app.deviceKey}|${app.appId ?? ""}`;
    this.#key = key;
    try {
      const info = await api.controlsInfo(target, app.appId);
      if (key === this.#key) {
        this.info = info;
        this.error = null;
      }
    } catch (e) {
      if (key === this.#key) {
        this.info = null;
        this.error = errorText(e);
      }
    }
  }

  persist() {
    save("controls", $state.snapshot(this.draft));
  }

  /** Applies a control and reads the device back. */
  async apply(name: string, control: Control, done: string) {
    const target = app.target;
    if (!target || this.busy) return;
    this.busy = name;
    try {
      await api.controlApply(target, control);
      this.persist();
      app.toast(done, "ok");
      await this.refresh();
    } catch (e) {
      app.toast(errorText(e), "err");
    } finally {
      this.busy = null;
    }
  }
}

export const controls = new ControlsStore();
