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
const WARMUP_MS: u64 = 5_000;
const MIN_PHASE_DURATION_MS: u64 = 30_000;

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
    pub hypothesis_id: Option<String>,
}

pub struct ProofSession {
    phase: ProofPhase,
    pub started_ms: u64,
    pub change_marked_ms: Option<u64>,
    baseline: VecDeque<ProofSample>,
    post: VecDeque<ProofSample>,
    action_label: Option<String>,
    pub hypothesis_id: Option<String>,
    finished: bool,
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
            hypothesis_id: None,
            finished: false,
        }
    }

    pub fn begin(&mut self, now_ms: u64) {
        *self = Self::new();
        self.started_ms = now_ms;
    }

    pub fn mark_change(&mut self, now_ms: u64, label: Option<String>) {
        if !self.finished && self.phase == ProofPhase::Baseline {
            self.phase = ProofPhase::PostChange;
            self.change_marked_ms = Some(now_ms);
            self.action_label = label;
        }
    }

    pub fn observe(&mut self, sample: ProofSample) {
        if self.finished {
            return;
        }
        let target = match self.phase {
            ProofPhase::Baseline => &mut self.baseline,
            ProofPhase::PostChange => &mut self.post,
        };
        if target.back().is_some_and(|s| sample.at_ms <= s.at_ms) {
            return;
        }
        if target.len() >= PROOF_PHASE_SAMPLE_LIMIT {
            target.pop_front();
        }
        target.push_back(sample);
    }

    pub fn status(&self) -> ProofReport {
        if self.finished {
            let mut report = self.report();
            report.active = false;
            return report;
        }
        if self.phase == ProofPhase::Baseline {
            return ProofReport {
                active: true,
                phase: Some(self.phase),
                baseline_samples: self.baseline.len(),
                post_samples: 0,
                comparisons: Vec::new(),
                hypothesis_id: self.hypothesis_id.clone(),
                verdict: "Baseline sampling in progress.".into(),
                caveat: "Apply the change, then mark the boundary to start post-change sampling."
                    .into(),
            };
        }
        self.report()
    }

    pub fn finish(&mut self) -> ProofReport {
        self.finished = true;
        self.status()
    }

    pub fn report(&self) -> ProofReport {
        // Exclude each phase's first 5s. These are sampled aggregate windows,
        // not independent frames; no significance/effective-count claim is made.
        let baseline = usable_phase(&self.baseline, self.started_ms);
        let post = usable_phase(&self.post, self.change_marked_ms.unwrap_or(u64::MAX));
        let mut comparisons = Vec::new();
        comparisons.extend(compare_series(
            "cpu_percent",
            "%",
            extract(&baseline, |s| s.cpu_percent),
            extract(&post, |s| s.cpu_percent),
            "context only",
        ));
        comparisons.extend(compare_series(
            "gpu_percent",
            "%",
            extract(&baseline, |s| s.gpu_percent),
            extract(&post, |s| s.gpu_percent),
            "context only",
        ));
        comparisons.extend(compare_series(
            "ram_used_bytes",
            "bytes",
            extract(&baseline, |s| s.ram_used_bytes),
            extract(&post, |s| s.ram_used_bytes),
            "context only",
        ));
        comparisons.extend(compare_series(
            "fps",
            "fps",
            extract(&baseline, |s| s.fps),
            extract(&post, |s| s.fps),
            "higher is better",
        ));
        comparisons.extend(compare_series(
            "frame_ms",
            "ms",
            extract(&baseline, |s| s.frame_ms),
            extract(&post, |s| s.frame_ms),
            "lower is better",
        ));
        comparisons.extend(compare_series(
            "frame_p99_ms",
            "ms",
            extract(&baseline, |s| s.frame_p99_ms),
            extract(&post, |s| s.frame_p99_ms),
            "lower is better",
        ));

        let verdict = if comparisons.is_empty() {
            "Insufficient data: each phase needs 30 seconds after a 5-second warm-up and five valid samples.".to_string()
        } else {
            "Descriptive changes observed. Workload comparability and repeatability are unverified; no reliable performance benefit established.".to_string()
        };

        ProofReport {
            active: true,
            phase: Some(ProofPhase::PostChange),
            baseline_samples: self.baseline.len(),
            post_samples: self.post.len(),
            comparisons,
            verdict,
            hypothesis_id: self.hypothesis_id.clone(),
            caveat: "Same-workload comparisons only: ensure the scene/load is comparable between baseline and post-change. Background noise, thermal state, and other processes confound the result.".into(),
        }
    }
}

fn extract(series: &[&ProofSample], pick: impl Fn(&ProofSample) -> Option<f64>) -> Vec<f64> {
    series
        .iter()
        .filter_map(|s| pick(s))
        .filter(|v| v.is_finite() && *v >= 0.0)
        .collect()
}

fn mean_stddev(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() < MIN_PROOF_SAMPLES || values.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance =
        values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / values.len() as f64;
    Some((mean, variance.sqrt()))
}

fn usable_phase(series: &VecDeque<ProofSample>, start: u64) -> Vec<&ProofSample> {
    let values: Vec<_> = series
        .iter()
        .filter(|s| s.at_ms >= start.saturating_add(WARMUP_MS))
        .collect();
    if values
        .first()
        .zip(values.last())
        .is_none_or(|(a, b)| b.at_ms - a.at_ms < MIN_PHASE_DURATION_MS)
    {
        Vec::new()
    } else {
        values
    }
}

/// Descriptive deltas only: aggregate windows are autocorrelated, workload
/// equivalence is not established, and utilization has no favorable direction.
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
    let direction = if delta > 0.0 {
        "increased"
    } else if delta < 0.0 {
        "decreased"
    } else {
        "unchanged"
    }
    .to_string();
    Some(MetricComparison {
        metric: metric.into(),
        unit: unit.into(),
        baseline_mean: base_mean,
        baseline_stddev: base_std,
        post_mean,
        post_stddev: post_std,
        delta,
        direction,
        confidence: "descriptive_only".into(),
        detail: format!(
            "n_base={}, n_post={}; mean {base_mean:.3} -> {post_mean:.3} ({better_direction}; direction is descriptive, no benefit inference)",
            baseline.len(),
            post.len()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utilization_decrease_is_never_improvement() {
        for metric in ["cpu", "gpu", "ram"] {
            let c = compare_series(metric, "%", vec![80.0; 10], vec![20.0; 10], "context only")
                .unwrap();
            assert_eq!(c.direction, "decreased");
            assert_eq!(c.confidence, "descriptive_only");
        }
    }
    #[test]
    fn short_phases_and_invalid_samples_cannot_establish_benefit() {
        let mut s = ProofSession::new();
        s.begin(0);
        for i in 0..100 {
            s.observe(ProofSample {
                at_ms: i,
                cpu_percent: Some(80.0),
                ..Default::default()
            });
        }
        s.mark_change(100, None);
        for i in 101..200 {
            s.observe(ProofSample {
                at_ms: i,
                cpu_percent: Some(20.0),
                ..Default::default()
            });
        }
        assert!(s.report().comparisons.is_empty());
        assert!(
            compare_series(
                "fps",
                "fps",
                vec![f64::NAN],
                vec![60.0; 10],
                "higher is better"
            )
            .is_none()
        );
    }
    #[test]
    fn representative_windows_remain_descriptive_and_exclude_warmup() {
        let mut s = ProofSession::new();
        s.begin(0);
        for i in 0..=40 {
            s.observe(ProofSample {
                at_ms: i * 1_000,
                cpu_percent: Some(if i < 5 { 0.0 } else { 80.0 }),
                ..Default::default()
            });
        }
        s.mark_change(41_000, None);
        for i in 41..=81 {
            s.observe(ProofSample {
                at_ms: i * 1_000,
                cpu_percent: Some(20.0),
                ..Default::default()
            });
        }
        let r = s.report();
        assert_eq!(r.comparisons[0].baseline_mean, 80.0);
        assert_eq!(r.comparisons[0].direction, "decreased");
        assert!(r.verdict.contains("unverified"));
    }
}
