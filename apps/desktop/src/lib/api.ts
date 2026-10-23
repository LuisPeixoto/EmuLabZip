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
};
