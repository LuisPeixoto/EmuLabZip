import { Channel, invoke } from "@tauri-apps/api/core";

export type Platform = "android" | "ios";
export type DeviceKind = "emulator" | "simulator" | "physical";
export type DeviceState = "online" | "booting" | "offline" | "unauthorized";
export type LogLevel = "verbose" | "debug" | "info" | "warn" | "error" | "fatal";

export const LEVELS: LogLevel[] = ["verbose", "debug", "info", "warn", "error", "fatal"];

export interface Device {
  id: string;
  name: string;
  platform: Platform;
  kind: DeviceKind;
  state: DeviceState;
  osVersion: string | null;
  apiLevel: number | null;
  model: string | null;
  abi: string | null;
}

export interface Target {
  platform: Platform;
  id: string;
}

export interface App {
  id: string;
  name: string;
  version: string | null;
  pid: number | null;
  user: boolean;
}

export interface InfoSection {
  title: string;
  items: [string, string][];
}

export interface PlatformStatus {
  platform: Platform;
  available: boolean;
  error: string | null;
}

export interface DevicesReport {
  devices: Device[];
  platforms: PlatformStatus[];
}

export interface LogEntry {
  seq: number;
  time: string;
  pid: number;
  tid: number;
  level: LogLevel;
  tag: string;
  message: string;
  subsystem: string | null;
}

export interface LogOptions {
  tail: number;
  verbose: boolean;
}

export interface LogBatch {
  entries: LogEntry[];
  dropped: number;
  ended: boolean;
}

export interface HostInfo {
  os: string;
  arch: string;
  version: string;
}

export interface PerfOptions {
  app: string | null;
  intervalMs: number;
}

export interface PerfSample {
  tMs: number;
  pid: number | null;
  appCpu: number | null;
  deviceCpu: number | null;
  appMemoryKb: number | null;
  deviceMemoryUsedKb: number | null;
  deviceMemoryTotalKb: number | null;
  fps: number | null;
  jankPercent: number | null;
  batteryLevel: number | null;
  batteryTempC: number | null;
}

export interface MemoryCategory {
  name: string;
  kb: number;
}

export interface MemoryReport {
  app: string;
  pid: number;
  metric: string;
  totalKb: number;
  categories: MemoryCategory[];
  details: [string, string][];
}

export interface CategoryDelta {
  name: string;
  beforeKb: number;
  afterKb: number;
  deltaKb: number;
}

export interface StorageRoot {
  id: string;
  label: string;
  location: string;
  unavailable: string | null;
}

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modified: number | null;
}

export type FileKind = "sqlite" | "sharedPrefs" | "plist" | "json" | "image" | "text" | "binary";

export interface Pref {
  key: string;
  kind: string;
  value: string;
}

export interface Column {
  name: string;
  declType: string;
  primaryKey: boolean;
  notNull: boolean;
}

export interface Table {
  name: string;
  kind: string;
  rows: number;
  columns: Column[];
  sql: string;
}

export interface Rows {
  columns: string[];
  rows: unknown[][];
  truncated: boolean;
}

export interface Preview {
  id: number;
  name: string;
  size: number;
  kind: FileKind | null;
  text: string | null;
  truncated: boolean;
  image: string | null;
  prefs: Pref[] | null;
  plist: unknown;
  tables: Table[] | null;
  hex: string | null;
  error: string | null;
}

export type FlowKind = "http" | "tunnel" | "webSocket";
export type FlowState = "pending" | "paused" | "completed" | "failed";

export interface FlowSummary {
  id: number;
  startedMs: number;
  kind: FlowKind;
  state: FlowState;
  method: string;
  scheme: string;
  host: string;
  port: number;
  path: string;
  url: string;
  httpVersion: string;
  /** What the proxy spoke to the server: h1, h2 or h3. Empty until it answered. */
  serverProtocol: string;
  status: number | null;
  contentType: string | null;
  requestSize: number;
  responseSize: number;
  durationMs: number | null;
  error: string | null;
  /** The rule that matched this request, if any. */
  rule: string | null;
  mocked: boolean;
  messageCount: number;
  /** "pcap" for flows imported from a capture. */
  source: string | null;
}

export interface NetBatch {
  cleared: boolean;
  flows: FlowSummary[];
}

export interface NetState {
  running: boolean;
  port: number | null;
  caPath: string | null;
  bypassed: string[];
  connected: string[];
  http3: boolean;
  hideHttp3: boolean;
}

export interface NetworkStatus {
  proxy: string | null;
  connected: boolean;
  note: string | null;
}

export type CaKind = "system" | "user" | "simulator";

export interface CaInstall {
  kind: CaKind;
  done: boolean;
  message: string;
  steps: string[];
  snippet: string | null;
}

export interface BodyView {
  size: number;
  decodedSize: number;
  truncated: boolean;
  encoding: string | null;
  kind: FileKind | null;
  text: string | null;
  image: string | null;
  hex: string | null;
  error: string | null;
}

export interface WsMessage {
  tMs: number;
  direction: "up" | "down";
  kind: "text" | "binary" | "close" | "ping" | "pong";
  text: string | null;
  base64: string | null;
  len: number;
  truncated: boolean;
}

export interface Flow extends Omit<FlowSummary, "contentType" | "durationMs" | "messageCount"> {
  messages: WsMessage[];
  messagesDropped: number;
  requestHeaders: [string, string][];
  responseHeaders: [string, string][];
  requestTruncated: boolean;
  responseTruncated: boolean;
  timing: { ttfbMs: number | null; totalMs: number | null };
}

export interface FlowDetail {
  flow: Flow;
  request: BodyView | null;
  response: BodyView | null;
}

export interface RuleMatcher {
  methods: string[];
  host: string | null;
  path: string | null;
  urlContains: string | null;
}

export type RuleAction =
  | { type: "mock"; status: number; headers: [string, string][]; body: string; base64: boolean; delayMs: number }
  | { type: "delay"; ms: number }
  | { type: "block" }
  | { type: "breakpoint"; request: boolean; response: boolean };

export interface Rule {
  id: number;
  name: string;
  enabled: boolean;
  matcher: RuleMatcher;
  action: RuleAction;
}

export interface Paused {
  id: number;
  flow: number;
  phase: "request" | "response";
  method: string;
  url: string;
  status: number | null;
  headers: [string, string][];
  bodyText: string | null;
  bodyBase64: string;
}

export type BreakpointEvent = { type: "paused"; paused: Paused } | { type: "resumed"; id: number };

export interface EditsInput {
  headers?: [string, string][];
  bodyText?: string;
  status?: number;
}

export interface DnsLookup {
  startedMs: number;
  name: string;
  kind: string;
  answers: string[];
  status: string | null;
  durationMs: number | null;
}

export interface CaptureSummary {
  packets: number;
  skippedPackets: number;
  httpRequests: number;
  tlsConnections: number;
  otherConnections: number;
  dnsLookups: number;
  durationMs: number;
  gaps: boolean;
  truncated: boolean;
}

export interface Imported {
  file: string;
  summary: CaptureSummary;
  dns: DnsLookup[];
}

export function targetOf(d: Device): Target {
  return { platform: d.platform, id: d.id };
}

export function osLabel(d: Device): string {
  const name = d.platform === "android" ? "Android" : "iOS";
  if (d.osVersion && d.apiLevel) return `${name} ${d.osVersion} · API ${d.apiLevel}`;
  if (d.osVersion) return `${name} ${d.osVersion}`;
  return name;
}

export type PerfettoPreset = "cpu" | "frames" | "memory" | "custom";

export type TraceRequest =
  | { kind: "perfetto"; preset: PerfettoPreset; config: string | null; durationS: number; app: string | null }
  | { kind: "cpuProfile"; app: string | null; durationS: number; systemWide: boolean }
  | { kind: "instruments"; template: string; app: string | null; durationS: number };

export type TraceFormat = "perfetto" | "simpleperfReport" | "instrumentsBundle";

export interface Recording {
  id: string;
  platform: Platform;
  device: string;
  request: TraceRequest;
  format: TraceFormat;
  createdMs: number;
  file: string;
  extras: string[];
  sizeBytes: number;
}

export type TraceEvent =
  | { type: "recording"; elapsedMs: number; stopping: boolean }
  | { type: "done"; recording: Recording }
  | { type: "failed"; message: string };

export interface ProcessorStatus {
  supported: boolean;
  installed: boolean;
  version: string;
  sizeBytes: number | null;
  url: string | null;
}

export interface DownloadProgress {
  done: number;
  total: number | null;
}

export interface CpuUse {
  name: string;
  pid: number;
  durNs: number;
}

export interface ThreadUse {
  name: string;
  tid: number;
  process: string;
  durNs: number;
}

export interface FrameSummary {
  total: number;
  missed: number;
  p50Ms: number;
  p90Ms: number;
  p99Ms: number;
  worstMs: number;
  jank: [string, number][];
  layers: { name: string; frames: number; missed: number; avgMs: number }[];
}

export interface TraceSummary {
  durationS: number;
  processes: number;
  threads: number;
  cpu: { cpus: number; busyFraction: number; byProcess: CpuUse[]; byThread: ThreadUse[] } | null;
  frames: FrameSummary | null;
  memory: {
    system: { name: string; min: number; max: number; avg: number }[];
    processes: { name: string; pid: number; peakBytes: number; lowBytes: number }[];
  } | null;
  slices: { name: string; count: number; totalNs: number; maxNs: number }[];
}

export type Origin = "app" | "framework" | "native" | "kernel";

export interface FlameNode {
  name: string;
  origin: Origin;
  value: number;
  own: number;
  samples: number;
  children: FlameNode[];
}

export interface CpuView {
  source: "simpleperf" | "instruments";
  unit: "ns" | "events";
  processes: {
    key: string;
    name: string;
    samples: number;
    weight: number;
    threads: { key: string; name: string; weight: number }[];
  }[];
  /** The process the flame graph shows. */
  selected: string | null;
  flame: FlameNode;
  hotspots: { name: string; origin: Origin; own: number }[];
}

export type Opened =
  | { view: "perfetto"; summary: TraceSummary }
  | ({ view: "cpu" } & CpuView)
  | { view: "external"; reason: string };

export type CrashKind = "crash" | "nativeCrash" | "anr";

export interface CrashSummary {
  id: string;
  kind: CrashKind;
  app: string | null;
  process: string;
  pid: number | null;
  timeMs: number | null;
  title: string;
}

export interface CrashFrame {
  index: number;
  symbol: string;
  image: string | null;
  location: string | null;
  origin: Origin;
}

export interface CrashThread {
  name: string;
  crashed: boolean;
  frames: CrashFrame[];
}

export interface CrashHint {
  text: string;
  command: string | null;
}

export interface CrashReport {
  summary: CrashSummary;
  details: [string, string][];
  threads: CrashThread[];
  hints: CrashHint[];
  raw: string;
}

export type Appearance = "light" | "dark";
export type Grant = "grant" | "revoke" | "reset";

export interface StatusBarOverride {
  time: string | null;
  battery: number | null;
  wifi: boolean | null;
  cellular: boolean | null;
}

export type Control =
  | { action: "location"; lat: string; lon: string }
  | { action: "clearLocation" }
  | { action: "appearance"; appearance: Appearance }
  | { action: "statusBar"; statusBar: StatusBarOverride }
  | { action: "clearStatusBar" }
  | { action: "permission"; app: string; permission: string; grant: Grant }
  | { action: "notification"; app: string; title: string; body: string; payload: string | null }
  | { action: "openUrl"; url: string }
  | { action: "battery"; level: number; charging: boolean }
  | { action: "resetBattery" };

export interface ControlInfo {
  appearance: Appearance | null;
  supported: string[];
  permissions: { name: string; granted: boolean }[];
  notes: string[];
}

export type PluginMode = "once" | "stream";

export interface PluginManifest {
  protocol: number;
  name: string;
  version: string;
  description: string;
  platforms: Platform[];
  command: string[];
  mode: PluginMode;
  timeoutS: number | null;
}

export interface PluginRow {
  manifest: PluginManifest;
  dir: string;
  enabled: boolean;
  /** What enabling it lets it do, in one line. */
  runs: string;
}

export interface PluginList {
  dir: string;
  plugins: PluginRow[];
  problems: { dir: string; message: string }[];
}

export type PluginEvent =
  | { type: "metric"; name: string; value: number; unit: string | null }
  | { type: "table"; title: string | null; columns: string[]; rows: unknown[][] }
  | { type: "text"; title: string | null; body: string }
  | { type: "log"; level: string | null; message: string }
  | { type: "error"; message: string };

export type PluginMessage =
  | { type: "event"; event: PluginEvent }
  | { type: "done"; outcome: string }
  | { type: "failed"; message: string };

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export const api = {
  hostInfo: () => invoke<HostInfo>("host_info"),
  listDevices: () => invoke<DevicesReport>("list_devices"),
  deviceInfo: (target: Target) => invoke<InfoSection[]>("device_info", { target }),
  listApps: (target: Target) => invoke<App[]>("list_apps", { target }),
  screenshot: (target: Target) => invoke<string>("screenshot", { target }),
  savePng: (name: string, data: string) => invoke<string | null>("save_png", { name, data }),
  saveText: (name: string, contents: string) => invoke<string | null>("save_text", { name, contents }),
  async startLogs(target: Target, options: LogOptions, onBatch: (b: LogBatch) => void): Promise<number> {
    const channel = new Channel<LogBatch>();
    channel.onmessage = onBatch;
    return invoke<number>("start_logs", { target, options, onBatch: channel });
  },
  stopSession: (id: number) => invoke<void>("stop_session", { id }),
  netState: () => invoke<NetState>("net_state"),
  netStart: (port: number) => invoke<NetState>("net_start", { port }),
  netStop: () => invoke<NetState>("net_stop"),
  netStatus: (target: Target) => invoke<NetworkStatus>("net_status", { target }),
  netConnect: (target: Target) => invoke<NetworkStatus>("net_connect", { target }),
  netDisconnect: (target: Target) => invoke<NetworkStatus>("net_disconnect", { target }),
  netInstallCa: (target: Target) => invoke<CaInstall>("net_install_ca", { target }),
  netClearBypass: () => invoke<NetState>("net_clear_bypass"),
  async netSubscribe(onBatch: (b: NetBatch) => void): Promise<number> {
    const channel = new Channel<NetBatch>();
    channel.onmessage = onBatch;
    return invoke<number>("net_subscribe", { onBatch: channel });
  },
  netRulesGet: () => invoke<Rule[]>("net_rules_get"),
  netRulesSet: (rules: Rule[]) => invoke<Rule[]>("net_rules_set", { rules }),
  async netBreakpointsSubscribe(onEvent: (e: BreakpointEvent) => void): Promise<number> {
    const channel = new Channel<BreakpointEvent>();
    channel.onmessage = onEvent;
    return invoke<number>("net_breakpoints_subscribe", { onEvent: channel });
  },
  netResume: (id: number, abort: boolean, editsInput: EditsInput = {}) =>
    invoke<void>("net_resume", { id, abort, editsInput }),
  netResumeAll: () => invoke<void>("net_resume_all"),
  netSetHttp3: (enabled: boolean) => invoke<NetState>("net_set_http3", { enabled }),
  netSetHideHttp3: (hide: boolean) => invoke<NetState>("net_set_hide_http3", { hide }),
  netImportPcap: () => invoke<Imported | null>("net_import_pcap"),
  crashList: (target: Target, app: string | null) => invoke<CrashSummary[]>("crash_list", { target, app }),
  crashGet: (target: Target, id: string) => invoke<CrashReport>("crash_get", { target, id }),
  crashSave: (target: Target, id: string) => invoke<string | null>("crash_save", { target, id }),
  pluginList: () => invoke<PluginList>("plugin_list"),
  pluginEnable: (name: string, enabled: boolean) => invoke<void>("plugin_enable", { name, enabled }),
  async pluginRun(name: string, target: Target, pkg: string | null, onMessage: (m: PluginMessage) => void): Promise<number> {
    const channel = new Channel<PluginMessage>();
    channel.onmessage = onMessage;
    return invoke<number>("plugin_run", { name, target, package: pkg, onEvent: channel });
  },
  pluginStop: (session: number) => invoke<void>("plugin_stop", { session }),
  pluginOpenFolder: () => invoke<void>("plugin_open_folder"),
  controlsInfo: (target: Target, app: string | null) => invoke<ControlInfo>("controls_info", { target, app }),
  controlApply: (target: Target, control: Control) => invoke<void>("control_apply", { target, control }),
  traceTemplates: (target: Target) => invoke<string[]>("trace_templates", { target }),
  async traceStart(target: Target, device: string, request: TraceRequest, onEvent: (e: TraceEvent) => void): Promise<number> {
    const channel = new Channel<TraceEvent>();
    channel.onmessage = onEvent;
    return invoke<number>("trace_start", { target, device, request, onEvent: channel });
  },
  traceStop: (session: number) => invoke<void>("trace_stop", { session }),
  traceList: () => invoke<Recording[]>("trace_list"),
  traceDelete: (id: string) => invoke<void>("trace_delete", { id }),
  traceProcessorStatus: () => invoke<ProcessorStatus>("trace_processor_status"),
  async traceInstallProcessor(onProgress: (p: DownloadProgress) => void): Promise<void> {
    const channel = new Channel<DownloadProgress>();
    channel.onmessage = onProgress;
    return invoke<void>("trace_install_processor", { onProgress: channel });
  },
  traceOpen: (id: string) => invoke<Opened>("trace_open", { id }),
  traceFlame: (id: string, process: string | null, thread: string | null) =>
    invoke<CpuView>("trace_flame", { id, process, thread }),
  traceFolded: (id: string, process: string | null, thread: string | null) =>
    invoke<string>("trace_folded", { id, process, thread }),
  traceQuery: (id: string, sql: string) => invoke<Rows>("trace_query", { id, sql }),
  traceOpenPerfettoUi: (id: string) => invoke<void>("trace_open_perfetto_ui", { id }),
  traceOpenExternal: (id: string) => invoke<void>("trace_open_external", { id }),
  traceSave: (id: string) => invoke<string | null>("trace_save", { id }),
  netClear: () => invoke<void>("net_clear"),
  netFlow: (id: number) => invoke<FlowDetail>("net_flow", { id }),
  netExportHar: () => invoke<string | null>("net_export_har"),
  async startPerf(target: Target, options: PerfOptions, onSample: (s: PerfSample) => void): Promise<number> {
    const channel = new Channel<PerfSample>();
    channel.onmessage = onSample;
    return invoke<number>("start_perf", { target, options, onSample: channel });
  },
  memoryReport: (target: Target, app: string) => invoke<MemoryReport>("memory_report", { target, app }),
  memoryDiff: (before: MemoryReport, after: MemoryReport) => invoke<CategoryDelta[]>("memory_diff", { before, after }),
  saveHeapDump: (target: Target, pkg: string) => invoke<string | null>("save_heap_dump", { target, package: pkg }),
  storageRoots: (target: Target, app: string) => invoke<StorageRoot[]>("storage_roots", { target, app }),
  listFiles: (target: Target, app: string, root: string, path: string) =>
    invoke<FileEntry[]>("list_files", { target, app, root, path }),
  openFile: (target: Target, app: string, root: string, path: string) =>
    invoke<Preview>("open_file", { target, app, root, path }),
  sqliteRows: (id: number, table: string, offset: number, limit: number) =>
    invoke<Rows>("sqlite_rows", { id, table, offset, limit }),
  sqliteQuery: (id: number, sql: string) => invoke<Rows>("sqlite_query", { id, sql }),
  saveFile: (target: Target, app: string, root: string, path: string) =>
    invoke<string | null>("save_file", { target, app, root, path }),
};
