use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

const ONE_HOUR_MS: u64 = 60 * 60 * 1_000;
const QUOTA_RETENTION_LIMIT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanerDecision {
    Disabled,
    Paused,
    Unsupported,
    TelemetryUnavailable,
    ExcludedApplication,
    ExclusionStatusUnavailable,
    StandbyBelowThreshold,
    FreeZeroAboveThreshold,
    MinimumInterval,
    Cooldown,
    RateLimited,
    AuthorizationRequired,
    ActionInProgress,
    Trigger,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionCleanerOverride {
    #[default]
    KeepGlobal,
    EnableDuringSession,
    PauseDuringSession,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerStatusSummary {
    pub state: String,
    pub label: String,
    pub decision: CleanerDecision,
    pub authorization_ready: bool,
    pub session_override: SessionCleanerOverride,
    pub purges_last_hour: u32,
    pub maximum_purges_per_hour: u32,
    pub cooldown_remaining_ms: u64,
    pub minimum_interval_remaining_ms: u64,
    pub poll_interval_ms: u64,
    pub last_evaluation_ms: Option<u64>,
    pub detail: String,
}

impl Default for CleanerStatusSummary {
    fn default() -> Self {
        Self {
            state: "disabled".into(),
            label: "Automatic cleaner disabled".into(),
            decision: CleanerDecision::Disabled,
            authorization_ready: false,
            session_override: SessionCleanerOverride::KeepGlobal,
            purges_last_hour: 0,
            maximum_purges_per_hour: 2,
            cooldown_remaining_ms: 0,
            minimum_interval_remaining_ms: 0,
            poll_interval_ms: 5_000,
            last_evaluation_ms: None,
            detail: "Automatic standby cleaning is disabled in settings.".into(),
        }
    }
}

#[derive(Clone, Debug)]
struct CompletedPurgeEntry {
    wall_timestamp_ms: u64,
    monotonic_instant: Option<Instant>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuotaJournalFile {
    schema_version: u32,
    completed_timestamps_ms: Vec<u64>,
}

#[derive(Default)]
pub struct CleanerPolicy {
    completed_purges: VecDeque<CompletedPurgeEntry>,
}

pub struct CleanerInputs<'a> {
    pub now_ms: u64,
    pub now_instant: Instant,
    pub standby_bytes: Option<f64>,
    pub free_zero_bytes: Option<f64>,
    pub foreground_executable: Option<&'a str>,
    pub authorization_ready: bool,
    pub build_supported: bool,
    pub action_in_progress: bool,
    pub session_override: SessionCleanerOverride,
}

#[cfg(test)]
impl<'a> CleanerInputs<'a> {
    pub fn simple(
        now_ms: u64,
        standby_bytes: Option<f64>,
        free_zero_bytes: Option<f64>,
        foreground_executable: Option<&'a str>,
        authorization_ready: bool,
    ) -> Self {
        Self {
            now_ms,
            now_instant: Instant::now(),
            standby_bytes,
            free_zero_bytes,
            foreground_executable,
            authorization_ready,
            build_supported: true,
            action_in_progress: false,
            session_override: SessionCleanerOverride::KeepGlobal,
        }
    }
}

impl CleanerPolicy {
    fn elapsed_since(entry: &CompletedPurgeEntry, now_ms: u64, now_instant: Instant) -> u64 {
        let wall_elapsed = now_ms.saturating_sub(entry.wall_timestamp_ms);
        if let Some(instant) = entry.monotonic_instant {
            now_instant.saturating_duration_since(instant).as_millis() as u64
        } else {
            wall_elapsed
        }
    }

    pub fn prune(&mut self, now_ms: u64, now_instant: Instant) {
        self.completed_purges
            .retain(|entry| Self::elapsed_since(entry, now_ms, now_instant) < ONE_HOUR_MS);
    }

    pub fn evaluate(
        &mut self,
        settings: &AppSettings,
        inputs: CleanerInputs<'_>,
    ) -> CleanerDecision {
        self.prune(inputs.now_ms, inputs.now_instant);

        let effective_enabled = match inputs.session_override {
            SessionCleanerOverride::KeepGlobal => settings.automatic_cleaning_enabled,
            SessionCleanerOverride::EnableDuringSession => true,
            SessionCleanerOverride::PauseDuringSession => settings.automatic_cleaning_enabled,
        };
        let effective_paused = match inputs.session_override {
            SessionCleanerOverride::PauseDuringSession => true,
            _ => settings.cleaner_paused,
        };

        if !effective_enabled {
            return CleanerDecision::Disabled;
        }
        if !inputs.build_supported {
            return CleanerDecision::Unsupported;
        }
        if effective_paused {
            return CleanerDecision::Paused;
        }
        if inputs.action_in_progress {
            return CleanerDecision::ActionInProgress;
        }
        let (Some(standby), Some(free_zero)) = (inputs.standby_bytes, inputs.free_zero_bytes)
        else {
            return CleanerDecision::TelemetryUnavailable;
        };
        if !standby.is_finite() || !free_zero.is_finite() || standby < 0.0 || free_zero < 0.0 {
            return CleanerDecision::TelemetryUnavailable;
        }
        if !settings.cleaner_excluded_applications.is_empty()
            && inputs.foreground_executable.is_none()
        {
            return CleanerDecision::ExclusionStatusUnavailable;
        }
        if inputs.foreground_executable.is_some_and(|executable| {
            let executable = executable.to_ascii_lowercase();
            settings
                .cleaner_excluded_applications
                .iter()
                .any(|excluded| {
                    if excluded.contains(['\\', '/']) {
                        executable == *excluded
                    } else {
                        executable.rsplit(['\\', '/']).next() == Some(excluded.as_str())
                    }
                })
        }) {
            return CleanerDecision::ExcludedApplication;
        }
        if let Some(last) = self.completed_purges.back() {
            let elapsed = Self::elapsed_since(last, inputs.now_ms, inputs.now_instant);
            if elapsed < settings.cleaner_minimum_interval_ms {
                return CleanerDecision::MinimumInterval;
            }
            if elapsed < settings.cleaner_cooldown_ms {
                return CleanerDecision::Cooldown;
            }
        }
        if self.completed_purges.len() >= settings.maximum_purges_per_hour as usize {
            return CleanerDecision::RateLimited;
        }
        if standby < settings.standby_threshold_bytes as f64 {
            return CleanerDecision::StandbyBelowThreshold;
        }
        if free_zero > settings.free_zero_threshold_bytes as f64 {
            return CleanerDecision::FreeZeroAboveThreshold;
        }
        if !inputs.authorization_ready {
            return CleanerDecision::AuthorizationRequired;
        }
        CleanerDecision::Trigger
    }

    pub fn evaluate_summary(
        &mut self,
        settings: &AppSettings,
        inputs: CleanerInputs<'_>,
    ) -> CleanerStatusSummary {
        let now_ms = inputs.now_ms;
        let now_instant = inputs.now_instant;
        let auth_ready = inputs.authorization_ready;
        let session_override = inputs.session_override;
        let decision = self.evaluate(settings, inputs);

        let (min_rem, cool_rem) = if let Some(last) = self.completed_purges.back() {
            let elapsed = Self::elapsed_since(last, now_ms, now_instant);
            (
                settings.cleaner_minimum_interval_ms.saturating_sub(elapsed),
                settings.cleaner_cooldown_ms.saturating_sub(elapsed),
            )
        } else {
            (0, 0)
        };
        let purges_last_hour = self.completed_purges.len() as u32;

        let (state, label, detail) = match decision {
            CleanerDecision::Disabled => (
                "disabled",
                "Automatic cleaner disabled",
                "Automatic standby cleaning is turned off. Manual purging remains available.",
            ),
            CleanerDecision::Paused => (
                "paused",
                "Cleaner paused",
                "Automatic standby evaluation is paused while telemetry continues.",
            ),
            CleanerDecision::Unsupported => (
                "unsupported",
                "Unsupported Windows build",
                "This Windows build is outside the validated allowlist; automatic cleaning is disabled.",
            ),
            CleanerDecision::ActionInProgress => (
                "cleaning_in_progress",
                "Cleaning in progress",
                "A standby-list purge and post-action settling measurement are currently running.",
            ),
            CleanerDecision::TelemetryUnavailable => (
                "error",
                "Telemetry unavailable",
                "Standby or Free & Zero counters are warming up or unavailable; automatic cleaning is suspended.",
            ),
            CleanerDecision::ExclusionStatusUnavailable => (
                "excluded_application",
                "Exclusion check pending",
                "Foreground executable identity could not be verified while application exclusions are configured.",
            ),
            CleanerDecision::ExcludedApplication => (
                "excluded_application",
                "Excluded foreground app active",
                "The current foreground application is in the cleaner exclusion list.",
            ),
            CleanerDecision::MinimumInterval | CleanerDecision::Cooldown => (
                "cooldown_active",
                "Cooldown active",
                "Waiting for the configured post-purge cooldown or minimum interval to elapse.",
            ),
            CleanerDecision::RateLimited => (
                "rate_limited",
                "Hourly rate limit reached",
                "Maximum completed purges per rolling hour reached; automatic cleaning will resume when quota frees.",
            ),
            CleanerDecision::StandbyBelowThreshold | CleanerDecision::FreeZeroAboveThreshold => {
                if auth_ready {
                    (
                        "thresholds_not_met",
                        "Authorized & monitoring (thresholds not met)",
                        "Authenticated session helper is ready. Standby and Free & Zero thresholds are not simultaneously met.",
                    )
                } else {
                    (
                        "configured_unauthorized",
                        "Configured — not authorized",
                        "Automatic policy is enabled, but an elevated cleaning session has not been authorized yet.",
                    )
                }
            }
            CleanerDecision::AuthorizationRequired => (
                "configured_unauthorized",
                "Thresholds met — awaiting authorization",
                "Both memory thresholds are met, but automatic execution requires an authorized elevated session helper.",
            ),
            CleanerDecision::Trigger => (
                "action_pending",
                "Action pending",
                "Both standby and Free & Zero thresholds are met and authorized; automatic purge is triggering.",
            ),
            CleanerDecision::Error => (
                "error",
                "Cleaner error",
                "The last automatic purge attempt encountered an error.",
            ),
        };

        CleanerStatusSummary {
            state: state.into(),
            label: label.into(),
            decision,
            authorization_ready: auth_ready,
            session_override,
            purges_last_hour,
            maximum_purges_per_hour: settings.maximum_purges_per_hour,
            cooldown_remaining_ms: cool_rem,
            minimum_interval_remaining_ms: min_rem,
            poll_interval_ms: settings.cleaner_poll_interval_ms,
            last_evaluation_ms: Some(now_ms),
            detail: detail.into(),
        }
    }

    #[cfg(test)]
    pub fn record_completed_purge(&mut self, now_ms: u64) {
        self.completed_purges.push_back(CompletedPurgeEntry {
            wall_timestamp_ms: now_ms,
            monotonic_instant: None,
        });
        while self.completed_purges.len() > QUOTA_RETENTION_LIMIT {
            self.completed_purges.pop_front();
        }
    }

    pub fn record_completed_purge_monotonic(&mut self, now_ms: u64, now_instant: Instant) {
        self.completed_purges.push_back(CompletedPurgeEntry {
            wall_timestamp_ms: now_ms,
            monotonic_instant: Some(now_instant),
        });
        while self.completed_purges.len() > QUOTA_RETENTION_LIMIT {
            self.completed_purges.pop_front();
        }
    }

    /// Restores persisted purge timestamps. When `anchor` is provided (the
    /// load path), each entry is back-dated on a synthetic monotonic timeline
    /// so cooldown/rate expiry cannot be defeated by wall-clock changes:
    /// elapsed time after load is measured monotonically, and the entry's
    /// pre-load age is the clamped wall age. Entries already older than one
    /// hour by the wall clock are pruned on restore. Timestamps in the future
    /// clamp to the anchor (treated as just-completed — conservative).
    fn restore_completed_purges_at(
        &mut self,
        timestamps: impl Iterator<Item = u64>,
        anchor: Option<(u64, Instant)>,
    ) {
        let mut timestamps = timestamps.collect::<Vec<_>>();
        timestamps.sort_unstable();
        timestamps.dedup();
        if timestamps.len() > QUOTA_RETENTION_LIMIT {
            timestamps.drain(0..(timestamps.len() - QUOTA_RETENTION_LIMIT));
        }
        self.completed_purges = timestamps
            .into_iter()
            .filter_map(|wall_timestamp_ms| {
                let entry = if let Some((anchor_ms, anchor_instant)) = anchor {
                    let clamped = wall_timestamp_ms.min(anchor_ms);
                    let age_ms = anchor_ms.saturating_sub(clamped);
                    if age_ms >= ONE_HOUR_MS {
                        return None;
                    }
                    CompletedPurgeEntry {
                        wall_timestamp_ms: clamped,
                        monotonic_instant: anchor_instant
                            .checked_sub(Duration::from_millis(age_ms)),
                    }
                } else {
                    CompletedPurgeEntry {
                        wall_timestamp_ms,
                        monotonic_instant: None,
                    }
                };
                Some(entry)
            })
            .collect();
    }

    #[cfg(test)]
    pub fn restore_completed_purges(&mut self, timestamps: impl Iterator<Item = u64>) {
        self.restore_completed_purges_at(timestamps, None);
    }

    pub fn load_quota_file(&mut self, path: &Path, history_timestamps: impl Iterator<Item = u64>) {
        let mut combined: Vec<u64> = history_timestamps.collect();
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(file) = serde_json::from_str::<QuotaJournalFile>(&raw)
            && file.schema_version == 1
        {
            combined.extend(file.completed_timestamps_ms);
        }
        let anchor = (crate::telemetry::timestamp_ms(), Instant::now());
        self.restore_completed_purges_at(combined.into_iter(), Some(anchor));
    }

    pub fn persist_quota_file(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Quota journal path has no parent directory.".to_string())?;
        fs::create_dir_all(parent).map_err(|e| format!("Could not create quota directory: {e}"))?;
        let temp = path.with_extension("json.new");
        let payload = serde_json::to_vec_pretty(&QuotaJournalFile {
            schema_version: 1,
            completed_timestamps_ms: self
                .completed_purges
                .iter()
                .map(|entry| entry.wall_timestamp_ms)
                .collect(),
        })
        .map_err(|e| format!("Could not serialize quota journal: {e}"))?;
        let mut file =
            File::create(&temp).map_err(|e| format!("Could not create temp quota file: {e}"))?;
        file.write_all(&payload)
            .map_err(|e| format!("Could not write temp quota file: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Could not flush temp quota file: {e}"))?;
        drop(file);
        crate::settings::atomic_replace(&temp, path)
            .map_err(|e| format!("Could not replace quota file: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn eligible_settings() -> AppSettings {
        AppSettings {
            automatic_cleaning_enabled: true,
            standby_threshold_bytes: 1_000,
            free_zero_threshold_bytes: 500,
            cleaner_cooldown_ms: 600_000,
            cleaner_minimum_interval_ms: 300_000,
            maximum_purges_per_hour: 2,
            ..Default::default()
        }
    }

    fn inputs(now_ms: u64) -> CleanerInputs<'static> {
        CleanerInputs::simple(now_ms, Some(1_001.0), Some(499.0), None, true)
    }

    #[test]
    fn requires_both_threshold_conditions() {
        let settings = eligible_settings();
        let mut policy = CleanerPolicy::default();
        assert_eq!(
            policy.evaluate(&settings, inputs(1)),
            CleanerDecision::Trigger
        );
        assert_eq!(
            policy.evaluate(
                &settings,
                CleanerInputs {
                    standby_bytes: Some(999.0),
                    ..inputs(1)
                }
            ),
            CleanerDecision::StandbyBelowThreshold
        );
        assert_eq!(
            policy.evaluate(
                &settings,
                CleanerInputs {
                    free_zero_bytes: Some(501.0),
                    ..inputs(1)
                }
            ),
            CleanerDecision::FreeZeroAboveThreshold
        );
    }

    #[test]
    fn authorization_and_exclusions_block_trigger() {
        let mut settings = eligible_settings();
        settings.cleaner_excluded_applications = vec!["obs64.exe".into()];
        let mut policy = CleanerPolicy::default();
        assert_eq!(
            policy.evaluate(
                &settings,
                CleanerInputs {
                    foreground_executable: Some("C:\\Program Files\\obs64.exe"),
                    ..inputs(1)
                }
            ),
            CleanerDecision::ExcludedApplication
        );
        assert_eq!(
            policy.evaluate(
                &settings,
                CleanerInputs {
                    authorization_ready: false,
                    foreground_executable: Some("game.exe"),
                    ..inputs(1)
                }
            ),
            CleanerDecision::AuthorizationRequired
        );
    }

    #[test]
    fn cooldown_and_hourly_rate_limit_are_enforced() {
        let settings = eligible_settings();
        let mut policy = CleanerPolicy::default();
        policy.record_completed_purge(1_000_000);
        assert_eq!(
            policy.evaluate(&settings, inputs(1_100_000)),
            CleanerDecision::MinimumInterval
        );
        assert_eq!(
            policy.evaluate(&settings, inputs(1_400_000)),
            CleanerDecision::Cooldown
        );
        policy.record_completed_purge(1_700_000);
        assert_eq!(
            policy.evaluate(&settings, inputs(2_400_000)),
            CleanerDecision::RateLimited
        );
        assert_eq!(
            policy.evaluate(&settings, inputs(5_000_001)),
            CleanerDecision::Trigger
        );
    }

    #[test]
    fn exclusions_require_identity_and_exact_filename() {
        let mut settings = eligible_settings();
        settings.cleaner_excluded_applications = vec!["obs64.exe".into()];
        let mut policy = CleanerPolicy::default();
        assert_eq!(
            policy.evaluate(&settings, inputs(1)),
            CleanerDecision::ExclusionStatusUnavailable
        );
        assert_eq!(
            policy.evaluate(
                &settings,
                CleanerInputs {
                    foreground_executable: Some("C:\\apps\\notobs64.exe"),
                    ..inputs(1)
                }
            ),
            CleanerDecision::Trigger
        );
        settings.cleaner_paused = true;
        assert_eq!(
            policy.evaluate(&settings, inputs(1)),
            CleanerDecision::Paused
        );
        settings.automatic_cleaning_enabled = false;
        assert_eq!(
            policy.evaluate(&settings, inputs(1)),
            CleanerDecision::Disabled
        );
    }

    #[test]
    fn persisted_manual_actions_and_monotonic_clock_protect_cooldown() {
        let settings = eligible_settings();
        let mut policy = CleanerPolicy::default();
        policy.restore_completed_purges([1_000_000, 900_000].into_iter());
        assert_eq!(
            policy.evaluate(&settings, inputs(1_100_000)),
            CleanerDecision::MinimumInterval
        );
        // Even if system wall clock jumps forward by 10 hours, monotonic instant within the same session enforces cooldown
        let t0 = Instant::now();
        let mut mono_policy = CleanerPolicy::default();
        mono_policy.record_completed_purge_monotonic(1_000_000, t0);
        let jumped_inputs = CleanerInputs {
            now_ms: 99_000_000,
            now_instant: t0 + Duration::from_secs(30),
            ..inputs(99_000_000)
        };
        assert_eq!(
            mono_policy.evaluate(&settings, jumped_inputs),
            CleanerDecision::MinimumInterval
        );
    }

    #[test]
    fn quota_file_restore_anchors_entries_on_monotonic_clock() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let settings = eligible_settings();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("memflux-quota-test-{nonce}"));
        let path = dir.join("cleaner-quota.json");
        let now = crate::telemetry::timestamp_ms();
        let journal = QuotaJournalFile {
            schema_version: 1,
            completed_timestamps_ms: vec![
                now - 5_000,           // 5s ago — still inside cooldown
                now + 60 * 60_000,     // 1h in the future — clamps to load moment
                now - 2 * 60 * 60_000, // 2h ago — expired, must not count
            ],
        };
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(&path, serde_json::to_vec(&journal).expect("json")).expect("write");

        let mut policy = CleanerPolicy::default();
        policy.load_quota_file(&path, std::iter::empty());
        // Two retained entries (future-clamped + recent) hit the hourly cap of 2.
        assert_eq!(policy.completed_purges.len(), 2);

        // Wall clock jumps 10h forward but only ~30s of real time passed:
        // the last purge's cooldown must still hold because expiry is monotonic.
        let jumped = CleanerInputs {
            now_ms: now.saturating_add(10 * 60 * 60_000),
            now_instant: Instant::now() + Duration::from_secs(30),
            ..inputs(0)
        };
        assert_eq!(
            policy.evaluate(&settings, jumped),
            CleanerDecision::MinimumInterval // ~30s real elapsed < 300s minimum interval
        );

        // After the monotonic hour truly passes, entries expire.
        let later = CleanerInputs {
            now_ms: now.saturating_add(61 * 60_000),
            now_instant: Instant::now() + Duration::from_secs(61 * 60),
            ..inputs(0)
        };
        assert_eq!(policy.evaluate(&settings, later), CleanerDecision::Trigger);

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(dir);
    }
}
