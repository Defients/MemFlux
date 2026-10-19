//! Performance Proof — opt-in measurement workflow. The user marks a baseline
//! period, applies an optimization, marks the change boundary, and compares
//! the post-change samples. Results report observed deltas with confidence
//! wording; the tool never claims causation — only that a measurable change
//! in sampled telemetry followed the action.

use serde::Serialize;
use std::collections::VecDeque;

/// Samples per phase cap (~10 minutes at 1 s cadence).
const PROOF_PHASE_SAMPLE_LIMIT: usize = 600;
/// Minimum samples on each side for a comparison to be reported.
const MIN_PROOF_SAMPLES: usize = 5;
/// Mean separation beyond one pooled standard deviation counts as measurable.
const MEASURABLE_SIGMA: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofPhase {
    /// Collecting pre-change samples.
    Baseline,
    /// Change boundary marked; collecting post-change samples.
    PostChange,
}

/// One backend telemetry point captured per sampler tick while a session runs.
#[derive(Clone, Debug, Default)]
pub struct ProofSample {
    pub at_ms: u64,
    pub cpu_percent: Option<f64>,
    pub gpu_percent: Option<f64>,
    pub ram_used_bytes: Option<f64>,
    pub fps: Option<f64>,
    pub frame_ms: Option<f64>,
    pub frame_p99_ms: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricComparison {
    pub metric: String,
    pub unit: String,
    pub baseline_mean: f64,
    pub baseline_stddev: f64,
    pub post_mean: f64,
    pub post_stddev: f64,
    pub delta: f64,
    /// Negative delta means the metric went down (not always worse).
    pub direction: String,
    /// "measurable" | "inconclusive" — never "proven".
    pub confidence: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofReport {
    pub active: bool,
    pub phase: Option<ProofPhase>,
    pub baseline_samples: usize,
    pub post_samples: usize,
    pub comparisons: Vec<MetricComparison>,
    pub verdict: String,
    pub caveat: String,
}

pub struct ProofSession {
    phase: ProofPhase,
    pub started_ms: u64,
    pub change_marked_ms: Option<u64>,
    baseline: VecDeque<ProofSample>,
    post: VecDeque<ProofSample>,
    action_label: Option<String>,
}

impl Default for ProofSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofSession {
    pub fn new() -> Self {
        Self {
            phase: ProofPhase::Baseline,
            started_ms: 0,
            change_marked_ms: None,
            baseline: VecDeque::new(),
            post: VecDeque::new(),
            action_label: None,
        }
    }

    pub fn begin(&mut self, now_ms: u64) {
        *self = Self::new();
        self.started_ms = now_ms;
    }

    pub fn mark_change(&mut self, now_ms: u64, label: Option<String>) {
        if self.phase == ProofPhase::Baseline {
            self.phase = ProofPhase::PostChange;
            self.change_marked_ms = Some(now_ms);
            self.action_label = label;
        }
    }

    pub fn observe(&mut self, sample: ProofSample) {
        let target = match self.phase {
            ProofPhase::Baseline => &mut self.baseline,
            ProofPhase::PostChange => &mut self.post,
        };
        if target.len() >= PROOF_PHASE_SAMPLE_LIMIT {
            target.pop_front();
        }
        target.push_back(sample);
    }

    pub fn status(&self) -> ProofReport {
        if self.phase == ProofPhase::Baseline {
            return ProofReport {
                active: true,
                phase: Some(self.phase),
                baseline_samples: self.baseline.len(),
                post_samples: 0,
                comparisons: Vec::new(),
                verdict: "Baseline sampling in progress.".into(),
                caveat: "Apply the change, then mark the boundary to start post-change sampling."
                    .into(),
            };
        }
        self.report()
    }

    pub fn report(&self) -> ProofReport {
        let baseline_ms: Vec<f64> = self.baseline.iter().map(|s| s.at_ms as f64).collect();
        let _ = baseline_ms;
        let mut comparisons = Vec::new();
        comparisons.extend(compare_series(
            "cpu_percent",
            "%",
            extract(&self.baseline, |s| s.cpu_percent),
            extract(&self.post, |s| s.cpu_percent),
            "lower is better",
        ));
        comparisons.extend(compare_series(
            "gpu_percent",
            "%",
            extract(&self.baseline, |s| s.gpu_percent),
            extract(&self.post, |s| s.gpu_percent),
            "lower is better",
        ));
        comparisons.extend(compare_series(
            "ram_used_bytes",
            "bytes",
            extract(&self.baseline, |s| s.ram_used_bytes),
            extract(&self.post, |s| s.ram_used_bytes),
            "lower is better",
        ));
        comparisons.extend(compare_series(
            "fps",
            "fps",
            extract(&self.baseline, |s| s.fps),
            extract(&self.post, |s| s.fps),
            "higher is better",
        ));
        comparisons.extend(compare_series(
            "frame_ms",
            "ms",
            extract(&self.baseline, |s| s.frame_ms),
            extract(&self.post, |s| s.frame_ms),
            "lower is better",
        ));
        comparisons.extend(compare_series(
            "frame_p99_ms",
            "ms",
            extract(&self.baseline, |s| s.frame_p99_ms),
            extract(&self.post, |s| s.frame_p99_ms),
            "lower is better",
        ));

        let measurable = comparisons
            .iter()
            .filter(|c| c.confidence == "measurable")
            .count();
        let verdict = if comparisons.is_empty() {
            "Insufficient samples on both sides of the change boundary to compare.".to_string()
        } else if measurable == 0 {
            "Observed differences are within sampling noise; no measurable effect detected."
                .to_string()
        } else {
            format!(
                "{} metric(s) moved beyond sampling noise after the change. This is a measured correlation — treat it as evidence, not proof.",
                measurable
            )
        };

        ProofReport {
            active: true,
            phase: Some(ProofPhase::PostChange),
            baseline_samples: self.baseline.len(),
            post_samples: self.post.len(),
            comparisons,
            verdict,
            caveat: "Same-workload comparisons only: ensure the scene/load is comparable between baseline and post-change. Background noise, thermal state, and other processes confound the result.".into(),
        }
    }
}

fn extract(series: &VecDeque<ProofSample>, pick: impl Fn(&ProofSample) -> Option<f64>) -> Vec<f64> {
    series.iter().filter_map(pick).collect()
}

fn mean_stddev(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() < MIN_PROOF_SAMPLES {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance =
        values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / values.len() as f64;
    Some((mean, variance.sqrt()))
}

/// Compare two series; returns None when either side lacks enough samples.
/// "Measurable" requires the means to differ by more than one pooled standard
/// deviation and direction wording is explicit about which direction is
/// "better" for each metric.
fn compare_series(
    metric: &str,
    unit: &str,
    baseline: Vec<f64>,
    post: Vec<f64>,
    better_direction: &str,
) -> Option<MetricComparison> {
    let (base_mean, base_std) = mean_stddev(&baseline)?;
    let (post_mean, post_std) = mean_stddev(&post)?;
    let delta = post_mean - base_mean;
    let pooled = ((base_std + post_std) / 2.0).max(f64::EPSILON);
    let measurable = delta.abs() > pooled * MEASURABLE_SIGMA;
    let improved = match better_direction {
        "lower is better" => delta < 0.0,
        _ => delta > 0.0,
    };
    let direction = if !measurable {
        "within_noise".to_string()
    } else if improved {
        "improved".to_string()
    } else {
        "regressed".to_string()
    };
    Some(MetricComparison {
        metric: metric.into(),
        unit: unit.into(),
        baseline_mean: base_mean,
        baseline_stddev: base_std,
        post_mean,
        post_stddev: post_std,
        delta,
        direction,
        confidence: if measurable {
            "measurable".into()
        } else {
            "inconclusive".into()
        },
        detail: format!(
            "n_base={}, n_post={}; mean {base_mean:.3} -> {post_mean:.3} ({better_direction})",
            baseline.len(),
            post.len()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(at: u64, cpu: Option<f64>, fps: Option<f64>) -> ProofSample {
        ProofSample {
            at_ms: at,
            cpu_percent: cpu,
            gpu_percent: None,
            ram_used_bytes: None,
            fps,
            frame_ms: None,
            frame_p99_ms: None,
        }
    }

    #[test]
    fn comparisons_require_minimum_samples() {
        let mut session = ProofSession::new();
        session.begin(0);
        for i in 0..3 {
            session.observe(sample(i, Some(50.0), Some(60.0)));
        }
        session.mark_change(5, Some("purge".into()));
        for i in 0..3 {
            session.observe(sample(100 + i, Some(10.0), Some(120.0)));
        }
        let report = session.report();
        assert!(report.comparisons.is_empty());
        assert!(report.verdict.contains("Insufficient samples"));
    }

    #[test]
    fn measurable_improvement_is_labeled_measurable_not_proven() {
        let mut session = ProofSession::new();
        session.begin(0);
        for i in 0..30 {
            session.observe(sample(i, Some(80.0 + (i % 3) as f64), Some(55.0)));
        }
        session.mark_change(100, Some("powershift".into()));
        for i in 0..30 {
            session.observe(sample(1_000 + i, Some(40.0 + (i % 3) as f64), Some(140.0)));
        }
        let report = session.report();
        let cpu = report
            .comparisons
            .iter()
            .find(|c| c.metric == "cpu_percent")
            .expect("cpu comparison");
        assert_eq!(cpu.confidence, "measurable");
        assert_eq!(cpu.direction, "improved");
        let fps = report
            .comparisons
            .iter()
            .find(|c| c.metric == "fps")
            .expect("fps comparison");
        assert_eq!(fps.direction, "improved");
        assert!(report.verdict.contains("correlation"));
    }

    #[test]
    fn noise_level_change_reports_inconclusive() {
        let mut session = ProofSession::new();
        session.begin(0);
        for i in 0..30 {
            session.observe(sample(i, Some(50.0 + (i % 10) as f64), None));
        }
        session.mark_change(100, None);
        for i in 0..30 {
            session.observe(sample(1_000 + i, Some(50.5 + (i % 10) as f64), None));
        }
        let report = session.report();
        let cpu = report
            .comparisons
            .iter()
            .find(|c| c.metric == "cpu_percent")
            .expect("cpu comparison");
        assert_eq!(cpu.confidence, "inconclusive");
        assert!(report.verdict.contains("no measurable effect"));
    }
}
