import { api, errorText, LEVELS, type LogBatch, type LogEntry, type LogLevel, type Target } from "../api";
import { app } from "./app.svelte";

export interface LogFilter {
  minLevel: LogLevel;
  tag: string;
  text: string;
  regex: boolean;
  /** Only lines from the app selected in the toolbar. */
  onlyApp: boolean;
}

const rank = (l: LogLevel) => LEVELS.indexOf(l);

type Matcher = (e: LogEntry) => boolean;

/** Compiles the filter once per change; returns an error message for an invalid regex. */
function compile(f: LogFilter, pids: Set<number> | null): { match: Matcher; error: string | null } {
  const min = rank(f.minLevel);
  const tag = f.tag.trim().toLowerCase();
  const text = f.text.trim();
  let test: ((s: string) => boolean) | null = null;
  let error: string | null = null;
  if (text && f.regex) {
    try {
      const re = new RegExp(text, "i");
      test = (s) => re.test(s);
    } catch (e) {
      error = errorText(e);
    }
  } else if (text) {
    const needle = text.toLowerCase();
    test = (s) => s.toLowerCase().includes(needle);
  }
  const match: Matcher = (e) =>
    rank(e.level) >= min &&
    (!pids || pids.has(e.pid)) &&
    (!tag || e.tag.toLowerCase().includes(tag)) &&
    (!test || test(e.message) || test(e.tag));
  return { match, error };
}

class LogStore {
  /** Every buffered line, oldest first. Raw state: arrays are replaced, never mutated in place. */
  entries = $state.raw<LogEntry[]>([]);
  filter = $state<LogFilter>({ minLevel: "verbose", tag: "", text: "", regex: false, onlyApp: false });
  paused = $state(false);
  /** The stream is running. */
  live = $state(false);
  ended = $state(false);
  dropped = $state(0);
  /** Lines per second, smoothed. */
  rate = $state(0);
  selected = $state<LogEntry | null>(null);

  #session: number | null = null;
  #attachedKey = "";
  #pending: LogEntry[] = [];
  #nextSeq = 0;
  #rateWindow: number[] = [];

  /** Process ids of the selected app, or null when the app filter is off. */
  pids = $derived.by(() => {
    if (!this.filter.onlyApp || !app.selectedApp) return null;
    return new Set(app.selectedApp.pid != null ? [app.selectedApp.pid] : []);
  });

  compiled = $derived(compile(this.filter, this.pids));
  visible = $derived(this.entries.filter(this.compiled.match));

  /** Follows the log of `target`. Calling it again with the same key is a no-op. */
  async attach(key: string, target: Target) {
    if (key === this.#attachedKey) return;
    await this.detach();
    this.#attachedKey = key;
    this.clear();
    this.ended = false;
    try {
      const id = await api.startLogs(
        target,
        { tail: app.settings.logTail, verbose: app.settings.logVerbose },
        (b) => this.#onBatch(key, b),
      );
      if (this.#attachedKey !== key) {
        // The user switched devices while the stream was starting.
        api.stopSession(id);
        return;
      }
      this.#session = id;
      this.live = true;
    } catch (e) {
      this.#attachedKey = "";
      app.toast(`Could not start logs: ${errorText(e)}`, "err");
    }
  }

  async detach() {
    const id = this.#session;
    this.#session = null;
    this.#attachedKey = "";
    this.live = false;
    if (id != null) await api.stopSession(id).catch(() => {});
  }

  /** Restarts the stream, for example after changing the verbosity in Settings. */
  async restart(target: Target, key: string) {
    await this.detach();
    await this.attach(key, target);
  }

  #onBatch(key: string, batch: LogBatch) {
    if (key !== this.#attachedKey) return;
    // Sequence numbers restart with each stream; renumber so keys stay unique across restarts.
    for (const e of batch.entries) e.seq = ++this.#nextSeq;
    this.dropped += batch.dropped;
    this.#trackRate(batch.entries.length);
    if (batch.ended) {
      this.live = false;
      this.ended = true;
    }
    if (this.paused) {
      this.#pending.push(...batch.entries);
      if (this.#pending.length > app.settings.logBuffer) this.#pending.splice(0, this.#pending.length - app.settings.logBuffer);
      return;
    }
    this.#append(batch.entries);
  }

  #append(lines: LogEntry[]) {
    if (!lines.length) return;
    const max = app.settings.logBuffer;
    const next = this.entries.length + lines.length > max ? this.entries.slice(this.entries.length + lines.length - max) : this.entries.slice();
    for (const l of lines.length > max ? lines.slice(lines.length - max) : lines) next.push(l);
    this.entries = next;
  }

  #trackRate(n: number) {
    const now = Date.now();
    this.#rateWindow.push(now, n);
    while (this.#rateWindow.length && this.#rateWindow[0] < now - 2000) this.#rateWindow.splice(0, 2);
    let total = 0;
    for (let i = 1; i < this.#rateWindow.length; i += 2) total += this.#rateWindow[i];
    this.rate = Math.round(total / 2);
  }

  togglePause() {
    this.paused = !this.paused;
    if (!this.paused) {
      const pending = this.#pending;
      this.#pending = [];
      this.#append(pending);
    }
  }

  clear() {
    this.entries = [];
    this.#pending = [];
    this.dropped = 0;
    this.selected = null;
  }

  resetFilter() {
    this.filter = { minLevel: "verbose", tag: "", text: "", regex: false, onlyApp: false };
  }

  /** Plain text in the logcat `threadtime` layout. */
  asText(lines: LogEntry[]): string {
    return lines
      .map((e) => `${e.time} ${String(e.pid).padStart(5)} ${String(e.tid).padStart(5)} ${e.level[0].toUpperCase()} ${e.tag}: ${e.message}`)
      .join("\n");
  }

  asJsonLines(lines: LogEntry[]): string {
    return lines.map((e) => JSON.stringify(e)).join("\n");
  }
}

export const logs = new LogStore();
