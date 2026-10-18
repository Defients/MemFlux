import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSettings, RuntimeStatus, SettingsEnvelope, TelemetryFrame } from "../types";

export const native = {
  latestFrame: () => invoke<TelemetryFrame | null>("get_latest_frame"),
  settings: () => invoke<SettingsEnvelope>("get_settings"),
  runtimeStatus: () => invoke<RuntimeStatus>("get_runtime_status"),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings }),
  setPaused: (paused: boolean) => invoke<boolean>("set_monitoring_paused", { paused }),
  onFrame: (handler: (frame: TelemetryFrame) => void): Promise<UnlistenFn> =>
    listen<TelemetryFrame>("telemetry://frame", (event) => handler(event.payload)),
  onMonitoring: (handler: (monitoring: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("monitoring://state", (event) => handler(event.payload))
};

