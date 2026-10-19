import { useEffect, useState } from "react";
import type { TelemetryFrame } from "../types";

export function PerformanceHud({ frame, paused }: { frame: TelemetryFrame | null; paused: boolean }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => { if (!document.hidden) setNow(Date.now()); }, 2000);
    return () => window.clearInterval(timer);
  }, []);
  const stale = paused || !frame || now - frame.snapshot.timestampMs > Math.max(5000, frame.snapshot.sampleIntervalMs * 3);
  const cpu = stale ? null : frame?.cpuUtilization.value ?? null;
  const ram = stale ? null : frame?.snapshot.physicalLoadPercent.value ?? null;
  return <details className="performance-hud" open>
    <summary aria-label="Show or hide live telemetry"><span className="hud-expanded-label">Live telemetry</span><span className="hud-compact-label">HUD</span></summary>
    <div className="hud-content">
      <HudMetric label="CPU" value={cpu} detail={stale ? "Paused or stale sample" : frame?.cpuUtilization.detail ?? frame?.cpuUtilization.source ?? "Waiting for native telemetry"} />
      <HudMetric label="GPU" value={null} detail="GPU engine collection is not implemented." />
      <HudMetric label="RAM" value={ram} detail="Physical memory load from GlobalMemoryStatusEx; standby is included in availability." />
      <div className="hud-row" title="FPS capture is not active"><span>FPS</span><strong>—</strong></div>
      <div className="hud-row" title="No measured frame-time stream"><span>Frame</span><strong>—</strong></div>
      <small className="hud-caption">{stale ? "Paused / awaiting fresh sample" : "No FPS capture active"}</small>
      <small className="hud-caption">Cleaner: {(frame?.cleanerDecision ?? "disabled").replaceAll("_", " ")}</small>
    </div>
  </details>;
}

function HudMetric({ label, value, detail }: { label: string; value: number | null; detail: string }) {
  return <div className={`hud-metric hud-${label.toLowerCase()}`} title={detail}>
    <div className="hud-row"><span>{label}</span><strong aria-label={`${label} ${value === null ? "unavailable" : `${value.toFixed(0)} percent`}`}>{value === null ? "—" : `${value.toFixed(0)}%`}</strong></div>
    <div className="hud-track" aria-hidden="true"><i style={{ width: `${value ?? 0}%` }} /></div>
  </div>;
}
