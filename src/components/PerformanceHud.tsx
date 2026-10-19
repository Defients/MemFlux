import { useEffect, useState } from "react";
import type { AppSettings, TelemetryFrame } from "../types";
import { freshValue, frameFresh } from "../lib/freshness";

export function PerformanceHud({
  frame,
  paused,
  settings
}: {
  frame: TelemetryFrame | null;
  paused: boolean;
  settings?: Partial<AppSettings> | undefined;
}) {
  const [now, setNow] = useState(Date.now());
  const currentTime = Math.max(now, Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => {
      if (!document.hidden) setNow(Date.now());
    }, 2000);
    return () => window.clearInterval(timer);
  }, []);

  const stale = !frameFresh(frame, paused, currentTime);

  const cpu = stale ? null : freshValue(frame?.cpuUtilization, currentTime);
  const gpuMetric = frame?.gpuTelemetry?.utilization ?? null;
  const gpu = stale ? null : freshValue(gpuMetric, currentTime);
  const ram = stale ? null : freshValue(frame?.snapshot.physicalLoadPercent, currentTime);

  const fpsMetric = frame?.fpsCapture?.fps ?? null;
  const frameTimeMetric = frame?.fpsCapture?.frameTimeMs ?? null;
  const fpsVal = stale ? null : freshValue(fpsMetric, currentTime);
  const frameMsVal = stale ? null : freshValue(frameTimeMetric, currentTime);

  const showCpu = settings?.hudShowCpu ?? true;
  const showGpu = settings?.hudShowGpu ?? true;
  const showRam = settings?.hudShowRam ?? true;
  const showFps = settings?.hudShowFps ?? true;
  const showFrameTime = settings?.hudShowFrameTime ?? true;
  const showCleaner = settings?.hudShowCleanerState ?? true;
  const detailed = settings?.hudDetailLevel === "detailed";

  const gpuDetail = stale
    ? "Paused or stale sample"
    : gpuMetric?.detail ?? gpuMetric?.source ?? "GPU engine collection is not implemented.";

  const captureCaption = stale
    ? "Paused / awaiting fresh sample"
    : frame?.fpsCapture?.active
      ? frame.fpsCapture.statusDetail
      : "No FPS capture active";

  const cleanerLabel =
    frame?.cleanerStatus?.label ?? (frame?.cleanerDecision ?? "disabled").replaceAll("_", " ");

  return (
    <details className={`performance-hud ${settings?.hudCompactMode ? "is-compact-hud" : ""}`} open>
      <summary aria-label="Show or hide live telemetry">
        <span className="hud-expanded-label">Live telemetry</span>
        <span className="hud-compact-label">HUD</span>
      </summary>
      <div className="hud-content">
        {showCpu && (
          <HudMetric
            label="CPU"
            value={cpu}
            detail={
              stale
                ? "Paused or stale sample"
                : frame?.cpuUtilization.detail ?? frame?.cpuUtilization.source ?? "Waiting for native telemetry"
            }
          />
        )}
        {showGpu && <HudMetric label="GPU" value={gpu} detail={gpuDetail} />}
        {showRam && (
          <HudMetric
            label="RAM"
            value={ram}
            detail="Physical memory load from GlobalMemoryStatusEx; standby is included in availability."
          />
        )}
        {detailed && frame?.gpuTelemetry?.selectedAdapter && (
          <small className="hud-caption">
            {frame.gpuTelemetry.selectedAdapter.name} ({frame.gpuTelemetry.engineBreakdown?.busiestEngineType ?? "3D"})
          </small>
        )}
        {showFps && (
          <div className="hud-row" title={fpsMetric?.detail ?? "FPS capture is not active"}>
            <span>FPS</span>
            <strong aria-label={`FPS ${fpsVal === null ? "unavailable" : `${fpsVal.toFixed(0)}`}`}>
              {fpsVal === null ? "—" : `${fpsVal.toFixed(0)}`}
            </strong>
          </div>
        )}
        {showFrameTime && (
          <div className="hud-row" title={frameTimeMetric?.detail ?? "No measured frame-time stream"}>
            <span>Frame</span>
            <strong aria-label={`Frame time ${frameMsVal === null ? "unavailable" : `${frameMsVal.toFixed(1)} ms`}`}>
              {frameMsVal === null ? "—" : `${frameMsVal.toFixed(1)} ms`}
            </strong>
          </div>
        )}
        <small className="hud-caption">{captureCaption}</small>
        {showCleaner && <small className="hud-caption">Cleaner: {cleanerLabel}</small>}
      </div>
    </details>
  );
}

function HudMetric({ label, value, detail }: { label: string; value: number | null; detail: string }) {
  return (
    <div className={`hud-metric hud-${label.toLowerCase()}`} title={detail}>
      <div className="hud-row">
        <span>{label}</span>
        <strong aria-label={`${label} ${value === null ? "unavailable" : `${value.toFixed(0)} percent`}`}>
          {value === null ? "—" : `${value.toFixed(0)}%`}
        </strong>
      </div>
      <div className="hud-track" aria-hidden="true">
        <i style={{ width: `${value ?? 0}%` }} />
      </div>
    </div>
  );
}
