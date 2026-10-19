import { useCallback, useEffect, useState } from "react";
import { native } from "../lib/native";
import { formatBytes, formatPercent } from "../lib/format";
import type { AppSettings, TelemetryFrame } from "../types";
import { freshValue, frameFresh } from "../lib/freshness";

/**
 * Independent floating Mini HUD. Runs inside the `minihud` webview window and
 * shares the native telemetry sampler — it consumes `telemetry://frame` events
 * the backend only emits while some UI surface is visible.
 */
export function MiniHud() {
  const [frame, setFrame] = useState<TelemetryFrame | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [paused, setPaused] = useState(false);
  const [now, setNow] = useState(Date.now());
  const currentTime = Math.max(now, Date.now());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    let active = true;
    let monitoringUpdated = false;
    const unlisteners: Array<() => void> = [];
    Promise.all([native.latestFrame(), native.settings(), native.runtimeStatus()])
      .then(([latest, envelope, runtime]) => {
        if (!active) return;
        if (latest) setFrame((current) => current ?? latest);
        setSettings((current) => current ?? envelope.settings);
        if (!monitoringUpdated) setPaused(runtime.paused);
      })
      .catch(() => undefined);
    native.onMonitoring((monitoring) => { monitoringUpdated = true; if (active) setPaused(!monitoring); })
      .then((unlisten) => { if (active) unlisteners.push(unlisten); else unlisten(); }).catch(() => undefined);
    native.onSettings((next) => { if (active) setSettings(next); })
      .then((unlisten) => { if (active) unlisteners.push(unlisten); else unlisten(); }).catch(() => undefined);
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
    void native.disableMiniHud().catch((reason: unknown) => setError(String(reason)));
  }, []);

  const preset = settings?.miniHudPreset ?? "compact";
  const opacity = ((settings?.miniHudOpacityPercent ?? 92) / 100).toFixed(2);

  const fresh = frameFresh(frame, paused, currentTime);
  const metric = (value: Parameters<typeof freshValue>[0]) => fresh ? freshValue(value, currentTime) : null;
  const cpu = metric(frame?.cpuUtilization);
  const gpu = metric(frame?.gpuTelemetry?.utilization);
  const total = metric(frame?.snapshot.totalPhysicalBytes);
  const available = metric(frame?.snapshot.availablePhysicalBytes);
  const ramUsed =
    total !== null && available !== null && available <= total
      ? total - available
      : null;
  const fps = metric(frame?.fpsCapture?.fps);
  const frameMs = metric(frame?.fpsCapture?.frameTimeMs);
  const p99 = frameMs === null ? null : frame?.fpsCapture?.frameTimeP99Ms ?? null;
  const cleanerState = fresh ? frame?.cleanerStatus?.label ?? "—" : "Paused / awaiting fresh data";

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
      {(!fresh || preset === "detailed") && <div className="minihud-footer">{cleanerState}</div>}
      {error && <div role="alert">{error}</div>}
    </div>
  );
}
