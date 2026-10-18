use crate::telemetry::{CollectionStatus, MemorySnapshot, OsMemoryState};
use serde::Serialize;

const REQUIRED_CRITICAL_SAMPLES: u32 = 3;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuardianOutcome {
    Observe,
    Recommend,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PressureLevel {
    Comfortable,
    Watch,
    Elevated,
    Critical,
    Unknown,
    Paused,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianDecision {
    pub outcome: GuardianOutcome,
    pub pressure: PressureLevel,
    pub reason_code: String,
    pub explanation: String,
    pub evidence: Vec<String>,
    pub confidence: String,
    pub sustained_samples: u32,
    pub action_capability: String,
}

pub struct GuardianEvaluator {
    consecutive_critical: u32,
}

impl GuardianEvaluator {
    pub fn new() -> Self {
        Self {
            consecutive_critical: 0,
        }
    }

    pub fn reset(&mut self) {
        self.consecutive_critical = 0;
    }

    pub fn evaluate(&mut self, snapshot: &MemorySnapshot, enabled: bool) -> GuardianDecision {
        if !enabled || snapshot.collection_status == CollectionStatus::Paused {
            self.reset();
            return decision(
                GuardianOutcome::Observe,
                PressureLevel::Paused,
                "guardian_paused",
                "Guardian monitoring is paused. No intervention can occur.",
                vec![],
                "not_evaluated",
                0,
            );
        }

        let (Some(total), Some(available), Some(commit_percent)) = (
            snapshot.total_physical_bytes.value,
            snapshot.available_physical_bytes.value,
            snapshot.commit_percent.value,
        ) else {
            self.reset();
            return decision(
                GuardianOutcome::Observe,
                PressureLevel::Unknown,
                "core_telemetry_unavailable",
                "Core memory telemetry is unavailable, so Guardian has suspended policy evaluation.",
                vec!["No action is permitted while core telemetry is unreliable.".into()],
                "insufficient",
                0,
            );
        };

        if total <= 0.0 || !total.is_finite() || !available.is_finite() {
            self.reset();
            return decision(
                GuardianOutcome::Observe,
                PressureLevel::Unknown,
                "core_telemetry_invalid",
                "Core memory telemetry failed validation, so Guardian is observing only.",
                vec![],
                "insufficient",
                0,
            );
        }

        let available_percent = (available / total * 100.0).clamp(0.0, 100.0);
        let page_reads = snapshot.page_reads_per_second.value;
        let os_low = snapshot.os_memory_state == OsMemoryState::Low;
        let paging_pressure = page_reads.is_some_and(|value| value >= 20.0);
        let critical =
            available_percent < 5.0 && commit_percent >= 90.0 && (os_low || paging_pressure);

        if critical {
            self.consecutive_critical = self.consecutive_critical.saturating_add(1);
            let evidence = vec![
                format!("Available physical memory: {available_percent:.1}%"),
                format!("Commit utilization: {commit_percent:.1}%"),
                page_reads
                    .map(|value| format!("Hard page-read activity: {value:.1} reads/s"))
                    .unwrap_or_else(|| "Hard page-read activity: unavailable".into()),
                format!(
                    "Windows low-memory signal: {}",
                    if os_low { "active" } else { "not active" }
                ),
            ];

            if self.consecutive_critical >= REQUIRED_CRITICAL_SAMPLES {
                return decision(
                    GuardianOutcome::Recommend,
                    PressureLevel::Critical,
                    "sustained_critical_pressure",
                    "Sustained memory pressure is present. Review memory-heavy workloads; Phase 1 will not perform a memory action.",
                    evidence,
                    if page_reads.is_some() {
                        "high"
                    } else {
                        "reduced"
                    },
                    self.consecutive_critical,
                );
            }

            return decision(
                GuardianOutcome::Observe,
                PressureLevel::Critical,
                "critical_pressure_pending_hysteresis",
                "A critical-pressure sample was observed, but Guardian requires sustained evidence before recommending action.",
                evidence,
                if page_reads.is_some() {
                    "high"
                } else {
                    "reduced"
                },
                self.consecutive_critical,
            );
        }

        self.consecutive_critical = 0;

        if os_low || available_percent < 10.0 || commit_percent >= 85.0 {
            return decision(
                GuardianOutcome::Observe,
                PressureLevel::Elevated,
                "elevated_pressure_observe",
                "Memory pressure is elevated, but the evidence does not justify an intervention. Monitoring continues.",
                vec![
                    format!("Available physical memory: {available_percent:.1}%"),
                    format!("Commit utilization: {commit_percent:.1}%"),
                ],
                if page_reads.is_some() {
                    "high"
                } else {
                    "reduced"
                },
                0,
            );
        }

        if available_percent < 15.0 || commit_percent >= 75.0 {
            return decision(
                GuardianOutcome::Observe,
                PressureLevel::Watch,
                "pressure_watch",
                "Memory headroom is narrowing. Guardian is watching the trend; no action is justified.",
                vec![
                    format!("Available physical memory: {available_percent:.1}%"),
                    format!("Commit utilization: {commit_percent:.1}%"),
                ],
                if page_reads.is_some() {
                    "high"
                } else {
                    "reduced"
                },
                0,
            );
        }

        let standby_note = match (snapshot.standby_bytes.value, available > 0.0) {
            (Some(standby), true) if standby / available > 0.45 => {
                " Standby cache is substantial, but it is already included in available memory and is not a problem by itself."
            }
            _ => "",
        };
        decision(
            GuardianOutcome::Observe,
            PressureLevel::Comfortable,
            "memory_headroom_healthy",
            &format!(
                "Memory headroom and commit capacity are healthy. No action is needed.{standby_note}"
            ),
            vec![
                format!("Available physical memory: {available_percent:.1}%"),
                format!("Commit utilization: {commit_percent:.1}%"),
            ],
            if page_reads.is_some() {
                "high"
            } else {
                "reduced"
            },
            0,
        )
    }
}

fn decision(
    outcome: GuardianOutcome,
    pressure: PressureLevel,
    reason_code: &str,
    explanation: &str,
    evidence: Vec<String>,
    confidence: &str,
    sustained_samples: u32,
) -> GuardianDecision {
    GuardianDecision {
        outcome,
        pressure,
        reason_code: reason_code.into(),
        explanation: explanation.into(),
        evidence,
        confidence: confidence.into(),
        sustained_samples,
        action_capability: "unsupported_monitoring_only".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{Metric, MetricStatus};

    fn metric(id: &str, value: Option<f64>) -> Metric {
        match value {
            Some(value) => Metric::available(id, value, "bytes", "fixture", 1, 1_000),
            None => Metric::missing(
                id,
                "bytes",
                "fixture",
                1,
                1_000,
                MetricStatus::Unavailable,
                "fixture unavailable",
            ),
        }
    }

    fn snapshot(available_percent: f64, commit_percent: f64, reads: Option<f64>) -> MemorySnapshot {
        let total = 100.0;
        MemorySnapshot {
            sequence: 1,
            timestamp_ms: 1,
            sample_interval_ms: 1_000,
            collection_status: CollectionStatus::Monitoring,
            collection_detail: None,
            os_memory_state: OsMemoryState::Neutral,
            total_physical_bytes: metric("total", Some(total)),
            available_physical_bytes: metric("available", Some(available_percent)),
            non_available_physical_bytes: metric("used", Some(total - available_percent)),
            physical_load_percent: metric("load", Some(100.0 - available_percent)),
            commit_used_bytes: metric("commit_used", Some(commit_percent)),
            commit_limit_bytes: metric("commit_limit", Some(100.0)),
            commit_percent: metric("commit_percent", Some(commit_percent)),
            standby_bytes: metric("standby", Some(available_percent / 2.0)),
            modified_bytes: metric("modified", Some(1.0)),
            free_zero_bytes: metric("free", Some(available_percent / 2.0)),
            page_reads_per_second: metric("reads", reads),
            process_count: metric("processes", Some(100.0)),
            thread_count: metric("threads", Some(1_000.0)),
        }
    }

    #[test]
    fn healthy_memory_is_observe_only() {
        let mut evaluator = GuardianEvaluator::new();
        let result = evaluator.evaluate(&snapshot(40.0, 50.0, Some(0.0)), true);
        assert_eq!(result.outcome, GuardianOutcome::Observe);
        assert_eq!(result.pressure, PressureLevel::Comfortable);
        assert_eq!(result.reason_code, "memory_headroom_healthy");
    }

    #[test]
    fn critical_pressure_requires_three_samples() {
        let mut evaluator = GuardianEvaluator::new();
        let critical = snapshot(4.0, 92.0, Some(30.0));
        for _ in 0..2 {
            let result = evaluator.evaluate(&critical, true);
            assert_eq!(result.outcome, GuardianOutcome::Observe);
            assert_eq!(result.reason_code, "critical_pressure_pending_hysteresis");
        }
        let result = evaluator.evaluate(&critical, true);
        assert_eq!(result.outcome, GuardianOutcome::Recommend);
        assert_eq!(result.reason_code, "sustained_critical_pressure");
    }

    #[test]
    fn healthy_sample_resets_hysteresis() {
        let mut evaluator = GuardianEvaluator::new();
        let critical = snapshot(4.0, 92.0, Some(30.0));
        evaluator.evaluate(&critical, true);
        evaluator.evaluate(&snapshot(40.0, 50.0, Some(0.0)), true);
        let result = evaluator.evaluate(&critical, true);
        assert_eq!(result.sustained_samples, 1);
    }

    #[test]
    fn missing_core_metric_suspends_policy() {
        let mut evaluator = GuardianEvaluator::new();
        let mut missing = snapshot(40.0, 50.0, Some(0.0));
        missing.available_physical_bytes.value = None;
        let result = evaluator.evaluate(&missing, true);
        assert_eq!(result.pressure, PressureLevel::Unknown);
        assert_eq!(result.reason_code, "core_telemetry_unavailable");
    }
}
