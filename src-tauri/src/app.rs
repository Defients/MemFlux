use crate::action::{self, ActionCapability, OPERATION_PURGE_STANDBY};
use crate::cleaner::{CleanerDecision, CleanerInputs, CleanerPolicy};
use crate::guardian::{GuardianDecision, GuardianEvaluator};
use crate::settings::{AppSettings, SettingsEnvelope, SettingsStore};
use crate::telemetry::{Collector, MemorySnapshot, PlatformCollector};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State, WindowEvent};

const TRAY_ID: &str = "memflux-tray";
const EVENT_FRAME: &str = "telemetry://frame";
const EVENT_MONITORING: &str = "monitoring://state";
const EVENT_ACTION: &str = "action://result";
const ACTION_HISTORY_LIMIT: usize = 100;
static ACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn action_id(timestamp: u64) -> String {
    format!(
        "manual-{timestamp}-{}",
        ACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    pub snapshot: MemorySnapshot,
    pub guardian: GuardianDecision,
    pub cleaner_decision: CleanerDecision,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    paused: bool,
    cleaner_paused: bool,
    cleaner_decision: CleanerDecision,
    automatic_execution_ready: bool,
    action_capability: ActionCapability,
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

struct ControlState {
    settings: AppSettings,
    paused: bool,
    shutdown: bool,
}

pub struct AppState {
    control: Mutex<ControlState>,
    wake: Condvar,
    latest: RwLock<Option<TelemetryFrame>>,
    settings_store: SettingsStore,
    settings_warning: Mutex<Option<String>>,
    action_lock: Mutex<()>,
    action_history: Mutex<VecDeque<ActionRecord>>,
    action_history_path: PathBuf,
    cleaner_decision: RwLock<CleanerDecision>,
}

impl AppState {
    fn new(store: SettingsStore, envelope: SettingsEnvelope, action_history_path: PathBuf) -> Self {
        let action_history = load_action_history(&action_history_path);
        Self {
            control: Mutex::new(ControlState {
                settings: envelope.settings,
                paused: false,
                shutdown: false,
            }),
            wake: Condvar::new(),
            latest: RwLock::new(None),
            settings_store: store,
            settings_warning: Mutex::new(envelope.warning),
            action_lock: Mutex::new(()),
            action_history: Mutex::new(action_history),
            action_history_path,
            cleaner_decision: RwLock::new(CleanerDecision::Disabled),
        }
    }

    fn request_shutdown(&self) {
        if let Ok(mut control) = self.control.lock() {
            control.shutdown = true;
            self.wake.notify_all();
        }
    }

    fn toggle_paused(&self) -> bool {
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        control.paused = !control.paused;
        let paused = control.paused;
        self.wake.notify_all();
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
    settings: AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<AppSettings, String> {
    let persisted = {
        let mut control = state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let persisted = state.settings_store.save(&settings)?;
        control.settings = persisted.clone();
        persisted
    };
    *state
        .settings_warning
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = None;
    state.wake.notify_all();
    log::info!("Settings updated and persisted.");
    Ok(persisted)
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
    let paused = state
        .control
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .paused;
    RuntimeStatus {
        paused,
        cleaner_paused: state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .settings
            .cleaner_paused,
        cleaner_decision: state
            .cleaner_decision
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        automatic_execution_ready: false,
        action_capability: action::capability(),
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
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || run_manual_purge(&app, &state))
        .await
        .map_err(|error| format!("Standby purge worker failed: {error}"))
}

fn run_manual_purge(app: &tauri::AppHandle, state: &Arc<AppState>) -> ActionRecord {
    let timestamp = crate::telemetry::timestamp_ms();
    let started = Instant::now();
    let Ok(_guard) = state.action_lock.try_lock() else {
        return record_action(
            app,
            state,
            ActionRecord {
                id: action_id(timestamp),
                timestamp_ms: timestamp,
                origin: "manual".into(),
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

    let capability = action::capability();
    if !capability.supported {
        return record_action(
            app,
            state,
            ActionRecord {
                id: action_id(timestamp),
                timestamp_ms: timestamp,
                origin: "manual".into(),
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
                id: action_id(timestamp),
                timestamp_ms: timestamp,
                origin: "manual".into(),
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

    log::info!("Manual standby purge requested; awaiting the narrow elevated helper if required.");
    let native = action::execute_standby_purge();
    let after = if native.request_succeeded {
        std::thread::sleep(Duration::from_millis(750));
        Some(measurement(&collector.collect(0, 750)))
    } else {
        None
    };
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
            id: action_id(timestamp),
            timestamp_ms: timestamp,
            origin: "manual".into(),
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
        // Hold the history lock through replacement so simultaneous skipped and
        // completed actions cannot race or persist an older snapshot last.
        if let Err(error) = persist_action_history(&state.action_history_path, &history) {
            log::warn!("Could not persist action history: {error}");
        }
    }
    if let Err(error) = app.emit(EVENT_ACTION, &record) {
        log::warn!("Could not emit action result: {error}");
    }
    log::info!(
        "Memory action completed: outcome={}, native_success={}, ntstatus={:?}, reduction_bytes={:?}",
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

fn show_dashboard(app: &tauri::AppHandle) {
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

fn spawn_sampler(app: tauri::AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("memflux-telemetry".into())
        .spawn(move || {
            let mut collector = PlatformCollector::new();
            let mut guardian = GuardianEvaluator::new();
            let mut cleaner = CleanerPolicy::default();
            let mut last_cleaner_evaluation_ms = 0_u64;
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
                let interval_ms = if window_visible {
                    settings.foreground_interval_ms
                } else {
                    settings.background_interval_ms
                };

                if !paused {
                    sequence = sequence.saturating_add(1);
                    let snapshot = collector.collect(sequence, interval_ms);
                    let decision = guardian.evaluate(&snapshot, settings.guardian_enabled);
                    let mut frame = TelemetryFrame {
                        snapshot,
                        guardian: decision,
                        cleaner_decision: state.cleaner_decision.read()
                            .unwrap_or_else(|error| error.into_inner()).clone(),
                    };
                    let now_ms = frame.snapshot.timestamp_ms;
                    if now_ms.saturating_sub(last_cleaner_evaluation_ms)
                        >= settings.cleaner_poll_interval_ms
                    {
                        cleaner.restore_completed_purges(state.action_history.lock()
                            .unwrap_or_else(|error| error.into_inner()).iter()
                            .filter(|record| record.native_request_succeeded)
                            .map(|record| record.timestamp_ms.saturating_add(record.duration_ms)));
                        let cleaner_decision = cleaner.evaluate(
                            &settings,
                            CleanerInputs {
                                now_ms,
                                standby_bytes: frame.snapshot.standby_bytes.value,
                                free_zero_bytes: frame.snapshot.free_zero_bytes.value,
                                foreground_executable: None,
                                authorization_ready: false,
                            },
                        );
                        *state
                            .cleaner_decision
                            .write()
                            .unwrap_or_else(|error| error.into_inner()) = cleaner_decision.clone();
                        frame.cleaner_decision = cleaner_decision;
                        last_cleaner_evaluation_ms = now_ms;
                    }
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
                    *state
                        .latest
                        .write()
                        .unwrap_or_else(|error| error.into_inner()) = Some(frame.clone());
                    if window_visible
                        && let Err(error) = app.emit(EVENT_FRAME, &frame) {
                            log::warn!("Could not emit telemetry frame: {error}");
                    }
                    update_tray_tooltip(
                        &app,
                        false,
                        Some(pressure_label(&frame.guardian.pressure)),
                    );
                } else {
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
    let toggle = MenuItem::with_id(
        app,
        "toggle-monitoring",
        "Pause / resume monitoring",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit = MenuItem::with_id(app, "exit", "Exit MemFlux", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &toggle, &separator, &exit])?;
    let state = app.state::<Arc<AppState>>().inner().clone();

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("MemFlux — Monitoring memory")
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_dashboard(app),
            "toggle-monitoring" => {
                let paused = state.toggle_paused();
                let _ = app.emit(EVENT_MONITORING, !paused);
                update_tray_tooltip(app, paused, None);
            }
            "exit" => {
                state.request_shutdown();
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
            set_monitoring_paused,
            get_runtime_status,
            get_action_history,
            purge_standby_memory
        ])
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let action_history_path = config_dir.join("action-history.json");
            let store = SettingsStore::new(settings_path);
            let envelope = store.load();
            if let Some(warning) = &envelope.warning {
                log::warn!("{warning}");
            }
            let state = Arc::new(AppState::new(store, envelope, action_history_path));
            app.manage(state.clone());
            build_tray(app)?;
            spawn_sampler(app.handle().clone(), state);
            log::info!("MemFlux Phase 1 monitoring runtime started.");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<Arc<AppState>>();
                let close_to_tray = state
                    .control
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .settings
                    .close_to_tray;
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    state.request_shutdown();
                    window.app_handle().exit(0);
                }
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running MemFlux");
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
