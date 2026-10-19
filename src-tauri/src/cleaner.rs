use crate::settings::AppSettings;
use serde::Serialize;
use std::collections::VecDeque;

const ONE_HOUR_MS: u64 = 60 * 60 * 1_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanerDecision {
    Disabled,
    Paused,
    TelemetryUnavailable,
    ExcludedApplication,
    ExclusionStatusUnavailable,
    StandbyBelowThreshold,
    FreeZeroAboveThreshold,
    MinimumInterval,
    Cooldown,
    RateLimited,
    AuthorizationRequired,
    Trigger,
}

#[derive(Default)]
pub struct CleanerPolicy {
    completed_purges_ms: VecDeque<u64>,
}

pub struct CleanerInputs<'a> {
    pub now_ms: u64,
    pub standby_bytes: Option<f64>,
    pub free_zero_bytes: Option<f64>,
    pub foreground_executable: Option<&'a str>,
    pub authorization_ready: bool,
}

impl CleanerPolicy {
    pub fn evaluate(
        &mut self,
        settings: &AppSettings,
        inputs: CleanerInputs<'_>,
    ) -> CleanerDecision {
        self.completed_purges_ms
            .retain(|timestamp| inputs.now_ms.saturating_sub(*timestamp) < ONE_HOUR_MS);
        if !settings.automatic_cleaning_enabled {
            return CleanerDecision::Disabled;
        }
        if settings.cleaner_paused {
            return CleanerDecision::Paused;
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
        if standby < settings.standby_threshold_bytes as f64 {
            return CleanerDecision::StandbyBelowThreshold;
        }
        if free_zero > settings.free_zero_threshold_bytes as f64 {
            return CleanerDecision::FreeZeroAboveThreshold;
        }
        if let Some(last) = self.completed_purges_ms.back().copied() {
            let elapsed = inputs.now_ms.saturating_sub(last);
            if elapsed < settings.cleaner_minimum_interval_ms {
                return CleanerDecision::MinimumInterval;
            }
            if elapsed < settings.cleaner_cooldown_ms {
                return CleanerDecision::Cooldown;
            }
        }
        if self.completed_purges_ms.len() >= settings.maximum_purges_per_hour as usize {
            return CleanerDecision::RateLimited;
        }
        if !inputs.authorization_ready {
            return CleanerDecision::AuthorizationRequired;
        }
        CleanerDecision::Trigger
    }

    #[cfg(test)]
    pub fn record_completed_purge(&mut self, now_ms: u64) {
        self.completed_purges_ms.push_back(now_ms);
    }

    pub fn restore_completed_purges(&mut self, timestamps: impl Iterator<Item = u64>) {
        let mut timestamps = timestamps.collect::<Vec<_>>();
        timestamps.sort_unstable();
        self.completed_purges_ms = timestamps.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        CleanerInputs {
            now_ms,
            standby_bytes: Some(1_001.0),
            free_zero_bytes: Some(499.0),
            foreground_executable: None,
            authorization_ready: true,
        }
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
    fn persisted_manual_actions_participate_in_cooldown() {
        let settings = eligible_settings();
        let mut policy = CleanerPolicy::default();
        policy.restore_completed_purges([1_000_000, 900_000].into_iter());
        assert_eq!(
            policy.evaluate(&settings, inputs(1_100_000)),
            CleanerDecision::MinimumInterval
        );
    }
}
