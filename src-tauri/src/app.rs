use crate::action::{self, ActionCapability, CleanerSessionStatus, OPERATION_PURGE_STANDBY};
use crate::cleaner::{CleanerDecision, CleanerInputs, CleanerPolicy, CleanerStatusSummary};
use crate::coordinator::{CoordinatorSnapshot, OptimizationCoordinator};
use crate::gamefocus::{GameFocusStatus, GameProfile};
use crate::guardian::{GuardianDecision, GuardianEvaluator};
use crate::performance::{
    CpuSensor, FpsCaptureStatus, GpuSensor, GpuTelemetry, PresentMonController,
};
use crate::powershift::{PowerShiftMode, PowerShiftOutcome};
use crate::proof::{ProofReport, ProofSample, ProofSession};
use crate::rewind::RewindEntry;
use crate::settings::{AppSettings, SettingsEnvelope, SettingsStore};
use crate::telemetry::{Collector, MemorySnapshot, PlatformCollector};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock, TryLockError};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, RunEvent, State, WindowEvent};

const TRAY_ID: &str = "memflux-tray";
const EVENT_FRAME: &str = "telemetry://frame";
const EVENT_MONITORING: &str = "monitoring://state";
const EVENT_ACTION: &str = "action://result";
const ACTION_HISTORY_LIMIT: usize = 100;
static ACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn action_id(origin: &str, timestamp: u64) -> String {
    format!(
        "{origin}-{timestamp}-{}",
        ACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    pub atlas: crate::atlas::AtlasReport,
    pub cpu_utilization: crate::telemetry::Metric,
    pub gpu_telemetry: GpuTelemetry,
    pub fps_capture: FpsCaptureStatus,
    pub snapshot: MemorySnapshot,
    pub guardian: GuardianDecision,
    pub cleaner_decision: CleanerDecision,
    pub cleaner_status: CleanerStatusSummary,
    pub coordinator: CoordinatorSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    paused: bool,
    cleaner_paused: bool,
    cleaner_decision: CleanerDecision,
    cleaner_status: CleanerStatusSummary,
    cleaner_session: CleanerSessionStatus,
    automatic_execution_ready: bool,
    action_capability: ActionCapability,
    shutting_down: bool,
    shutdown_phase: ShutdownPhase,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionMeasurement {
    standby_bytes: Option<f64>,
    free_zero_bytes: Option<f64>,
    available_bytes: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRecord {
    id: String,
    timestamp_ms: u64,
    origin: String,
    operation: String,
    outcome: String,
    native_request_succeeded: bool,
    native_lifecycle: String,
    elevation_used: bool,
    late_completion_possible: bool,
    nt_status_hex: Option<String>,
    before: Option<ActionMeasurement>,
    after: Option<ActionMeasurement>,
    observed_reduction_bytes: Option<f64>,
    duration_ms: u64,
    native_duration_ms: u64,
    reason: String,
}

/// Ordered shutdown lifecycle. Logged and exposed through runtime status so a
/// stalled phase is diagnosable instead of a silent hang.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownPhase {
    Running,
    ShutdownRequested,
    DrainingActions,
    StoppingCapture,
    RestoringSystemState,
    StoppingPrivilegedHelper,
    FlushingPersistence,
    ShutdownComplete,
    ShutdownDegraded,
}

struct ControlState {
    settings: AppSettings,
    paused: bool,
    shutdown: bool,
    shutdown_phase: ShutdownPhase,
}

/// Poll a mutex until it is acquired or the deadline expires. Poisoned locks
/// are recovered (consistent with the rest of the codebase); contention is the
/// only thing that can starve shutdown work.
fn lock_with_timeout<'a, T>(mutex: &'a Mutex<T>, deadline: Duration) -> Option<MutexGuard<'a, T>> {
    let started = Instant::now();
    loop {
        match mutex.try_lock() {
            Ok(guard) => return Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {
                if started.elapsed() >= deadline {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

pub struct AppState {
    control: Mutex<ControlState>,
    wake: Condvar,
    cleaner_wake: Condvar,
    latest: RwLock<Option<TelemetryFrame>>,
    settings_store: SettingsStore,
    settings_warning: Mutex<Option<String>>,
    action_lock: Mutex<()>,
    action_history: Mutex<VecDeque<ActionRecord>>,
    action_history_path: PathBuf,
    cleaner_quota_path: PathBuf,
    cleaner_policy: Mutex<CleanerPolicy>,
    cleaner_decision: RwLock<CleanerDecision>,
    cleaner_status: RwLock<CleanerStatusSummary>,
    coordinator: Mutex<OptimizationCoordinator>,
    presentmon: Mutex<PresentMonController>,
    proof: Mutex<ProofSession>,
    proof_active: AtomicBool,
    shutdown_started: AtomicBool,
    /// (last persisted position, last persist instant) for Mini HUD moves.
    mini_hud_pending: Mutex<(Option<(i32, i32)>, Instant)>,
}

impl AppState {
    fn edit_settings(&self, edit: impl FnOnce(&mut AppSettings)) -> Result<AppSettings, String> {
        let mut control = self.control.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = control.settings.clone();
        edit(&mut next);
        let saved = self.settings_store.save(&next)?;
        control.settings = saved.clone();
        Ok(saved)
    }

    fn new(
        store: SettingsStore,
        envelope: SettingsEnvelope,
        action_history_path: PathBuf,
        cleaner_quota_path: PathBuf,
        rewind_path: PathBuf,
    ) -> Self {
        let action_history = load_action_history(&action_history_path);
        let mut cleaner_policy = CleanerPolicy::default();
        cleaner_policy.load_quota_file(
            &cleaner_quota_path,
            action_history
                .iter()
                .filter(|record| record.native_request_succeeded)
                .map(|record| record.timestamp_ms.saturating_add(record.duration_ms)),
        );
        let now_ms = crate::telemetry::timestamp_ms();
        let coordinator = OptimizationCoordinator::new(rewind_path, now_ms);

        Self {
            control: Mutex::new(ControlState {
                settings: envelope.settings,
                paused: false,
                shutdown: false,
                shutdown_phase: ShutdownPhase::Running,
            }),
            wake: Condvar::new(),
            cleaner_wake: Condvar::new(),
            latest: RwLock::new(None),
            settings_store: store,
            settings_warning: Mutex::new(envelope.warning),
            action_lock: Mutex::new(()),
            action_history: Mutex::new(action_history),
            action_history_path,
            cleaner_quota_path,
            cleaner_policy: Mutex::new(cleaner_policy),
            cleaner_decision: RwLock::new(CleanerDecision::Disabled),
            cleaner_status: RwLock::new(CleanerStatusSummary::default()),
            coordinator: Mutex::new(coordinator),
            presentmon: Mutex::new(PresentMonController::new()),
            proof: Mutex::new(ProofSession::new()),
            proof_active: AtomicBool::new(false),
            shutdown_started: AtomicBool::new(false),
            mini_hud_pending: Mutex::new((None, Instant::now())),
        }
    }

    fn is_shutting_down(&self) -> bool {
        self.control
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .shutdown
    }

    fn set_shutdown_phase(&self, phase: ShutdownPhase) {
        if let Ok(mut control) = self.control.lock() {
            control.shutdown_phase = phase;
        }
        log::info!("Shutdown phase: {phase:?}");
    }

    /// Ordered, single-flight shutdown sequence:
    /// refuse new actions -> drain in-flight action -> stop capture ->
    /// restore system state (GameFocus + Rewind journal) -> revoke the
    /// privileged helper -> final persistence flush.
    ///
    /// Every step is bounded; a stuck subsystem degrades the shutdown instead
    /// of hanging the process, and unresolved restoration obligations remain in
    /// the journal for startup recovery on next launch.
    fn execute_shutdown(&self, _app: &tauri::AppHandle) {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut degraded_notes: Vec<String> = Vec::new();
        {
            let mut control = self
                .control
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            control.shutdown = true;
            control.shutdown_phase = ShutdownPhase::ShutdownRequested;
        }
        self.wake.notify_all();
        self.cleaner_wake.notify_all();
        log::info!("MemFlux shutdown requested; coordinated teardown starting.");

        // 1) Wait briefly for an in-flight native action to finish so we do not
        //    race its post-measurement or quota accounting.
        self.set_shutdown_phase(ShutdownPhase::DrainingActions);
        let drain_deadline = Instant::now() + Duration::from_secs(3);
        let mut action_drained = false;
        while Instant::now() < drain_deadline {
            if self.action_lock.try_lock().is_ok() {
                action_drained = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !action_drained {
            let note = "A memory action was still running; shutdown continued and \
                        startup recovery remains armed."
                .to_string();
            log::warn!("{note}");
            degraded_notes.push(note);
        }

        // 2) Stop frame capture so the helper process and ETW session close.
        self.set_shutdown_phase(ShutdownPhase::StoppingCapture);
        match lock_with_timeout(&self.presentmon, Duration::from_millis(750)) {
            Some(mut pm) => pm.stop_capture(),
            None => {
                let note = "PresentMon controller was busy during shutdown; capture stop skipped."
                    .to_string();
                log::warn!("{note}");
                degraded_notes.push(note);
            }
        }

        // 3) Restore system state: close GameFocus session, then reconcile all
        //    journal restoration obligations against live Windows state.
        self.set_shutdown_phase(ShutdownPhase::RestoringSystemState);
        match lock_with_timeout(&self.coordinator, Duration::from_secs(3)) {
            Some(mut coord) => {
                let report = coord.shutdown_and_restore(crate::telemetry::timestamp_ms());
                log::info!(
                    "Shutdown restoration: {} restored, {} external preserved, {} unresolved, gamefocus_closed={}.",
                    report.restored,
                    report.external_preserved,
                    report.still_unresolved.len(),
                    report.gamefocus_session_closed
                );
                degraded_notes.extend(report.notes);
            }
            None => {
                let note = "Coordinator was busy during shutdown; journal obligations \
                            deferred to startup recovery."
                    .to_string();
                log::warn!("{note}");
                degraded_notes.push(note);
            }
        }

        // 4) Revoke the privileged cleaner session (internally bounded IPC).
        self.set_shutdown_phase(ShutdownPhase::StoppingPrivilegedHelper);
        let revoked = action::revoke_cleaner_session();
        if revoked.authorized {
            let note = "Cleaner session helper did not confirm revocation before its \
                        timeout; its controller watchdog will end it on process exit."
                .to_string();
            log::warn!("{note}");
            degraded_notes.push(note);
        }

        // 5) Final persistence flush: bounded action history + any pending
        //    Mini HUD position not yet throttled to disk.
        self.proof_active.store(false, Ordering::Relaxed);

        self.set_shutdown_phase(ShutdownPhase::FlushingPersistence);
        if let Ok(history) = self.action_history.lock()
            && let Err(error) = persist_action_history(&self.action_history_path, &history)
        {
            log::warn!("Final action-history flush failed: {error}");
        }
        if let Ok(mut pending) = self.mini_hud_pending.lock()
            && let Some((x, y)) = pending.0
        {
            if let Err(error) = self.edit_settings(|settings| {
                settings.mini_hud_x = Some(x);
                settings.mini_hud_y = Some(y);
            }) {
                log::warn!("Final Mini HUD position flush failed: {error}");
            }
            pending.0 = None;
        }

        let phase = if degraded_notes.is_empty() {
            ShutdownPhase::ShutdownComplete
        } else {
            for note in &degraded_notes {
                log::warn!("Shutdown degraded: {note}");
            }
            ShutdownPhase::ShutdownDegraded
        };
        self.set_shutdown_phase(phase);
        log::info!("MemFlux shutdown lifecycle finished ({phase:?}).");
    }

    fn toggle_paused(&self) -> bool {
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        control.paused = !control.paused;
        let paused = control.paused;
        self.wake.notify_all();
        self.cleaner_wake.notify_all();
        paused
    }
}

#[tauri::command]
fn get_latest_frame(state: State<'_, Arc<AppState>>) -> Option<TelemetryFrame> {
    state
        .latest
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<AppState>>) -> SettingsEnvelope {
    let settings = state
        .control
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .settings
        .clone();
    let warning = state
        .settings_warning
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    SettingsEnvelope {
        settings,
        warning,
        path: state.settings_store.path().display().to_string(),
    }
}

#[tauri::command]
fn update_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
    expected_settings: AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<AppSettings, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; settings changes are refused.".into());
    }
    let persisted = {
        let mut control = state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let persisted =
            state
                .settings_store
                .save_checked(&control.settings, &expected_settings, &settings)?;
        control.settings = persisted.clone();
        persisted
    };
    *state
        .settings_warning
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = None;
    sync_mini_hud(&app, &persisted);
    let _ = app.emit("settings://changed", &persisted);
    state.wake.notify_all();
    state.cleaner_wake.notify_all();
    log::info!("Settings updated and persisted.");
    Ok(persisted)
}

#[tauri::command]
fn disable_mini_hud(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down.".into());
    }
    let settings = state.edit_settings(|next| next.mini_hud_enabled = false)?;
    sync_mini_hud(&app, &settings);
    let _ = app.emit("settings://changed", &settings);
    state.wake.notify_all();
    Ok(())
}

#[tauri::command]
fn set_monitoring_paused(
    paused: bool,
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> bool {
    {
        let mut control = state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        control.paused = paused;
    }
    state.wake.notify_all();
    state.cleaner_wake.notify_all();
    let _ = app.emit(EVENT_MONITORING, !paused);
    update_tray_tooltip(&app, paused, None);
    log::info!(
        "Monitoring {} by user request.",
        if paused { "paused" } else { "resumed" }
    );
    paused
}

#[tauri::command]
fn get_runtime_status(state: State<'_, Arc<AppState>>) -> RuntimeStatus {
    let (paused, cleaner_paused, shutting_down, shutdown_phase) = {
        let control = state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (
            control.paused,
            control.settings.cleaner_paused,
            control.shutdown,
            control.shutdown_phase,
        )
    };
    let cleaner_session = action::cleaner_session_status();
    let action_capability = action::capability();
    let automatic_execution_ready = cleaner_session.authorized && action_capability.supported;
    RuntimeStatus {
        paused,
        cleaner_paused,
        cleaner_decision: state
            .cleaner_decision
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        cleaner_status: state
            .cleaner_status
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        cleaner_session,
        automatic_execution_ready,
        action_capability,
        shutting_down,
        shutdown_phase,
    }
}

#[tauri::command]
fn get_action_history(state: State<'_, Arc<AppState>>) -> Vec<ActionRecord> {
    state
        .action_history
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .cloned()
        .collect()
}

#[tauri::command]
async fn purge_standby_memory(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ActionRecord, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; new actions are refused.".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || run_standby_purge(&app, &state, "manual"))
        .await
        .map_err(|error| format!("Standby purge worker failed: {error}"))
}

#[tauri::command]
async fn authorize_cleaner_session(
    state: State<'_, Arc<AppState>>,
) -> Result<CleanerSessionStatus, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; session authorization is refused.".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = action::authorize_cleaner_session();
        state.cleaner_wake.notify_all();
        state.wake.notify_all();
        status
    })
    .await
    .map_err(|error| format!("Cleaner session authorization worker failed: {error}"))
}

#[tauri::command]
fn revoke_cleaner_session(state: State<'_, Arc<AppState>>) -> CleanerSessionStatus {
    let status = action::revoke_cleaner_session();
    state.cleaner_wake.notify_all();
    state.wake.notify_all();
    status
}

#[tauri::command]
fn get_coordinator_snapshot(state: State<'_, Arc<AppState>>) -> CoordinatorSnapshot {
    state
        .coordinator
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .snapshot()
}

#[tauri::command]
fn apply_powershift_mode(
    mode: PowerShiftMode,
    custom_guid: Option<String>,
    allow_on_battery: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<PowerShiftOutcome, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; new changes are refused.".into());
    }
    let battery_allowed = allow_on_battery.unwrap_or_else(|| {
        state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .settings
            .powershift_allow_on_battery
    });
    let now_ms = crate::telemetry::timestamp_ms();
    let mut coord = state
        .coordinator
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    coord.apply_manual_powershift(mode, custom_guid.as_deref(), battery_allowed, now_ms)
}

#[tauri::command]
fn restore_rewind_entry(
    action_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<RewindEntry, String> {
    if state.is_shutting_down() {
        return Err(
            "MemFlux is shutting down; restoration runs under the shutdown sequence.".into(),
        );
    }
    let now_ms = crate::telemetry::timestamp_ms();
    let mut coord = state
        .coordinator
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    coord.restore_rewind_entry(&action_id, now_ms)
}

#[tauri::command]
fn restore_all_rewind(state: State<'_, Arc<AppState>>) -> Vec<RewindEntry> {
    if state.is_shutting_down() {
        return Vec::new();
    }
    let now_ms = crate::telemetry::timestamp_ms();
    let mut coord = state
        .coordinator
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    coord.restore_all_rewind(now_ms)
}

#[tauri::command]
fn activate_gamefocus_session(
    profile: GameProfile,
    pid: Option<u32>,
    executable_path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<GameFocusStatus, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; new sessions are refused.".into());
    }
    let settings = state
        .control
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .settings
        .clone();
    let now_ms = crate::telemetry::timestamp_ms();
    let status = {
        let mut coord = state
            .coordinator
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        coord.activate_gamefocus_session(
            profile.clone(),
            pid,
            executable_path.clone(),
            settings.powershift_allow_on_battery,
            now_ms,
        )?
    };
    if profile.auto_fps_capture
        && settings.fps_capture_permitted
        && let Some(target_pid) = pid
        && let Ok(mut pm) = state.presentmon.lock()
    {
        let _ = pm.start_capture(target_pid, executable_path, true);
    }
    state.cleaner_wake.notify_all();
    Ok(status)
}

#[tauri::command]
fn deactivate_gamefocus_session(state: State<'_, Arc<AppState>>) -> GameFocusStatus {
    let now_ms = crate::telemetry::timestamp_ms();
    let status = {
        let mut coord = state
            .coordinator
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        coord.deactivate_gamefocus_and_restore("Deactivated by user request.", now_ms)
    };
    if let Ok(mut pm) = state.presentmon.lock() {
        pm.stop_capture();
    }
    state.cleaner_wake.notify_all();
    status
}

#[tauri::command]
fn start_fps_capture(
    pid: u32,
    executable_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<FpsCaptureStatus, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; capture cannot start.".into());
    }
    let permitted = state
        .control
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .settings
        .fps_capture_permitted;
    let mut pm = state
        .presentmon
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    pm.start_capture(pid, executable_name, permitted)
}

#[tauri::command]
fn stop_fps_capture(state: State<'_, Arc<AppState>>) -> FpsCaptureStatus {
    let permitted = state
        .control
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .settings
        .fps_capture_permitted;
    let mut pm = state
        .presentmon
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    pm.stop_capture();
    pm.status(1_000, permitted)
}

// ── V4.2 Performance Proof ───────────────────────────────────────────────────

#[tauri::command]
fn begin_proof_session(
    state: State<'_, Arc<AppState>>,
    hypothesis_id: Option<String>,
) -> Result<ProofReport, String> {
    if state.is_shutting_down() {
        return Err("MemFlux is shutting down; measurement cannot start.".into());
    }
    let now_ms = crate::telemetry::timestamp_ms();
    let report = {
        let mut proof = state
            .proof
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.proof_active.load(Ordering::Relaxed) {
            return Err(
                "A Proof session is already active. Finish it before beginning another baseline."
                    .into(),
            );
        }
        proof.begin(now_ms);
        proof.hypothesis_id = hypothesis_id.map(|id| id.chars().take(160).collect());
        state.proof_active.store(true, Ordering::Relaxed);
        proof.status()
    };
    log::info!("Performance Proof baseline sampling started.");
    Ok(report)
}

#[tauri::command]
fn mark_proof_change(
    state: State<'_, Arc<AppState>>,
    label: Option<String>,
) -> Result<ProofReport, String> {
    if !state.proof_active.load(Ordering::Relaxed) {
        return Err("No Performance Proof session is active.".into());
    }
    let report = {
        let mut proof = state
            .proof
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        proof.mark_change(crate::telemetry::timestamp_ms(), label);
        proof.status()
    };
    log::info!("Performance Proof change boundary marked.");
    Ok(report)
}

#[tauri::command]
fn proof_session_status(state: State<'_, Arc<AppState>>) -> ProofReport {
    if !state.proof_active.load(Ordering::Relaxed) {
        let proof = state.proof.lock().unwrap_or_else(|e| e.into_inner());
        if proof.started_ms != 0 {
            return proof.status();
        }
        return ProofReport {
            active: false,
            phase: None,
            baseline_samples: 0,
            post_samples: 0,
            comparisons: Vec::new(),
            verdict: "No session active.".into(),
            caveat: String::new(),
            hypothesis_id: None,
        };
    }
    state
        .proof
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .status()
}

/// Final report, then the session stops collecting.
#[tauri::command]
fn end_proof_session(state: State<'_, Arc<AppState>>) -> ProofReport {
    let mut proof = state
        .proof
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    state.proof_active.store(false, Ordering::Relaxed);
    let report = proof.finish();
    log::info!("Performance Proof session ended: {}", report.verdict);
    report
}

fn run_standby_purge(app: &tauri::AppHandle, state: &Arc<AppState>, origin: &str) -> ActionRecord {
    let timestamp = crate::telemetry::timestamp_ms();
    let started = Instant::now();
    let Ok(_guard) = state.action_lock.try_lock() else {
        return record_action(
            app,
            state,
            ActionRecord {
                id: action_id(origin, timestamp),
                timestamp_ms: timestamp,
                origin: origin.into(),
                operation: OPERATION_PURGE_STANDBY.into(),
                outcome: "skipped_busy".into(),
                native_request_succeeded: false,
                native_lifecycle: "not_started".into(),
                elevation_used: false,
                late_completion_possible: false,
                nt_status_hex: None,
                before: None,
                after: None,
                observed_reduction_bytes: None,
                duration_ms: 0,
                native_duration_ms: 0,
                reason: "Another memory action is already running.".into(),
            },
        );
    };

    if state.is_shutting_down() {
        return record_action(
            app,
            state,
            ActionRecord {
                id: action_id(origin, timestamp),
                timestamp_ms: timestamp,
                origin: origin.into(),
                operation: OPERATION_PURGE_STANDBY.into(),
                outcome: "skipped_shutting_down".into(),
                native_request_succeeded: false,
                native_lifecycle: "not_started".into(),
                elevation_used: false,
                late_completion_possible: false,
                nt_status_hex: None,
                before: None,
                after: None,
                observed_reduction_bytes: None,
                duration_ms: 0,
                native_duration_ms: 0,
                reason: "MemFlux is shutting down; the memory action was not started.".into(),
            },
        );
    };

    let capability = action::capability();
    if !capability.supported {
        return record_action(
            app,
            state,
            ActionRecord {
                id: action_id(origin, timestamp),
                timestamp_ms: timestamp,
                origin: origin.into(),
                operation: OPERATION_PURGE_STANDBY.into(),
                outcome: "unsupported".into(),
                native_request_succeeded: false,
                native_lifecycle: "unsupported".into(),
                elevation_used: false,
                late_completion_possible: false,
                nt_status_hex: None,
                before: None,
                after: None,
                observed_reduction_bytes: None,
                duration_ms: started.elapsed().as_millis() as u64,
                native_duration_ms: 0,
                reason: capability.explanation,
            },
        );
    }

    let mut collector = PlatformCollector::new();
    let before_snapshot = collector.collect(0, 0);
    let before = measurement(&before_snapshot);
    if before.standby_bytes.is_none() || before.free_zero_bytes.is_none() {
        return record_action(
            app,
            state,
            ActionRecord {
                id: action_id(origin, timestamp),
                timestamp_ms: timestamp,
                origin: origin.into(),
                operation: OPERATION_PURGE_STANDBY.into(),
                outcome: "telemetry_unavailable".into(),
                native_request_succeeded: false,
                native_lifecycle: "not_started".into(),
                elevation_used: false,
                late_completion_possible: false,
                nt_status_hex: None,
                before: Some(before),
                after: None,
                observed_reduction_bytes: None,
                duration_ms: started.elapsed().as_millis() as u64,
                native_duration_ms: 0,
                reason: "Standby and free/zero counters must both be valid before a purge.".into(),
            },
        );
    }

    log::info!(
        "{origin} standby purge requested; using authenticated session helper or narrow helper."
    );
    let native = if origin == "automatic" {
        action::execute_automatic_standby_purge()
    } else {
        action::execute_standby_purge()
    };

    let after = if native.request_succeeded {
        std::thread::sleep(Duration::from_millis(750));
        Some(measurement(&collector.collect(0, 750)))
    } else {
        None
    };

    if native.request_succeeded {
        let completed_ms = timestamp.saturating_add(started.elapsed().as_millis() as u64);
        let mut policy = state
            .cleaner_policy
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        policy.record_completed_purge_monotonic(completed_ms, Instant::now());
        if let Err(err) = policy.persist_quota_file(&state.cleaner_quota_path) {
            log::warn!("Could not persist cleaner quota ledger: {err}");
        }
    }

    let reduction = after.as_ref().and_then(|after| {
        before
            .standby_bytes
            .zip(after.standby_bytes)
            .map(|(before, after)| (before - after).max(0.0))
    });
    let outcome = action::classify_outcome(&native, reduction);
    let reason = match (native.request_succeeded, reduction) {
        (true, Some(bytes)) if bytes >= 1024.0 * 1024.0 => format!(
            "{} Measured standby reduction after settling: {:.2} MiB.",
            native.detail,
            bytes / 1024.0 / 1024.0
        ),
        (true, None) => format!(
            "{} Post-action standby measurement is unavailable; reduction cannot be determined.",
            native.detail
        ),
        (true, _) => format!(
            "{} No material standby reduction was observed after the settling interval.",
            native.detail
        ),
        (false, _) => native.detail,
    };

    record_action(
        app,
        state,
        ActionRecord {
            id: action_id(origin, timestamp),
            timestamp_ms: timestamp,
            origin: origin.into(),
            operation: OPERATION_PURGE_STANDBY.into(),
            outcome: outcome.into(),
            native_request_succeeded: native.request_succeeded,
            native_lifecycle: serde_json::to_value(&native.lifecycle)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into()),
            elevation_used: native.elevation_used,
            late_completion_possible: native.late_completion_possible,
            nt_status_hex: native.nt_status.map(|status| format!("0x{status:08X}")),
            before: Some(before),
            after,
            observed_reduction_bytes: reduction,
            duration_ms: started.elapsed().as_millis() as u64,
            native_duration_ms: native.duration_ms,
            reason,
        },
    )
}

fn measurement(snapshot: &MemorySnapshot) -> ActionMeasurement {
    ActionMeasurement {
        standby_bytes: snapshot.standby_bytes.value,
        free_zero_bytes: snapshot.free_zero_bytes.value,
        available_bytes: snapshot.available_physical_bytes.value,
    }
}

fn record_action(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    record: ActionRecord,
) -> ActionRecord {
    {
        let mut history = state
            .action_history
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if history.len() == ACTION_HISTORY_LIMIT {
            history.pop_front();
        }
        history.push_back(record.clone());
        if let Err(error) = persist_action_history(&state.action_history_path, &history) {
            log::warn!("Could not persist action history: {error}");
        }
    }
    if let Err(error) = app.emit(EVENT_ACTION, &record) {
        log::warn!("Could not emit action result: {error}");
    }
    log::info!(
        "Memory action completed: origin={}, outcome={}, native_success={}, ntstatus={:?}, reduction_bytes={:?}",
        record.origin,
        record.outcome,
        record.native_request_succeeded,
        record.nt_status_hex,
        record.observed_reduction_bytes
    );
    record
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActionHistoryFile {
    schema_version: u32,
    records: VecDeque<ActionRecord>,
}

fn load_action_history(path: &Path) -> VecDeque<ActionRecord> {
    let Ok(raw) = fs::read_to_string(path) else {
        return VecDeque::with_capacity(ACTION_HISTORY_LIMIT);
    };
    match serde_json::from_str::<ActionHistoryFile>(&raw) {
        Ok(mut file) if file.schema_version == 1 => {
            while file.records.len() > ACTION_HISTORY_LIMIT {
                file.records.pop_front();
            }
            file.records
        }
        Ok(_) => {
            log::warn!("Action history uses an unsupported schema and was preserved.");
            VecDeque::with_capacity(ACTION_HISTORY_LIMIT)
        }
        Err(error) => {
            log::warn!("Action history is invalid and was preserved: {error}");
            VecDeque::with_capacity(ACTION_HISTORY_LIMIT)
        }
    }
}

fn persist_action_history(path: &Path, records: &VecDeque<ActionRecord>) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Action history path has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create action history directory: {error}"))?;
    let temporary = path.with_extension("json.new");
    let payload = serde_json::to_vec_pretty(&ActionHistoryFile {
        schema_version: 1,
        records: records.clone(),
    })
    .map_err(|error| format!("Could not serialize action history: {error}"))?;
    let mut file = File::create(&temporary)
        .map_err(|error| format!("Could not create temporary action history: {error}"))?;
    file.write_all(&payload)
        .map_err(|error| format!("Could not write action history: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("Could not flush action history: {error}"))?;
    drop(file);
    crate::settings::atomic_replace(&temporary, path)
        .map_err(|error| format!("Could not replace action history: {error}"))
}

const MINI_HUD_LABEL: &str = "minihud";
const MINI_HUD_WIDTH: f64 = 268.0;
const MINI_HUD_HEIGHT: f64 = 148.0;

fn show_dashboard(app: &tauri::AppHandle) {
    if app.get_webview_window("main").is_none() {
        // Ultra-Light mode destroyed the webview; recreate on demand.
        if let Err(error) = recreate_main_window(app) {
            log::warn!("{error}");
            return;
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        if let Some(state) = app.try_state::<Arc<AppState>>()
            && let Some(frame) = state
                .latest
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .as_ref()
        {
            let _ = app.emit(EVENT_FRAME, frame);
        }
    }
}

fn recreate_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("MemFlux — Standby Memory Cleaner & Performance Control")
        .inner_size(1240.0, 800.0)
        .min_inner_size(900.0, 620.0)
        .center()
        .resizable(true)
        .visible(true)
        .build()
        .map_err(|error| format!("Could not recreate dashboard window: {error}"))?;
    log::info!("Dashboard webview recreated (ultra-light restore).");
    Ok(())
}

/// Create or refresh the independent Mini HUD webview against current
/// settings. Shares the same frontend bundle; the `hud=mini` route renders a
/// compact component instead of the dashboard.
fn sync_mini_hud(app: &tauri::AppHandle, settings: &AppSettings) {
    let existing = app.get_webview_window(MINI_HUD_LABEL);
    if !settings.mini_hud_enabled {
        if let Some(window) = existing {
            let _ = window.destroy();
            log::info!("Mini HUD window closed (disabled).");
        }
        return;
    }
    let window = match existing {
        Some(window) => window,
        None => {
            let mut builder = tauri::WebviewWindowBuilder::new(
                app,
                MINI_HUD_LABEL,
                tauri::WebviewUrl::App("index.html?hud=mini".into()),
            )
            .title("MemFlux Mini HUD")
            .inner_size(MINI_HUD_WIDTH, MINI_HUD_HEIGHT)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(settings.mini_hud_always_on_top)
            .skip_taskbar(true)
            .visible(true);
            if let (Some(x), Some(y)) = (settings.mini_hud_x, settings.mini_hud_y) {
                builder = builder.position(x as f64, y as f64);
            } else {
                builder = builder.center();
            }
            match builder.build() {
                Ok(window) => {
                    log::info!("Mini HUD window created.");
                    window
                }
                Err(error) => {
                    log::warn!("Could not create Mini HUD window: {error}");
                    return;
                }
            }
        }
    };
    let _ = window.set_always_on_top(settings.mini_hud_always_on_top);
    let _ = window.set_ignore_cursor_events(settings.mini_hud_click_through);
}

/// Persist the Mini HUD's last known position (throttled — window moves fire
/// continuously while dragging).
fn note_mini_hud_position(state: &AppState, x: i32, y: i32) {
    let mut pending = state
        .mini_hud_pending
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    pending.0 = Some((x, y));
    if pending.1.elapsed() < Duration::from_millis(1_500) {
        return;
    }
    pending.1 = Instant::now();
    if let Err(error) = state.edit_settings(|settings| {
        settings.mini_hud_x = Some(x);
        settings.mini_hud_y = Some(y);
    }) {
        log::warn!("Could not persist Mini HUD position: {error}");
    }
}

fn update_tray_tooltip(app: &tauri::AppHandle, paused: bool, pressure: Option<&str>) {
    let text = if paused {
        "MemFlux — Monitoring paused".to_string()
    } else if let Some(pressure) = pressure {
        format!("MemFlux — {pressure} memory pressure")
    } else {
        "MemFlux — Monitoring memory".to_string()
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

fn spawn_cleaner_scheduler(app: tauri::AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("memflux-cleaner".into())
        .spawn(move || {
            let mut collector = PlatformCollector::new();
            let mut sequence = 0_u64;

            loop {
                let (settings, shutdown) = {
                    let control = state
                        .control
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    (control.settings.clone(), control.shutdown)
                };
                if shutdown {
                    log::info!("Independent cleaner scheduler stopped cleanly.");
                    break;
                }

                let poll_interval = settings.cleaner_poll_interval_ms.clamp(1_000, 60_000);
                let now_ms = crate::telemetry::timestamp_ms();
                let now_instant = Instant::now();

                // Reuse cached telemetry snapshot if fresh within poll_interval, otherwise collect directly
                let (standby_bytes, free_zero_bytes) = {
                    let cached = state
                        .latest
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .as_ref()
                        .and_then(|frame| {
                            if now_ms.saturating_sub(frame.snapshot.timestamp_ms) <= poll_interval {
                                Some((
                                    frame.snapshot.standby_bytes.value,
                                    frame.snapshot.free_zero_bytes.value,
                                ))
                            } else {
                                None
                            }
                        });
                    match cached {
                        Some(pair) => pair,
                        None => {
                            sequence = sequence.saturating_add(1);
                            let snap = collector.collect(sequence, poll_interval);
                            (snap.standby_bytes.value, snap.free_zero_bytes.value)
                        }
                    }
                };

                let foreground = if settings.cleaner_excluded_applications.is_empty() {
                    None
                } else {
                    crate::performance::foreground_executable()
                };
                let session_status = action::cleaner_session_status();
                let capability = action::capability();
                let action_busy = state.action_lock.try_lock().is_err();
                let session_override = state
                    .coordinator
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .cleaner_override();

                let summary = {
                    let mut policy = state
                        .cleaner_policy
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    policy.evaluate_summary(
                        &settings,
                        CleanerInputs {
                            now_ms,
                            now_instant,
                            standby_bytes,
                            free_zero_bytes,
                            foreground_executable: foreground.as_deref(),
                            authorization_ready: session_status.authorized,
                            build_supported: capability.supported,
                            action_in_progress: action_busy,
                            session_override,
                        },
                    )
                };

                *state
                    .cleaner_decision
                    .write()
                    .unwrap_or_else(|error| error.into_inner()) = summary.decision.clone();
                *state
                    .cleaner_status
                    .write()
                    .unwrap_or_else(|error| error.into_inner()) = summary.clone();

                if summary.decision == CleanerDecision::Trigger && !action_busy && !shutdown {
                    let _ = run_standby_purge(&app, &state, "automatic");
                }

                let control = state
                    .control
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if control.shutdown {
                    break;
                }
                let _ = state
                    .cleaner_wake
                    .wait_timeout(control, Duration::from_millis(poll_interval))
                    .unwrap_or_else(|error| error.into_inner());
            }
        })
        .expect("failed to start independent cleaner scheduler thread");
}

fn spawn_sampler(app: tauri::AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("memflux-telemetry".into())
        .spawn(move || {
            let mut collector = PlatformCollector::new();
            let mut cpu = CpuSensor::default();
            let mut gpu = GpuSensor::default();
            let mut guardian = GuardianEvaluator::new();
            let mut atlas = crate::atlas::AtlasEngine::default();
            let mut sequence = 0_u64;

            loop {
                let (settings, paused, shutdown) = {
                    let control = state
                        .control
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    (control.settings.clone(), control.paused, control.shutdown)
                };
                if shutdown {
                    log::info!("Telemetry thread stopped cleanly.");
                    break;
                }

                let window_visible = app
                    .get_webview_window("main")
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false);
                let mini_hud_visible = app
                    .get_webview_window(MINI_HUD_LABEL)
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false);
                // A UI surface (dashboard or mini HUD) is consuming frames.
                let ui_active = window_visible || mini_hud_visible;
                let interval_ms = if ui_active {
                    settings.foreground_interval_ms
                } else {
                    settings.background_interval_ms
                };

                if !paused {
                    sequence = sequence.saturating_add(1);
                    let snapshot = collector.collect(sequence, interval_ms);
                    let decision = guardian.evaluate(&snapshot, settings.guardian_enabled);
                    let hud_active = ui_active
                        && (settings.performance_hud_enabled || settings.mini_hud_enabled);
                    let cpu_metric = cpu.collect(interval_ms, ui_active || state.proof_active.load(Ordering::Relaxed));
                    let gpu_telemetry = gpu.collect(
                        interval_ms,
                        (hud_active && settings.hud_show_gpu) || settings.gamefocus_enabled,
                        &settings.hud_selected_gpu,
                    );
                    let fps_capture = state
                        .presentmon
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .status(interval_ms, settings.fps_capture_permitted);

                    let fg_process = if settings.gamefocus_enabled {
                        crate::performance::foreground_window_process()
                    } else {
                        None
                    };

                    let coordinator_snapshot = {
                        let mut coord = state
                            .coordinator
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        coord.tick(
                            &settings,
                            fg_process,
                            crate::performance::is_process_alive,
                            &gpu_telemetry.per_process_gpu_percent,
                            snapshot.timestamp_ms,
                        );
                        coord.snapshot()
                    };

                    let cleaner_decision = state
                        .cleaner_decision
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone();
                    let cleaner_status = state
                        .cleaner_status
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone();

                    let atlas = atlas.observe(&snapshot, &cpu_metric, &gpu_telemetry, &fps_capture, crate::telemetry::timestamp_ms());
                    let frame = TelemetryFrame {
                        atlas,
                        cpu_utilization: cpu_metric,
                        gpu_telemetry,
                        fps_capture,
                        snapshot,
                        guardian: decision,
                        cleaner_decision,
                        cleaner_status,
                        coordinator: coordinator_snapshot,
                    };

                    if sequence == 1 || sequence.is_multiple_of(60) {
                        log::info!(
                            "Telemetry milestone: sequence={sequence}, collection={:?}, pressure={:?}, available_bytes={:?}, commit_percent={:?}, standby_bytes={:?}, page_reads_per_second={:?}",
                            frame.snapshot.collection_status,
                            frame.guardian.pressure,
                            frame.snapshot.available_physical_bytes.value,
                            frame.snapshot.commit_percent.value,
                            frame.snapshot.standby_bytes.value,
                            frame.snapshot.page_reads_per_second.value,
                        );
                    }
                    if state.proof_active.load(Ordering::Relaxed)
                        && let Ok(mut proof) = state.proof.lock()
                    {
                        proof.observe(ProofSample {
                            at_ms: frame.snapshot.timestamp_ms,
                            cpu_percent: frame.cpu_utilization.fresh_value(frame.atlas.timestamp_ms),
                            gpu_percent: frame.gpu_telemetry.utilization.fresh_value(frame.atlas.timestamp_ms),
                            ram_used_bytes: match (
                                frame.snapshot.total_physical_bytes.value,
                                frame.snapshot.available_physical_bytes.value,
                            ) {
                                (Some(total), Some(avail)) => Some((total - avail).max(0.0)),
                                _ => None,
                            },
                            fps: frame.fps_capture.fps.fresh_value(frame.atlas.timestamp_ms),
                            frame_ms: frame.fps_capture.frame_time_ms.fresh_value(frame.atlas.timestamp_ms),
                            frame_p99_ms: frame.fps_capture.frame_time_ms.fresh_value(frame.atlas.timestamp_ms).and(frame.fps_capture.frame_time_p99_ms),
                        });
                    }
                    *state
                        .latest
                        .write()
                        .unwrap_or_else(|error| error.into_inner()) = Some(frame.clone());
                    if ui_active && let Err(error) = app.emit(EVENT_FRAME, &frame) {
                        log::warn!("Could not emit telemetry frame: {error}");
                    }
                    update_tray_tooltip(
                        &app,
                        false,
                        Some(pressure_label(&frame.guardian.pressure)),
                    );
                } else {
                    cpu.collect(interval_ms, false);
                    gpu.collect(interval_ms, false, "auto");
                    guardian.reset();
                }

                let control = state
                    .control
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let _ = state
                    .wake
                    .wait_timeout(control, Duration::from_millis(interval_ms))
                    .unwrap_or_else(|error| error.into_inner());
            }
        })
        .expect("failed to start telemetry thread");
}

fn pressure_label(pressure: &crate::guardian::PressureLevel) -> &'static str {
    use crate::guardian::PressureLevel;
    match pressure {
        PressureLevel::Comfortable => "comfortable",
        PressureLevel::Watch => "watch",
        PressureLevel::Elevated => "elevated",
        PressureLevel::Critical => "critical",
        PressureLevel::Unknown => "unknown",
        PressureLevel::Paused => "paused",
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open dashboard", true, None::<&str>)?;
    let mini_hud = MenuItem::with_id(app, "toggle-minihud", "Toggle Mini HUD", true, None::<&str>)?;
    let toggle = MenuItem::with_id(
        app,
        "toggle-monitoring",
        "Pause / resume monitoring",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit = MenuItem::with_id(app, "exit", "Exit MemFlux", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &mini_hud, &toggle, &separator, &exit])?;
    let state = app.state::<Arc<AppState>>().inner().clone();

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("MemFlux — Monitoring memory")
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_dashboard(app),
            "toggle-minihud" => {
                match state.edit_settings(|settings| {
                    settings.mini_hud_enabled = !settings.mini_hud_enabled
                }) {
                    Ok(settings) => {
                        sync_mini_hud(app, &settings);
                        let _ = app.emit("settings://changed", &settings);
                        state.wake.notify_all();
                    }
                    Err(error) => log::warn!("Could not persist Mini HUD toggle: {error}"),
                }
            }
            "toggle-monitoring" => {
                let paused = state.toggle_paused();
                let _ = app.emit(EVENT_MONITORING, !paused);
                update_tray_tooltip(app, paused, None);
            }
            "exit" => {
                state.execute_shutdown(app);
                app.exit(0);
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_dashboard(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_latest_frame,
            get_settings,
            update_settings,
            disable_mini_hud,
            set_monitoring_paused,
            get_runtime_status,
            get_action_history,
            purge_standby_memory,
            authorize_cleaner_session,
            revoke_cleaner_session,
            get_coordinator_snapshot,
            apply_powershift_mode,
            restore_rewind_entry,
            restore_all_rewind,
            activate_gamefocus_session,
            deactivate_gamefocus_session,
            start_fps_capture,
            stop_fps_capture,
            begin_proof_session,
            mark_proof_change,
            proof_session_status,
            end_proof_session
        ])
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let action_history_path = config_dir.join("action-history.json");
            let cleaner_quota_path = config_dir.join("cleaner-quota.json");
            let rewind_path = config_dir.join("rewind-journal.json");
            let store = SettingsStore::new(settings_path);
            let envelope = store.load();
            if let Some(warning) = &envelope.warning {
                log::warn!("{warning}");
            }
            let state = Arc::new(AppState::new(
                store,
                envelope,
                action_history_path,
                cleaner_quota_path,
                rewind_path,
            ));
            app.manage(state.clone());
            build_tray(app)?;
            {
                let settings = state
                    .control
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .settings
                    .clone();
                sync_mini_hud(app.handle(), &settings);
            }
            spawn_cleaner_scheduler(app.handle().clone(), state.clone());
            spawn_sampler(app.handle().clone(), state);
            log::info!("MemFlux V4.2 runtime started.");
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => match window.label() {
                    "main" => {
                        let state = window.state::<Arc<AppState>>();
                        let (close_to_tray, ultra_light) = {
                            let control = state
                                .control
                                .lock()
                                .unwrap_or_else(|error| error.into_inner());
                            (
                                control.settings.close_to_tray,
                                control.settings.ultra_light_tray_mode,
                            )
                        };
                        if close_to_tray {
                            api.prevent_close();
                            if ultra_light {
                                // Ultra-Light Tray Mode: free the entire
                                // WebView2 subtree; the tray remains functional
                                // and "Open dashboard" recreates the window.
                                let _ = window.destroy();
                                log::info!("Dashboard webview destroyed (ultra-light tray mode).");
                            } else {
                                let _ = window.hide();
                            }
                        } else {
                            state.execute_shutdown(window.app_handle());
                            window.app_handle().exit(0);
                        }
                    }
                    MINI_HUD_LABEL => {
                        // Dismissing the HUD persists its disabled state.
                        let state = window.state::<Arc<AppState>>();
                        match state.edit_settings(|settings| settings.mini_hud_enabled = false) {
                            Ok(settings) => {
                                let _ = window.app_handle().emit("settings://changed", &settings);
                            }
                            Err(error) => {
                                log::warn!("Could not persist Mini HUD disabled state: {error}")
                            }
                        }
                    }
                    _ => {}
                },
                WindowEvent::Moved(position) if window.label() == MINI_HUD_LABEL => {
                    let state = window.state::<Arc<AppState>>();
                    note_mini_hud_position(&state, position.x, position.y);
                }
                _ => {}
            }
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building MemFlux");
    // Catch-all: any exit path that skipped the coordinated teardown (process
    // exit codes, last-window-close edge cases) still runs restoration once.
    app.run(|app_handle, event| {
        if matches!(event, RunEvent::Exit)
            && let Some(state) = app_handle.try_state::<Arc<AppState>>()
        {
            state.execute_shutdown(app_handle);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn record(index: u64) -> ActionRecord {
        ActionRecord {
            id: format!("test-{index}"),
            timestamp_ms: index,
            origin: "manual".into(),
            operation: OPERATION_PURGE_STANDBY.into(),
            outcome: "native_failed".into(),
            native_request_succeeded: false,
            native_lifecycle: "native_completed".into(),
            elevation_used: true,
            late_completion_possible: false,
            nt_status_hex: Some("0xC0000061".into()),
            before: None,
            after: None,
            observed_reduction_bytes: None,
            duration_ms: 1,
            native_duration_ms: 1,
            reason: "fixture".into(),
        }
    }

    #[test]
    fn action_history_round_trips_and_is_bounded_on_load() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("memflux-history-test-{nonce}"));
        let path = directory.join("action-history.json");
        let records = (0..105).map(record).collect::<VecDeque<_>>();
        persist_action_history(&path, &records).expect("persist history");
        let loaded = load_action_history(&path);
        assert_eq!(loaded.len(), ACTION_HISTORY_LIMIT);
        assert_eq!(loaded.front().map(|item| item.id.as_str()), Some("test-5"));
        assert_eq!(loaded.back().map(|item| item.id.as_str()), Some("test-104"));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(directory);
    }
}
