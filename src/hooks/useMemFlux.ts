import { useCallback, useEffect, useMemo, useState } from "react";
import { native } from "../lib/native";
import type { AppSettings, RuntimeStatus, SettingsEnvelope, TelemetryFrame } from "../types";

const MAX_HISTORY = 90;

export function useMemFlux() {
  const [frame, setFrame] = useState<TelemetryFrame | null>(null);
  const [history, setHistory] = useState<TelemetryFrame[]>([]);
  const [settingsEnvelope, setSettingsEnvelope] = useState<SettingsEnvelope | null>(null);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const acceptFrame = useCallback((next: TelemetryFrame) => {
    setFrame(next);
    setHistory((current) => {
      if (current.at(-1)?.snapshot.sequence === next.snapshot.sequence) return current;
      return [...current, next].slice(-MAX_HISTORY);
    });
  }, []);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    Promise.all([native.latestFrame(), native.settings(), native.runtimeStatus()])
      .then(([latest, envelope, status]) => {
        if (!active) return;
        if (latest) acceptFrame(latest);
        setSettingsEnvelope(envelope);
        setRuntime(status);
      })
      .catch((reason: unknown) => {
        if (active) setError(reason instanceof Error ? reason.message : String(reason));
      });

    native.onFrame((next) => {
      if (active) acceptFrame(next);
    }).then((unlisten) => {
      if (active) unlisteners.push(unlisten);
      else unlisten();
    }).catch((reason: unknown) => {
      if (active) setError(reason instanceof Error ? reason.message : String(reason));
    });

    native.onMonitoring((monitoring) => {
      if (active) setRuntime((current) => current ? { ...current, paused: !monitoring } : current);
    }).then((unlisten) => {
      if (active) unlisteners.push(unlisten);
      else unlisten();
    }).catch(() => undefined);

    return () => {
      active = false;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [acceptFrame]);

  const setPaused = useCallback(async (paused: boolean) => {
    setError(null);
    try {
      const actual = await native.setPaused(paused);
      setRuntime((current) => current ? { ...current, paused: actual } : current);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, []);

  const saveSettings = useCallback(async (settings: AppSettings) => {
    setSaving(true);
    setError(null);
    try {
      const saved = await native.updateSettings(settings);
      setSettingsEnvelope((current) => ({
        settings: saved,
        warning: null,
        path: current?.path ?? ""
      }));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    } finally {
      setSaving(false);
    }
  }, []);

  return useMemo(() => ({
    frame,
    history,
    settingsEnvelope,
    runtime,
    error,
    saving,
    setPaused,
    saveSettings
  }), [frame, history, settingsEnvelope, runtime, error, saving, setPaused, saveSettings]);
}

