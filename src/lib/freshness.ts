import type { Metric, TelemetryFrame } from "../types";

export function freshValue(metric: Metric | null | undefined, now: number): number | null {
  if (!metric || metric.status !== "available" || metric.value === null || !Number.isFinite(metric.value) || metric.value < 0) return null;
  const age = now - metric.timestampMs;
  return age >= 0 && age <= Math.max(3500, metric.sampleIntervalMs * 3) ? metric.value : null;
}

export function frameFresh(frame: TelemetryFrame | null, paused: boolean, now: number): boolean {
  return !paused && !!frame && frame.snapshot.collectionStatus !== "paused" && now >= frame.snapshot.timestampMs
    && now - frame.snapshot.timestampMs <= Math.max(5000, frame.snapshot.sampleIntervalMs * 3);
}
