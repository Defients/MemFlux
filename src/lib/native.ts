import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ActionRecord, AppSettings, RuntimeStatus, SettingsEnvelope, TelemetryFrame } from "../types";

export const native = {
  latestFrame: () => invoke<TelemetryFrame | null>("get_latest_frame"),
  settings: () => invoke<SettingsEnvelope>("get_settings"),
  runtimeStatus: () => invoke<RuntimeStatus>("get_runtime_status"),
  actionHistory: () => invoke<ActionRecord[]>("get_action_history"),
  purgeStandby: () => invoke<ActionRecord>("purge_standby_memory"),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings }),
  setPaused: (paused: boolean) => invoke<boolean>("set_monitoring_paused", { paused }),
  onFrame: (handler: (frame: TelemetryFrame) => void): Promise<UnlistenFn> =>
    listen<TelemetryFrame>("telemetry://frame", (event) => handler(event.payload)),
  onMonitoring: (handler: (monitoring: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("monitoring://state", (event) => handler(event.payload)),
  onAction: (handler: (record: ActionRecord) => void): Promise<UnlistenFn> =>
    listen<ActionRecord>("action://result", (event) => handler(event.payload))
};

