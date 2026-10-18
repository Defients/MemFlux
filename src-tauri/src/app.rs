use crate::guardian::{GuardianDecision, GuardianEvaluator};
use crate::settings::{AppSettings, SettingsEnvelope, SettingsStore};
use crate::telemetry::{Collector, MemorySnapshot, PlatformCollector};
use serde::Serialize;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State, WindowEvent};

const TRAY_ID: &str = "memflux-tray";
const EVENT_FRAME: &str = "telemetry://frame";
const EVENT_MONITORING: &str = "monitoring://state";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    pub snapshot: MemorySnapshot,
    pub guardian: GuardianDecision,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    paused: bool,
    action_capability: String,
    action_explanation: String,
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
}

impl AppState {
    fn new(store: SettingsStore, envelope: SettingsEnvelope) -> Self {
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
    let persisted = state.settings_store.save(&settings)?;
    {
        let mut control = state
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        control.settings = persisted.clone();
    }
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
        action_capability: "unsupported".into(),
        action_explanation: "Phase 1 is monitoring-only. MemFlux does not expose standby-list clearing on this unverified configuration.".into(),
    }
}

fn show_dashboard(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
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
                    let frame = TelemetryFrame {
                        snapshot,
                        guardian: decision,
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
                    *state
                        .latest
                        .write()
                        .unwrap_or_else(|error| error.into_inner()) = Some(frame.clone());
                    if let Err(error) = app.emit(EVENT_FRAME, &frame) {
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
            get_runtime_status
        ])
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let store = SettingsStore::new(settings_path);
            let envelope = store.load();
            if let Some(warning) = &envelope.warning {
                log::warn!("{warning}");
            }
            let state = Arc::new(AppState::new(store, envelope));
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
