import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ActionRecord,
  AppSettings,
  CleanerSessionStatus,
  CoordinatorSnapshot,
  FpsCaptureStatus,
  GameFocusStatus,
  GameProfile,
  PowerShiftMode,
  PowerShiftOutcome,
  ProofReport,
  RewindEntry,
  RuntimeStatus,
  SettingsEnvelope,
  TelemetryFrame
} from "../types";

export const native = {
  latestFrame: () => invoke<TelemetryFrame | null>("get_latest_frame"),
  settings: () => invoke<SettingsEnvelope>("get_settings"),
  runtimeStatus: () => invoke<RuntimeStatus>("get_runtime_status"),
  actionHistory: () => invoke<ActionRecord[]>("get_action_history"),
  purgeStandby: () => invoke<ActionRecord>("purge_standby_memory"),
  authorizeCleanerSession: () => invoke<CleanerSessionStatus>("authorize_cleaner_session"),
  revokeCleanerSession: () => invoke<CleanerSessionStatus>("revoke_cleaner_session"),
  coordinatorSnapshot: () => invoke<CoordinatorSnapshot>("get_coordinator_snapshot"),
  applyPowerShiftMode: (mode: PowerShiftMode, customGuid?: string | null, allowOnBattery?: boolean) =>
    invoke<PowerShiftOutcome>("apply_powershift_mode", { mode, customGuid: customGuid ?? null, allowOnBattery }),
  restoreRewindEntry: (actionId: string) => invoke<RewindEntry>("restore_rewind_entry", { actionId }),
  restoreAllRewind: () => invoke<RewindEntry[]>("restore_all_rewind"),
  activateGameFocusSession: (profile: GameProfile, pid: number | null, executablePath: string) =>
    invoke<GameFocusStatus>("activate_gamefocus_session", { profile, pid, executablePath }),
  deactivateGameFocusSession: () => invoke<GameFocusStatus>("deactivate_gamefocus_session"),
  startFpsCapture: (pid: number, executableName: string) =>
    invoke<FpsCaptureStatus>("start_fps_capture", { pid, executableName }),
  stopFpsCapture: () => invoke<FpsCaptureStatus>("stop_fps_capture"),
  beginProofSession: () => invoke<ProofReport>("begin_proof_session"),
  markProofChange: (label?: string) => invoke<ProofReport>("mark_proof_change", { label: label ?? null }),
  proofSessionStatus: () => invoke<ProofReport>("proof_session_status"),
  endProofSession: () => invoke<ProofReport>("end_proof_session"),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings }),
  setPaused: (paused: boolean) => invoke<boolean>("set_monitoring_paused", { paused }),
  onFrame: (handler: (frame: TelemetryFrame) => void): Promise<UnlistenFn> =>
    listen<TelemetryFrame>("telemetry://frame", (event) => handler(event.payload)),
  onMonitoring: (handler: (monitoring: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("monitoring://state", (event) => handler(event.payload)),
  onAction: (handler: (record: ActionRecord) => void): Promise<UnlistenFn> =>
    listen<ActionRecord>("action://result", (event) => handler(event.payload))
};
