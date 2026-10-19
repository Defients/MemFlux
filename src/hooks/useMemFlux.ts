import { useCallback, useEffect, useMemo, useState } from "react";
import { native } from "../lib/native";
import type {
  ActionRecord,
  AppSettings,
  GameProfile,
  PowerShiftMode,
  RuntimeStatus,
  SettingsEnvelope,
  TelemetryFrame
} from "../types";

const MAX_HISTORY = 90;

export function useMemFlux() {
  const [frame, setFrame] = useState<TelemetryFrame | null>(null);
  const [history, setHistory] = useState<TelemetryFrame[]>([]);
  const [settingsEnvelope, setSettingsEnvelope] = useState<SettingsEnvelope | null>(null);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [purging, setPurging] = useState(false);
  const [authorizingSession, setAuthorizingSession] = useState(false);
  const [actionHistory, setActionHistory] = useState<ActionRecord[]>([]);

  const acceptAction = useCallback((record: ActionRecord) => {
    setActionHistory((current) => {
      const withoutDuplicate = current.filter((item) => item.id !== record.id);
      return [...withoutDuplicate, record].slice(-100);
    });
  }, []);

  const acceptFrame = useCallback((next: TelemetryFrame) => {
    setFrame(next);
    setHistory((current) => {
      if (current.at(-1)?.snapshot.sequence === next.snapshot.sequence) return current;
      return [...current, next].slice(-MAX_HISTORY);
    });
  }, []);

  const refreshRuntime = useCallback(async () => {
    try {
      const status = await native.runtimeStatus();
      setRuntime(status);
    } catch {
      // ignore transient poll failure
    }
  }, []);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    Promise.all([native.latestFrame(), native.settings(), native.runtimeStatus(), native.actionHistory()])
      .then(([latest, envelope, status, actions]) => {
        if (!active) return;
        if (latest) acceptFrame(latest);
        setSettingsEnvelope(envelope);
        setRuntime(status);
        setActionHistory(actions);
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

    native.onAction((record) => {
      if (active) acceptAction(record);
    }).then((unlisten) => {
      if (active) unlisteners.push(unlisten);
      else unlisten();
    }).catch(() => undefined);

    return () => {
      active = false;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [acceptAction, acceptFrame]);

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
      await refreshRuntime();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    } finally {
      setSaving(false);
    }
  }, [refreshRuntime]);

  const purgeStandby = useCallback(async () => {
    setPurging(true);
    setError(null);
    try {
      const record = await native.purgeStandby();
      acceptAction(record);
      await refreshRuntime();
      return record;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    } finally {
      setPurging(false);
    }
  }, [acceptAction, refreshRuntime]);

  const authorizeCleanerSession = useCallback(async () => {
    setAuthorizingSession(true);
    setError(null);
    try {
      const sessionStatus = await native.authorizeCleanerSession();
      await refreshRuntime();
      return sessionStatus;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    } finally {
      setAuthorizingSession(false);
    }
  }, [refreshRuntime]);

  const revokeCleanerSession = useCallback(async () => {
    setError(null);
    try {
      const sessionStatus = await native.revokeCleanerSession();
      await refreshRuntime();
      return sessionStatus;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, [refreshRuntime]);

  const applyPowerShift = useCallback(async (mode: PowerShiftMode, customGuid?: string | null, allowOnBattery?: boolean) => {
    setError(null);
    try {
      const outcome = await native.applyPowerShiftMode(mode, customGuid, allowOnBattery);
      const coord = await native.coordinatorSnapshot();
      setFrame((curr) => curr ? { ...curr, coordinator: coord } : curr);
      return outcome;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, []);

  const restoreRewindEntry = useCallback(async (actionId: string) => {
    setError(null);
    try {
      const entry = await native.restoreRewindEntry(actionId);
      const coord = await native.coordinatorSnapshot();
      setFrame((curr) => curr ? { ...curr, coordinator: coord } : curr);
      return entry;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, []);

  const restoreAllRewind = useCallback(async () => {
    setError(null);
    try {
      const entries = await native.restoreAllRewind();
      const coord = await native.coordinatorSnapshot();
      setFrame((curr) => curr ? { ...curr, coordinator: coord } : curr);
      return entries;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, []);

  const activateGameFocus = useCallback(async (profile: GameProfile, pid: number | null, executablePath: string) => {
    setError(null);
    try {
      await native.activateGameFocusSession(profile, pid, executablePath);
      const coord = await native.coordinatorSnapshot();
      setFrame((curr) => curr ? { ...curr, coordinator: coord } : curr);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, []);

  const deactivateGameFocus = useCallback(async () => {
    setError(null);
    try {
      await native.deactivateGameFocusSession();
      const coord = await native.coordinatorSnapshot();
      setFrame((curr) => curr ? { ...curr, coordinator: coord } : curr);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }, []);

  const startFpsCapture = useCallback(async (pid: number, executableName: string) => {
    setError(null);
    try {
      const status = await native.startFpsCapture(pid, executableName);
      setFrame((curr) => curr ? { ...curr, fpsCapture: status } : curr);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, []);

  const stopFpsCapture = useCallback(async () => {
    setError(null);
    try {
      const status = await native.stopFpsCapture();
      setFrame((curr) => curr ? { ...curr, fpsCapture: status } : curr);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, []);

  return useMemo(() => ({
    frame,
    history,
    settingsEnvelope,
    runtime,
    error,
    saving,
    purging,
    authorizingSession,
    actionHistory,
    setPaused,
    saveSettings,
    purgeStandby,
    authorizeCleanerSession,
    revokeCleanerSession,
    applyPowerShift,
    restoreRewindEntry,
    restoreAllRewind,
    activateGameFocus,
    deactivateGameFocus,
    startFpsCapture,
    stopFpsCapture
  }), [
    frame,
    history,
    settingsEnvelope,
    runtime,
    error,
    saving,
    purging,
    authorizingSession,
    actionHistory,
    setPaused,
    saveSettings,
    purgeStandby,
    authorizeCleanerSession,
    revokeCleanerSession,
    applyPowerShift,
    restoreRewindEntry,
    restoreAllRewind,
    activateGameFocus,
    deactivateGameFocus,
    startFpsCapture,
    stopFpsCapture
  ]);
}
