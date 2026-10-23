import { api, errorText, type PerfSample, type Target } from "../api";
import { app } from "./app.svelte";

/** How many samples the charts keep: two minutes at the default interval. */
export const WINDOW = 120;

class PerfStore {
  samples = $state.raw<PerfSample[]>([]);
  running = $state(false);
  intervalMs = $state(1000);
  latest = $derived(this.samples.at(-1) ?? null);

  #session: number | null = null;
  #key = "";
  /** Device and app being sampled. Samples are kept across interval changes, not subject changes. */
  #subject = "";

  /** Starts sampling `appId` on `target`, restarting when anything changed. */
  async start(target: Target, appId: string | null) {
    const subject = `${target.platform}:${target.id}:${appId ?? ""}`;
    const key = `${subject}:${this.intervalMs}`;
    if (key === this.#key && this.running) return;
    await this.stop();
    if (subject !== this.#subject) this.samples = [];
    this.#subject = subject;
    this.#key = key;
    try {
      const id = await api.startPerf(target, { app: appId, intervalMs: this.intervalMs }, (s) => {
        if (this.#key !== key) return;
        const next = this.samples.length >= WINDOW ? this.samples.slice(1) : this.samples.slice();
        next.push(s);
        this.samples = next;
      });
      if (this.#key !== key) {
        api.stopSession(id);
        return;
      }
      this.#session = id;
      this.running = true;
    } catch (e) {
      this.#key = "";
      app.toast(`Could not start sampling: ${errorText(e)}`, "err");
    }
  }

  async stop() {
    const id = this.#session;
    this.#session = null;
    this.running = false;
    if (id != null) await api.stopSession(id).catch(() => {});
  }

  clear() {
    this.samples = [];
  }

  /** Values of one field over the window, padded on the left so charts scroll from the right. */
  series(pick: (s: PerfSample) => number | null): (number | null)[] {
    const values = this.samples.map(pick);
    return Array(Math.max(0, WINDOW - values.length)).fill(null).concat(values);
  }
}

export const perf = new PerfStore();
