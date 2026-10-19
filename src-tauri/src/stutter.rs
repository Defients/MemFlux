//! Stutter Detective — online frame-time anomaly detection over real
//! PresentMon `MsBetweenPresents` samples. Detection compares each frame
//! against a rolling baseline of recent *non-stutter* frames; correlation with
//! CPU/GPU/memory telemetry happens at display time by timestamp. Correlation
//! is reported as correlation only — never as causal proof.

use crate::performance::PresentedFrame;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Frames retained for the rolling baseline estimate.
const BASELINE_WINDOW: usize = 512;
/// Stutter events retained for UI/reporting.
const EVENT_LIMIT: usize = 64;
/// Minimum baseline frames before anomaly flagging (warm-up).
const MIN_BASELINE_FRAMES: usize = 64;
/// A frame flags as stutter when its duration exceeds
/// max(baseline_p95 * RATIO, baseline_p95 + FLOOR_MS).
const STUTTER_RATIO: f64 = 1.8;
const STUTTER_FLOOR_MS: f64 = 7.0;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StutterSeverity {
    /// Dropped present reported by the provider.
    DroppedPresent,
    /// 1.8x–2.6x the rolling P95 baseline.
    Mild,
    /// 2.6x–4x the baseline.
    Moderate,
    /// >4x the baseline (or >100 ms absolute frame time).
    Severe,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StutterEvent {
    pub id: String,
    pub at_ms: u64,
    pub frame_time_ms: f64,
    pub baseline_p95_ms: f64,
    pub ratio: f64,
    pub dropped: bool,
    pub severity: StutterSeverity,
    /// Honest framing for the UI: this is a correlated observation, not proof.
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StutterSummary {
    pub events_last_minute: u32,
    pub worst_ratio_last_minute: Option<f64>,
    pub baseline_p95_ms: Option<f64>,
    pub baseline_frames: usize,
}

pub struct StutterDetector {
    /// Recent non-anomalous frame times used as the live baseline.
    baseline: VecDeque<f64>,
    events: VecDeque<StutterEvent>,
    frames_seen: u64,
    event_seq: u64,
}

impl Default for StutterDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StutterDetector {
    pub fn new() -> Self {
        Self {
            baseline: VecDeque::with_capacity(BASELINE_WINDOW),
            events: VecDeque::new(),
            frames_seen: 0,
            event_seq: 0,
        }
    }

    fn baseline_p95(&self) -> Option<f64> {
        if self.baseline.len() < MIN_BASELINE_FRAMES {
            return None;
        }
        let mut sorted: Vec<f64> = self.baseline.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() as f64 * 0.95).floor() as usize).min(sorted.len() - 1);
        Some(sorted[idx])
    }

    /// Feed one presented frame. Emits an event when the frame is anomalous
    /// against the live baseline; normal frames extend the baseline.
    pub fn observe(&mut self, frame: &PresentedFrame, now_ms: u64) -> Option<StutterEvent> {
        self.frames_seen += 1;
        let ms = frame.ms_between_presents;
        let baseline = match self.baseline_p95() {
            Some(b) if b.is_finite() && b > 0.0 => b,
            _ => {
                self.push_baseline(ms);
                return None;
            }
        };

        let flag_threshold = (baseline * STUTTER_RATIO).max(baseline + STUTTER_FLOOR_MS);
        let is_stutter = ms > flag_threshold || frame.dropped;
        if !is_stutter {
            self.push_baseline(ms);
            return None;
        }

        self.event_seq += 1;
        let ratio = ms / baseline;
        let severity = if frame.dropped {
            StutterSeverity::DroppedPresent
        } else if ms >= 100.0 || ratio >= 4.0 {
            StutterSeverity::Severe
        } else if ratio >= 2.6 {
            StutterSeverity::Moderate
        } else {
            StutterSeverity::Mild
        };
        let event = StutterEvent {
            id: format!("stutter-{}-{}", frame.received_ms, self.event_seq),
            at_ms: frame.received_ms.max(now_ms),
            frame_time_ms: ms,
            baseline_p95_ms: baseline,
            ratio,
            dropped: frame.dropped,
            severity,
            note: "Correlated frame-time anomaly; see matching CPU/GPU/memory telemetry — correlation is not causation.".into(),
        };
        if self.events.len() >= EVENT_LIMIT {
            self.events.pop_front();
        }
        self.events.push_back(event.clone());
        // Stutter frames do not enter the baseline (they would drag it up and
        // mask sustained degradation).
        Some(event)
    }

    fn push_baseline(&mut self, ms: f64) {
        if self.baseline.len() >= BASELINE_WINDOW {
            self.baseline.pop_front();
        }
        self.baseline.push_back(ms);
    }

    pub fn recent_events(&self, limit: usize) -> Vec<StutterEvent> {
        self.events
            .iter()
            .rev()
            .take(limit)
            .rev()
            .cloned()
            .collect()
    }

    pub fn summary(&self, now_ms: u64) -> StutterSummary {
        let recent: Vec<&StutterEvent> = self
            .events
            .iter()
            .filter(|e| now_ms.saturating_sub(e.at_ms) <= 60_000)
            .collect();
        StutterSummary {
            events_last_minute: recent.len() as u32,
            worst_ratio_last_minute: recent.iter().map(|e| e.ratio).reduce(f64::max),
            baseline_p95_ms: self.baseline_p95(),
            baseline_frames: self.baseline.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ms: f64, dropped: bool, received: u64) -> PresentedFrame {
        PresentedFrame {
            ms_between_presents: ms,
            dropped,
            received_ms: received,
            until_displayed_ms: None,
        }
    }

    #[test]
    fn warmup_never_flags() {
        let mut detector = StutterDetector::new();
        for i in 0..MIN_BASELINE_FRAMES as u64 {
            assert!(detector.observe(&frame(16.6, false, i), i).is_none());
        }
        // Even a huge frame during warm-up is not flagged (baseline building).
        let mut fresh = StutterDetector::new();
        assert!(fresh.observe(&frame(120.0, false, 1), 1).is_none());
    }

    #[test]
    fn outlier_frames_flag_and_stay_out_of_baseline() {
        let mut detector = StutterDetector::new();
        for i in 0..100 {
            detector.observe(&frame(16.6, false, i), i);
        }
        let event = detector
            .observe(&frame(50.0, false, 1_000), 1_000)
            .expect("3x baseline is a stutter");
        assert_eq!(event.severity, StutterSeverity::Moderate);
        assert!(event.ratio > 2.6);
        // The stutter frame was excluded from the baseline — consecutive
        // stutters keep flagging rather than silently absorbing.
        let second = detector
            .observe(&frame(50.0, false, 1_100), 1_100)
            .expect("repeated stutter still flags");
        assert_eq!(second.severity, StutterSeverity::Moderate);
        // Normal frames never flag.
        assert!(
            detector
                .observe(&frame(16.4, false, 1_200), 1_200)
                .is_none()
        );
    }

    #[test]
    fn dropped_presents_and_absolute_huge_frames_flag() {
        let mut detector = StutterDetector::new();
        for i in 0..100 {
            detector.observe(&frame(16.6, false, i), i);
        }
        let dropped = detector
            .observe(&frame(16.6, true, 2_000), 2_000)
            .expect("dropped present flags regardless of frame time");
        assert_eq!(dropped.severity, StutterSeverity::DroppedPresent);
        let huge = detector
            .observe(&frame(140.0, false, 2_100), 2_100)
            .expect(">100ms frame is severe");
        assert_eq!(huge.severity, StutterSeverity::Severe);
    }

    #[test]
    fn floor_prevents_micro_flagging_at_high_fps_baselines() {
        let mut detector = StutterDetector::new();
        // 300 Hz-ish baseline: p95 ≈ 3.3ms. Threshold = max(1.8*3.3, 3.3+7)=10.3.
        for i in 0..100 {
            detector.observe(&frame(3.3, false, i), i);
        }
        assert!(detector.observe(&frame(6.0, false, 1_000), 1_000).is_none());
        assert!(detector.observe(&frame(9.0, false, 1_100), 1_100).is_none());
        assert!(
            detector
                .observe(&frame(11.0, false, 1_200), 1_200)
                .is_some()
        );
    }
}
