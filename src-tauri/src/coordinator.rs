use crate::cleaner::SessionCleanerOverride;
use crate::gamefocus::{
    CandidateApplication, GameFocusStatus, GameProfile, GameSessionLifecycle, WorkloadAnalyzer,
    is_non_game_executable,
};
use crate::powershift::{
    self, PowerShiftMode, PowerShiftOutcome, PowerShiftStatus, TARGET_WINDOWS_POWER_SCHEME,
};
use crate::rewind::{RewindEntry, RewindJournal, RewindStatus};
use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_COORDINATOR_EVENTS: usize = 100;
static EVENT_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorEvent {
    pub id: String,
    pub timestamp_ms: u64,
    pub subsystem: String,
    pub event_type: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorSnapshot {
    pub gamefocus: GameFocusStatus,
    pub powershift: PowerShiftStatus,
    pub rewind_entries: Vec<RewindEntry>,
    pub active_interventions_count: usize,
    pub cleaner_override: SessionCleanerOverride,
    pub events: Vec<CoordinatorEvent>,
}

pub struct OptimizationCoordinator {
    rewind: RewindJournal,
    workload_analyzer: WorkloadAnalyzer,
    gamefocus: GameFocusStatus,
    events: VecDeque<CoordinatorEvent>,
    manual_hold_idle_for_pid: Option<u32>,
}

impl OptimizationCoordinator {
    pub fn new(rewind_path: PathBuf, now_ms: u64) -> Self {
        let mut rewind = RewindJournal::load(rewind_path);
        let mut events = VecDeque::with_capacity(MAX_COORDINATOR_EVENTS);

        let recovered = rewind.recover_on_startup(
            now_ms,
            |_| powershift::read_live_scheme_guid(),
            |_, guid| powershift::apply_live_scheme_guid(guid),
        );
        for entry in recovered {
            let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
            events.push_back(CoordinatorEvent {
                id: format!("coord-{now_ms}-{seq}"),
                timestamp_ms: now_ms,
                subsystem: "rewind".into(),
                event_type: "startup_recovery".into(),
                summary: format!(
                    "Startup recovery processed {} ({:?})",
                    entry.target_identity, entry.status
                ),
                detail: entry.error_detail.unwrap_or_else(|| {
                    format!(
                        "Restored '{}' ({}) after previous session exit.",
                        entry.previous_state_label, entry.previous_state
                    )
                }),
            });
        }

        Self {
            rewind,
            workload_analyzer: WorkloadAnalyzer::default(),
            gamefocus: GameFocusStatus::default(),
            events,
            manual_hold_idle_for_pid: None,
        }
    }

    fn log_event(
        &mut self,
        now_ms: u64,
        subsystem: &str,
        event_type: &str,
        summary: String,
        detail: String,
    ) {
        let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
        if self.events.len() >= MAX_COORDINATOR_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(CoordinatorEvent {
            id: format!("coord-{now_ms}-{seq}"),
            timestamp_ms: now_ms,
            subsystem: subsystem.into(),
            event_type: event_type.into(),
            summary,
            detail,
        });
    }

    pub fn snapshot(&self) -> CoordinatorSnapshot {
        let powershift = powershift::query_status(&self.rewind);
        let rewind_entries = self.rewind.entries();
        let active_interventions_count = self.rewind.active_entries().len();
        let mut gf = self.gamefocus.clone();
        if let Some(active_scheme) = &powershift.active_scheme {
            gf.active_power_scheme_name = Some(active_scheme.name.clone());
            gf.active_power_scheme_guid = Some(active_scheme.guid.clone());
        }
        gf.power_intervention_active = powershift.memflux_owns_active_change;

        CoordinatorSnapshot {
            gamefocus: gf,
            powershift,
            rewind_entries,
            active_interventions_count,
            cleaner_override: self.gamefocus.cleaner_override,
            events: self.events.iter().cloned().collect(),
        }
    }

    pub fn cleaner_override(&self) -> SessionCleanerOverride {
        match self.gamefocus.lifecycle {
            GameSessionLifecycle::Active | GameSessionLifecycle::TemporarilyBackgrounded => {
                self.gamefocus.cleaner_override
            }
            _ => SessionCleanerOverride::KeepGlobal,
        }
    }

    pub fn apply_manual_powershift(
        &mut self,
        mode: PowerShiftMode,
        custom_guid: Option<&str>,
        allow_on_battery: bool,
        now_ms: u64,
    ) -> Result<PowerShiftOutcome, String> {
        let outcome = powershift::apply_power_mode(
            mode,
            custom_guid,
            "powershift",
            None,
            "user_explicit",
            allow_on_battery,
            now_ms,
            &mut self.rewind,
        )?;
        self.log_event(
            now_ms,
            "powershift",
            if outcome.action_executed {
                "scheme_applied"
            } else {
                "scheme_unchanged"
            },
            outcome
                .applied_scheme
                .as_ref()
                .map(|s| format!("PowerShift set to {}", s.name))
                .unwrap_or_else(|| "PowerShift evaluated".into()),
            outcome.detail.clone(),
        );
        Ok(outcome)
    }

    pub fn restore_rewind_entry(
        &mut self,
        action_id: &str,
        now_ms: u64,
    ) -> Result<RewindEntry, String> {
        let entry = self.rewind.restore_entry(
            action_id,
            now_ms,
            |_| powershift::read_live_scheme_guid(),
            |_, prev| powershift::apply_live_scheme_guid(prev),
        )?;
        let detail = entry.error_detail.clone().unwrap_or_else(|| {
            format!(
                "Restored '{}' ({}).",
                entry.previous_state_label, entry.previous_state
            )
        });
        self.gamefocus.restoration_state = match entry.status {
            RewindStatus::Restored => format!("Restored to {}", entry.previous_state_label),
            RewindStatus::ConflictSkipped => {
                "External user change preserved (restore skipped)".into()
            }
            _ => format!("Restore status: {:?}", entry.status),
        };
        self.log_event(
            now_ms,
            "rewind",
            "restore_entry",
            format!("Rewind {} -> {:?}", entry.action_id, entry.status),
            detail,
        );
        Ok(entry)
    }

    pub fn restore_all_rewind(&mut self, now_ms: u64) -> Vec<RewindEntry> {
        let results = self.rewind.restore_all_eligible(
            now_ms,
            |_| powershift::read_live_scheme_guid(),
            |_, prev| powershift::apply_live_scheme_guid(prev),
        );
        for entry in &results {
            let detail = entry.error_detail.clone().unwrap_or_else(|| {
                format!(
                    "Restored '{}' ({}).",
                    entry.previous_state_label, entry.previous_state
                )
            });
            self.log_event(
                now_ms,
                "rewind",
                "restore_all",
                format!("Restored {} ({:?})", entry.target_identity, entry.status),
                detail,
            );
        }
        if !results.is_empty() {
            self.gamefocus.restoration_state = "All eligible MemFlux changes restored".into();
        }
        results
    }

    pub fn activate_gamefocus_session(
        &mut self,
        profile: GameProfile,
        pid: Option<u32>,
        executable_path: String,
        allow_on_battery: bool,
        now_ms: u64,
    ) -> Result<GameFocusStatus, String> {
        let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
        let session_id = format!("gf-session-{now_ms}-{seq}");
        let exe_name = executable_path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&executable_path)
            .to_string();

        self.manual_hold_idle_for_pid = None;
        self.gamefocus.lifecycle = GameSessionLifecycle::ProfilePending;
        self.gamefocus.session_id = Some(session_id.clone());
        self.gamefocus.active_pid = pid;
        self.gamefocus.active_executable_name = Some(exe_name.clone());
        self.gamefocus.active_executable_path = Some(executable_path);
        self.gamefocus.selected_profile = Some(profile.clone());
        self.gamefocus.session_started_ms = Some(now_ms);
        self.gamefocus.session_duration_ms = 0;
        self.gamefocus.cleaner_override = profile.cleaner_override;
        self.gamefocus.candidate = None;

        let mut restoration_note =
            "Read-only session active (no system settings mutated)".to_string();
        if profile.approved_power_intervention
            && profile.powershift_mode != PowerShiftMode::CurrentSystemMode
        {
            match powershift::apply_power_mode(
                profile.powershift_mode,
                profile.custom_power_scheme_guid.as_deref(),
                "gamefocus",
                Some(session_id.clone()),
                "gamefocus_profile_approved",
                allow_on_battery,
                now_ms,
                &mut self.rewind,
            ) {
                Ok(outcome) => {
                    if outcome.action_executed {
                        let prev_name = outcome
                            .previous_scheme
                            .as_ref()
                            .map(|s| s.name.as_str())
                            .unwrap_or("previous scheme");
                        let applied_name = outcome
                            .applied_scheme
                            .as_ref()
                            .map(|s| s.name.as_str())
                            .unwrap_or("target scheme");
                        restoration_note = format!(
                            "Rewind tracking active: will restore '{prev_name}' when session ends (currently '{applied_name}')"
                        );
                    } else {
                        restoration_note = outcome.detail.clone();
                    }
                    self.log_event(
                        now_ms,
                        "gamefocus",
                        "powershift_activated",
                        format!("GameFocus applied PowerShift for {}", profile.name),
                        outcome.detail,
                    );
                }
                Err(error) => {
                    restoration_note = format!("PowerShift skipped: {error}");
                    self.log_event(
                        now_ms,
                        "gamefocus",
                        "powershift_skipped",
                        format!("GameFocus PowerShift skipped for {}", profile.name),
                        error,
                    );
                }
            }
        }

        self.gamefocus.lifecycle = GameSessionLifecycle::Active;
        self.gamefocus.restoration_state = restoration_note;
        self.log_event(
            now_ms,
            "gamefocus",
            "session_started",
            format!("GameFocus session started: {} ({})", profile.name, exe_name),
            format!(
                "Profile preset: {:?}, cleaner override: {:?}.",
                profile.preset, profile.cleaner_override
            ),
        );

        Ok(self.snapshot().gamefocus)
    }

    pub fn deactivate_gamefocus_and_restore(
        &mut self,
        reason: &str,
        now_ms: u64,
    ) -> GameFocusStatus {
        if let Some(pid) = self.gamefocus.active_pid {
            // Prevent auto-detection from immediately re-triggering on the same running PID after manual deactivation
            self.manual_hold_idle_for_pid = Some(pid);
        }
        self.gamefocus.lifecycle = GameSessionLifecycle::Restoring;

        let mut restore_summary = "No power scheme changes required restoration.".to_string();
        if self
            .rewind
            .active_for_target(TARGET_WINDOWS_POWER_SCHEME)
            .is_some()
        {
            match powershift::restore_power_scheme(now_ms, &mut self.rewind) {
                Ok(Some(entry)) => {
                    restore_summary = match entry.status {
                        RewindStatus::Restored => format!(
                            "Restored Windows power scheme to '{}' ({}).",
                            entry.previous_state_label, entry.previous_state
                        ),
                        RewindStatus::ConflictSkipped => entry
                            .error_detail
                            .unwrap_or_else(|| "Preserved external power scheme selection.".into()),
                        _ => entry
                            .error_detail
                            .unwrap_or_else(|| "Power scheme restoration failed.".into()),
                    };
                }
                Ok(None) => {}
                Err(err) => {
                    restore_summary = format!("Power scheme restoration error: {err}");
                }
            }
        }

        let ended_name = self
            .gamefocus
            .selected_profile
            .as_ref()
            .map(|p| p.name.clone())
            .or_else(|| self.gamefocus.active_executable_name.clone())
            .unwrap_or_else(|| "Game session".into());

        self.gamefocus.lifecycle = GameSessionLifecycle::Completed;
        self.gamefocus.active_pid = None;
        self.gamefocus.cleaner_override = SessionCleanerOverride::KeepGlobal;
        self.gamefocus.competing_workloads.clear();
        self.gamefocus.recommendations.clear();
        self.gamefocus.restoration_state = restore_summary.clone();
        self.gamefocus.last_completed_summary =
            Some(format!("{ended_name}: {reason} {restore_summary}"));

        self.log_event(
            now_ms,
            "gamefocus",
            "session_completed",
            format!("GameFocus session ended ({ended_name})"),
            format!("{reason} {restore_summary}"),
        );

        self.snapshot().gamefocus
    }

    pub fn tick(
        &mut self,
        settings: &AppSettings,
        foreground: Option<(u32, String)>,
        is_pid_alive: impl Fn(u32) -> bool,
        per_process_gpu: &HashMap<u32, f64>,
        now_ms: u64,
    ) {
        self.gamefocus.enabled = settings.gamefocus_enabled;
        if !settings.gamefocus_enabled {
            if matches!(
                self.gamefocus.lifecycle,
                GameSessionLifecycle::Active | GameSessionLifecycle::TemporarilyBackgrounded
            ) {
                self.deactivate_gamefocus_and_restore(
                    "GameFocus was disabled in settings.",
                    now_ms,
                );
            }
            self.gamefocus.lifecycle = GameSessionLifecycle::Idle;
            self.gamefocus.candidate = None;
            return;
        }

        // Handle active or temporarily backgrounded session
        if matches!(
            self.gamefocus.lifecycle,
            GameSessionLifecycle::Active | GameSessionLifecycle::TemporarilyBackgrounded
        ) {
            if let Some(started) = self.gamefocus.session_started_ms {
                self.gamefocus.session_duration_ms = now_ms.saturating_sub(started);
            }

            // Check if the foreground window switched to a newly respawned PID of the same game profile (launcher handoff / restart)
            if let (Some((fg_pid, fg_path)), Some(profile)) =
                (&foreground, &self.gamefocus.selected_profile)
                && profile.matches_executable(fg_path)
            {
                self.gamefocus.active_pid = Some(*fg_pid);
                self.gamefocus.active_executable_path = Some(fg_path.clone());
            }

            let pid_alive = self.gamefocus.active_pid.map(&is_pid_alive).unwrap_or(true);

            if !pid_alive {
                self.deactivate_gamefocus_and_restore(
                    "Detected game process exit; automatically restoring previous configuration.",
                    now_ms,
                );
                return;
            }

            // Debounce focus changes: if user Alt+Tabs away while the game process is still alive,
            // mark TemporarilyBackgrounded without changing or restoring power schemes.
            let fg_matches_active = match (&foreground, self.gamefocus.active_pid) {
                (Some((fg_pid, _)), Some(active_pid)) => *fg_pid == active_pid,
                (Some((_, fg_path)), None) => self
                    .gamefocus
                    .selected_profile
                    .as_ref()
                    .is_some_and(|p| p.matches_executable(fg_path)),
                _ => false,
            };

            self.gamefocus.lifecycle = if fg_matches_active {
                GameSessionLifecycle::Active
            } else {
                GameSessionLifecycle::TemporarilyBackgrounded
            };

            let custom_protected = self
                .gamefocus
                .selected_profile
                .as_ref()
                .map(|p| p.protected_applications.as_slice())
                .unwrap_or(&[]);
            let (protected, competing, recommendations) = self.workload_analyzer.sample_workloads(
                self.gamefocus.active_pid,
                custom_protected,
                per_process_gpu,
            );
            self.gamefocus.streaming_detected = !protected.is_empty();
            self.gamefocus.active_protected_workloads = protected;
            self.gamefocus.competing_workloads = competing;
            self.gamefocus.recommendations = recommendations;
            return;
        }

        // Not in an active session: inspect foreground application for profile match or candidate detection
        let Some((fg_pid, fg_path)) = foreground else {
            return;
        };

        if self.manual_hold_idle_for_pid == Some(fg_pid) {
            return;
        } else if let Some(held_pid) = self.manual_hold_idle_for_pid
            && !is_pid_alive(held_pid)
        {
            self.manual_hold_idle_for_pid = None;
        }

        if settings.gamefocus_auto_detect
            && let Some(matched_profile) = settings
                .game_profiles
                .iter()
                .find(|profile| profile.matches_executable(&fg_path))
                .cloned()
        {
            let _ = self.activate_gamefocus_session(
                matched_profile,
                Some(fg_pid),
                fg_path,
                settings.powershift_allow_on_battery,
                now_ms,
            );
            return;
        }

        if !is_non_game_executable(&fg_path) {
            let exe_name = fg_path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&fg_path)
                .to_string();
            self.gamefocus.lifecycle = GameSessionLifecycle::CandidateDetected;
            self.gamefocus.candidate = Some(CandidateApplication {
                pid: fg_pid,
                executable_name: exe_name,
                executable_path: fg_path,
            });
        } else {
            self.gamefocus.candidate = None;
            if self.gamefocus.lifecycle == GameSessionLifecycle::CandidateDetected {
                self.gamefocus.lifecycle = GameSessionLifecycle::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamefocus::GamePreset;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_rewind_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("memflux-coord-test-{name}-{nonce}"))
            .join("rewind.json")
    }

    #[test]
    fn gamefocus_session_survives_alt_tab_and_restores_on_process_exit() {
        let path = temp_rewind_path("lifecycle");
        let mut coord = OptimizationCoordinator::new(path.clone(), 1_000);
        let mut profile = GameProfile::from_preset(
            "elden".into(),
            "Elden Ring".into(),
            "eldenring.exe".into(),
            GamePreset::PerformanceGaming,
        );
        // Disable actual OS power mutation in pure state-machine test or use CurrentSystemMode
        profile.powershift_mode = PowerShiftMode::CurrentSystemMode;
        let settings = AppSettings {
            gamefocus_enabled: true,
            gamefocus_auto_detect: true,
            game_profiles: vec![profile],
            ..Default::default()
        };

        // 1. Game enters foreground -> session becomes Active with temporary cleaner override
        coord.tick(
            &settings,
            Some((4242, r"C:\Games\EldenRing\eldenring.exe".into())),
            |pid| pid == 4242,
            &HashMap::new(),
            2_000,
        );
        assert_eq!(coord.gamefocus.lifecycle, GameSessionLifecycle::Active);
        assert_eq!(
            coord.cleaner_override(),
            SessionCleanerOverride::EnableDuringSession
        );
        // Global settings are NOT mutated
        assert!(!settings.automatic_cleaning_enabled);

        // 2. User Alt+Tabs to Chrome while game PID 4242 is still running -> TemporarilyBackgrounded
        coord.tick(
            &settings,
            Some((9999, r"C:\Program Files\Google\Chrome\chrome.exe".into())),
            |pid| pid == 4242,
            &HashMap::new(),
            5_000,
        );
        assert_eq!(
            coord.gamefocus.lifecycle,
            GameSessionLifecycle::TemporarilyBackgrounded
        );
        assert_eq!(
            coord.cleaner_override(),
            SessionCleanerOverride::EnableDuringSession
        );

        // 3. User Alt+Tabs back to game -> Active again
        coord.tick(
            &settings,
            Some((4242, r"C:\Games\EldenRing\eldenring.exe".into())),
            |pid| pid == 4242,
            &HashMap::new(),
            7_000,
        );
        assert_eq!(coord.gamefocus.lifecycle, GameSessionLifecycle::Active);

        // 4. Game process exits (is_pid_alive returns false) -> Restores and completes session
        coord.tick(
            &settings,
            Some((9999, r"C:\Windows\explorer.exe".into())),
            |_| false,
            &HashMap::new(),
            10_000,
        );
        assert_eq!(coord.gamefocus.lifecycle, GameSessionLifecycle::Completed);
        assert_eq!(coord.cleaner_override(), SessionCleanerOverride::KeepGlobal);

        let _ = std::fs::remove_file(&path);
        if let Some(p) = path.parent() {
            let _ = std::fs::remove_dir(p);
        }
    }
}
