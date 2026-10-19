import type { Metric, MetricStatus, PressureLevel } from "../types";

const UNAVAILABLE = "Unavailable";

export function formatBytes(value: number | null, decimals = 1): string {
  if (value === null || !Number.isFinite(value) || value < 0) return UNAVAILABLE;
  if (value === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  const scaled = value / 1024 ** exponent;
  return `${scaled.toFixed(exponent === 0 ? 0 : decimals)} ${units[exponent]}`;
}

export function formatPercent(value: number | null, decimals = 0): string {
  if (value === null || !Number.isFinite(value)) return UNAVAILABLE;
  return `${value.toFixed(decimals)}%`;
}

export function formatRate(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return UNAVAILABLE;
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} /s`;
}

export function formatCount(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return UNAVAILABLE;
  return Math.round(value).toLocaleString();
}

export function formatMetric(metric: Metric): string {
  if (metric.status !== "available") return metric.status === "warming_up" ? "Warming up" : UNAVAILABLE;
  if (metric.unit === "bytes") return formatBytes(metric.value);
  if (metric.unit === "percent") return formatPercent(metric.value, 1);
  if (metric.unit === "reads_per_second") return formatRate(metric.value);
  if (metric.unit === "count") return formatCount(metric.value);
  return metric.value?.toLocaleString() ?? UNAVAILABLE;
}

const TIMESTAMP_FORMATTER = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
  second: "2-digit"
});

export function formatTimestamp(timestampMs: number): string {
  return TIMESTAMP_FORMATTER.format(timestampMs);
}

export function statusLabel(status: MetricStatus): string {
  return status.replace("_", " ");
}

export function pressureLabel(pressure: PressureLevel): string {
  return pressure.charAt(0).toUpperCase() + pressure.slice(1);
}

export function availablePercent(frame: { snapshot: { availablePhysicalBytes: Metric; totalPhysicalBytes: Metric } }): number | null {
  const available = frame.snapshot.availablePhysicalBytes.value;
  const total = frame.snapshot.totalPhysicalBytes.value;
  if (available === null || total === null || total <= 0) return null;
  return Math.max(0, Math.min(100, (available / total) * 100));
}

