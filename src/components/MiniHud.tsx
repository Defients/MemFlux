import { useCallback, useEffect, useState } from "react";
import { native } from "../lib/native";
import { formatBytes, formatPercent } from "../lib/format";
import type { AppSettings, TelemetryFrame } from "../types";

/**
 * Independent floating Mini HUD. Runs inside the `minihud` webview window and
 * shares the native telemetry sampler — it consumes `telemetry://frame` events
 * the backend only emits while some UI surface is visible.
 */
export function MiniHud() {
  const [frame, setFrame] = useState<TelemetryFrame | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];
    Promise.all([native.latestFrame(), native.settings()])
      .then(([latest, envelope]) => {
        if (!active) return;
        if (latest) setFrame(latest);
        setSettings(envelope.settings);
      })
      .catch(() => undefined);
    native
      .onFrame((next) => {
        if (active) setFrame(next);
      })
      .then((unlisten) => {
        if (active) unlisteners.push(unlisten);
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  const close = useCallback(() => {
    if (!settings) return;
    const next: AppSettings = { ...settings, miniHudEnabled: false };
    void native.updateSettings(next).catch(() => undefined);
  }, [settings]);

  const preset = settings?.miniHudPreset ?? "compact";
  const opacity = ((settings?.miniHudOpacityPercent ?? 92) / 100).toFixed(2);

  const cpu = frame?.cpuUtilization.value ?? null;
  const gpu = frame?.gpuTelemetry?.utilization.value ?? null;
  const ramUsed =
    frame && frame.snapshot.totalPhysicalBytes.value !== null && frame.snapshot.availablePhysicalBytes.value !== null
      ? frame.snapshot.totalPhysicalBytes.value - frame.snapshot.availablePhysicalBytes.value
      : null;
  const fps = frame?.fpsCapture?.fps.value ?? null;
  const frameMs = frame?.fpsCapture?.frameTimeMs.value ?? null;
  const p99 = frame?.fpsCapture?.frameTimeP99Ms ?? null;
  const cleanerState = frame?.cleanerStatus?.label ?? "—";

  return (
    <div className="minihud-root" style={{ opacity }}>
      <div className="minihud-header" data-tauri-drag-region>
        <span className="minihud-title" data-tauri-drag-region>
          MemFlux
        </span>
        <button
          className="minihud-close"
          onClick={close}
          title="Disable Mini HUD"
          aria-label="Close Mini HUD"
        >
          ×
        </button>
      </div>
      <div className={`minihud-grid preset-${preset}`}>
        <div className="minihud-cell">
          <span>CPU</span>
          <strong>{cpu === null ? "—" : formatPercent(cpu, 0)}</strong>
        </div>
        {preset !== "minimal" && (
          <div className="minihud-cell">
            <span>GPU</span>
            <strong>{gpu === null ? "—" : formatPercent(gpu, 0)}</strong>
          </div>
        )}
        {preset !== "minimal" && (
          <div className="minihud-cell">
            <span>RAM</span>
            <strong>{ramUsed === null ? "—" : formatBytes(ramUsed, 0)}</strong>
          </div>
        )}
        <div className="minihud-cell">
          <span>FPS</span>
          <strong>{fps === null ? "—" : fps.toFixed(0)}</strong>
        </div>
        {preset === "detailed" && (
          <div className="minihud-cell">
            <span>Frame</span>
            <strong>{frameMs === null ? "—" : `${frameMs.toFixed(1)} ms`}</strong>
          </div>
        )}
        {preset === "detailed" && (
          <div className="minihud-cell">
            <span>P99</span>
            <strong>{p99 === null ? "—" : `${p99.toFixed(1)} ms`}</strong>
          </div>
        )}
      </div>
      {preset === "detailed" && <div className="minihud-footer">{cleanerState}</div>}
    </div>
  );
}
