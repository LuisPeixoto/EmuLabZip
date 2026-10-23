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
export type FlowState = "pending" | "completed" | "failed";

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
  status: number | null;
  contentType: string | null;
  requestSize: number;
  responseSize: number;
  durationMs: number | null;
  error: string | null;
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

export interface Flow extends Omit<FlowSummary, "contentType" | "durationMs"> {
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

export function targetOf(d: Device): Target {
  return { platform: d.platform, id: d.id };
}

export function osLabel(d: Device): string {
  const name = d.platform === "android" ? "Android" : "iOS";
  if (d.osVersion && d.apiLevel) return `${name} ${d.osVersion} · API ${d.apiLevel}`;
  if (d.osVersion) return `${name} ${d.osVersion}`;
  return name;
}

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
