// Formatting helpers shared by the views.

export function hue(text: string): number {
  let h = 0;
  for (const c of text) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

export function compact(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

export function fileStamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

export function slug(text: string): string {
  return text.replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "") || "device";
}

export function kb(value: number | null | undefined): string {
  if (value == null) return "—";
  if (value < 1024) return `${Math.round(value)} KB`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(value < 10 * 1024 ? 1 : 0)} MB`;
  return `${(value / 1024 / 1024).toFixed(2)} GB`;
}

export function bytes(value: number): string {
  return value < 1024 ? `${value} B` : kb(value / 1024);
}

export function percent(value: number | null | undefined, digits = 1): string {
  return value == null ? "—" : `${value.toFixed(digits)}%`;
}

export function dateTime(secs: number | null): string {
  if (!secs) return "";
  return new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" });
}
