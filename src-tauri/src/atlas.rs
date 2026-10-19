//! Read-only evidence fusion on the existing sampler. Rules describe system
//! conditions and coarse associations, never a calibrated causal probability.
use crate::performance::{FpsCaptureStatus, GpuTelemetry};
use crate::telemetry::{MemorySnapshot, Metric, MetricStatus};
use serde::Serialize;
use std::collections::VecDeque;

const TICK_LIMIT: usize = 90;
const INCIDENT_LIMIT: usize = 64;
const INCIDENT_RADIUS_MS: u64 = 3_000;
const NO_CAUSE: &str =
    "No reliable bottleneck identified with the currently available measurements.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorStatus {
    Available,
    WarmingUp,
    Unavailable,
    Stale,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReading {
    pub sensor_id: String,
    pub category: String,
    pub value: Option<f64>,
    pub unit: String,
    pub status: SensorStatus,
    pub source: String,
    pub target_identity: String,
    pub device_identity: Option<String>,
    pub event_timestamp_ms: Option<u64>,
    pub observation_timestamp_ms: u64,
    pub sampling_interval_ms: u64,
    pub uncertainty_ms: u64,
    pub capability_level: String,
    pub limitation: String,
}

impl SensorReading {
    fn normalize(
        metric: &Metric,
        category: &str,
        device: Option<String>,
        now: u64,
        limitation: &str,
    ) -> Self {
        let value = metric
            .fresh_value(now)
            .filter(|v| metric.unit != "percent" || *v <= 100.0);
        let status = match metric.status {
            MetricStatus::Available if value.is_some() => SensorStatus::Available,
            MetricStatus::Available
                if metric.timestamp_ms > now
                    || metric.value.is_none_or(|v| {
                        !v.is_finite() || v < 0.0 || (metric.unit == "percent" && v > 100.0)
                    }) =>
            {
                SensorStatus::Error
            }
            MetricStatus::Available => SensorStatus::Stale,
            MetricStatus::WarmingUp => SensorStatus::WarmingUp,
            MetricStatus::Unavailable => SensorStatus::Unavailable,
            MetricStatus::Error => SensorStatus::Error,
        };
        Self {
            sensor_id: metric.id.clone(),
            category: category.into(),
            value,
            unit: metric.unit.clone(),
            status,
            source: metric.source.clone(),
            target_identity: "system".into(),
            device_identity: device,
            event_timestamp_ms: Some(metric.timestamp_ms),
            observation_timestamp_ms: now,
            sampling_interval_ms: metric.sample_interval_ms,
            uncertainty_ms: metric.sample_interval_ms,
            capability_level: "existing_native_source".into(),
            limitation: format!("{limitation} {}", metric.detail.as_deref().unwrap_or("")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Moderate,
    Weak,
    Inconclusive,
    InsufficientData,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub explanation: String,
    pub confidence: Confidence,
    pub evidence: Vec<SensorReading>,
    pub missing_evidence: Vec<String>,
    pub contradictory_evidence: Vec<String>,
    pub recommended_test: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasIncident {
    pub id: String,
    pub session_id: String,
    pub target_identity: String,
    pub provider: String,
    pub provider_qpc: Option<String>,
    pub provider_seconds: Option<f64>,
    pub event_timestamp_ms: Option<u64>,
    pub observation_timestamp_ms: u64,
    pub frame_time_ms: f64,
    pub severity: String,
    pub window_start_ms: Option<u64>,
    pub window_end_ms: Option<u64>,
    pub context_samples: usize,
    pub context_state: String,
    pub finding: Finding,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCoverage {
    pub domain: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasReport {
    pub timestamp_ms: u64,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub sensors: Vec<SensorReading>,
    pub domains: Vec<DomainCoverage>,
    pub incidents: Vec<AtlasIncident>,
    pub retained_samples: usize,
    pub evaluation_micros: u64,
}

struct Tick {
    at: u64,
    interval: u64,
    sensors: Vec<SensorReading>,
}

#[derive(Default)]
pub struct AtlasEngine {
    ticks: VecDeque<Tick>,
    incidents: VecDeque<AtlasIncident>,
    seen: VecDeque<String>,
}

fn value(tick: &Tick, id: &str) -> Option<f64> {
    tick.sensors
        .iter()
        .find(|s| s.sensor_id == id && s.status == SensorStatus::Available)?
        .value
}

fn pressure(tick: &Tick) -> Option<bool> {
    let total = value(tick, "total_physical_bytes")?;
    let available = value(tick, "available_physical_bytes")?;
    if total <= 0.0 || available > total {
        return None;
    }
    Some(
        available / total < 0.10
            && value(tick, "commit_percent")? >= 85.0
            && value(tick, "page_reads_per_second")? >= 20.0,
    )
}

fn no_finding() -> Finding {
    Finding { id: "insufficient-evidence".into(), title: "No clear bottleneck".into(),
        explanation: NO_CAUSE.into(), confidence: Confidence::InsufficientData, evidence: vec![],
        missing_evidence: vec!["Workload-attributed execution, paging and device latency are needed to identify a cause.".into()],
        contradictory_evidence: vec![], recommended_test: "Capture the affected scene with explicitly enabled frame capture and compare equivalent workload windows.".into() }
}

fn investigate(ticks: &[&Tick], incident: bool) -> Finding {
    let mut run: Vec<&Tick> = Vec::new();
    let mut sustained: Vec<&Tick> = Vec::new();
    for tick in ticks {
        if run.last().is_some_and(|last| {
            tick.at.saturating_sub(last.at) > tick.interval.max(last.interval).saturating_mul(3) / 2
        }) {
            run.clear();
        }
        if pressure(tick) == Some(true) {
            run.push(tick);
            if run.len() >= 3 && tick.at.saturating_sub(run[0].at) >= 2_000 {
                // Retain an evidence span that actually demonstrates the
                // duration floor, including at the 500ms sampling setting.
                sustained = vec![run[0], run[run.len() - 2], run[run.len() - 1]];
            }
        } else {
            run.clear();
        }
    }
    if !sustained.is_empty() {
        let recovered = ticks.last().is_some_and(|t| pressure(t) == Some(false));
        let missing_now = ticks.last().is_none_or(|t| pressure(t).is_none());
        let confidence = if recovered || missing_now {
            Confidence::Inconclusive
        } else if incident {
            Confidence::Weak
        } else {
            Confidence::Moderate
        };
        return Finding {
            id: "memory-paging-pressure".into(),
            title: if incident { "Memory/paging pressure near a frame anomaly" } else { "Sustained system memory/paging pressure observed" }.into(),
            explanation: "At least three samples across two seconds show <10% physical headroom, >=85% commit and >=20 hard page reads/s. System-wide counters cannot establish that paging delayed the captured workload.".into(),
            confidence,
            evidence: sustained.iter().flat_map(|t| t.sensors.iter().filter(|s| matches!(s.sensor_id.as_str(), "available_physical_bytes" | "total_physical_bytes" | "commit_percent" | "page_reads_per_second")).cloned()).collect(),
            missing_evidence: vec!["Page reads are system-wide; game hard faults and storage latency/device attribution are unavailable.".into(), "Coarse sampling cannot resolve a single frame's cause. Thresholds are investigation heuristics, not calibrated probabilities.".into()],
            contradictory_evidence: if recovered { vec!["Later samples no longer meet the joint pressure condition; recovery weakens a persistent-pressure explanation.".into()] } else if missing_now { vec!["Latest required evidence is missing; current pressure is unconfirmed.".into()] } else { vec![] },
            recommended_test: "Repeat the same scene while recording frame pacing and page reads, then compare after a user-selected memory-heavy workload subsides. A standby purge is not justified by this finding alone.".into(),
        };
    }
    let mut result = no_finding();
    if let Some(tick) = ticks.last() {
        let healthy_cache = value(tick, "total_physical_bytes")
            .zip(value(tick, "available_physical_bytes"))
            .zip(value(tick, "standby_bytes"))
            .is_some_and(|((total, available), standby)| {
                total > 0.0
                    && available <= total
                    && available / total >= 0.2
                    && standby / total >= 0.2
                    && value(tick, "commit_percent").is_some_and(|v| v < 80.0)
                    && value(tick, "page_reads_per_second").is_some_and(|v| v < 1.0)
            });
        if healthy_cache {
            result.id = "healthy-cache".into();
            result.title = "Reclaimable cache with no measured memory-pressure signal".into();
            result.explanation = "This sample has substantial standby cache, physical headroom, low page reads and commit below 80%. A large standby list alone does not justify cleaning; other workload constraints remain unmeasured.".into();
            result.confidence = Confidence::Moderate;
            result.evidence = tick
                .sensors
                .iter()
                .filter(|s| s.category == "memory")
                .cloned()
                .collect();
            result.recommended_test = "No memory intervention indicated by this sample. If a workload is affected, collect its frame timing and attributed resource evidence.".into();
        }
    }
    result
}

impl AtlasEngine {
    pub fn observe(
        &mut self,
        memory: &MemorySnapshot,
        cpu: &Metric,
        gpu: &GpuTelemetry,
        frames: &FpsCaptureStatus,
        now: u64,
    ) -> AtlasReport {
        let started = std::time::Instant::now();
        let sensors: Vec<_> = [
            &memory.total_physical_bytes, &memory.available_physical_bytes,
            &memory.commit_used_bytes, &memory.commit_limit_bytes, &memory.commit_percent,
            &memory.standby_bytes, &memory.free_zero_bytes, &memory.modified_bytes, &memory.page_reads_per_second,
        ].into_iter().map(|m| SensorReading::normalize(m, "memory", None, now, "System-wide memory context; no process/device attribution.")).chain([
            SensorReading::normalize(cpu, "cpu", None, now, "Aggregate CPU cannot identify a saturated thread or scheduling delay."),
            SensorReading::normalize(&gpu.utilization, "gpu", gpu.selected_adapter.as_ref().map(|a| a.luid_key.clone()), now, "Selected-adapter busiest engine; high utilization alone does not establish a bottleneck."),
        ]).collect();
        if self.ticks.back().is_some_and(|t| {
            now <= t.at || now - t.at > memory.sample_interval_ms.max(t.interval).saturating_mul(3)
        }) {
            self.ticks.clear(); // interruption/clock reset cannot count as sustained evidence
        }
        self.ticks.push_back(Tick {
            at: now,
            interval: memory.sample_interval_ms,
            sensors: sensors.clone(),
        });
        if self.ticks.len() > TICK_LIMIT {
            self.ticks.pop_front();
        }
        let recent: Vec<_> = self.ticks.iter().filter(|t| now - t.at <= 15_000).collect();
        let finding = investigate(&recent, false);
        if let (Some(session), Some(pid), Some(creation)) = (
            &frames.capture_session_id,
            frames.target_pid,
            &frames.target_creation_stamp,
        ) {
            for event in &frames.recent_stutters {
                let id = format!("{session}:{}", event.id);
                if self.seen.contains(&id) {
                    continue;
                }
                self.seen.push_back(id.clone());
                if self.seen.len() > 128 {
                    self.seen.pop_front();
                }
                self.incidents.push_back(AtlasIncident {
                    id,
                    session_id: session.clone(),
                    target_identity: format!("pid:{pid}:created:{creation}"),
                    provider: frames.provider_name.clone(),
                    provider_qpc: event.provider_qpc.clone(),
                    provider_seconds: event.provider_seconds,
                    event_timestamp_ms: event.at_ms,
                    observation_timestamp_ms: event.observed_ms,
                    frame_time_ms: event.frame_time_ms,
                    severity: format!("{:?}", event.severity),
                    window_start_ms: event.at_ms.map(|t| t.saturating_sub(INCIDENT_RADIUS_MS)),
                    window_end_ms: event.at_ms.map(|t| t.saturating_add(INCIDENT_RADIUS_MS)),
                    context_samples: 0,
                    context_state: "awaiting_context".into(),
                    finding: no_finding(),
                });
                if self.incidents.len() > INCIDENT_LIMIT {
                    self.incidents.pop_front();
                }
            }
        }
        for incident in &mut self.incidents {
            if incident.context_state == "complete" || incident.context_state == "unavailable" {
                continue;
            }
            let Some(at) = incident.event_timestamp_ms else {
                incident.context_state = "unavailable".into();
                incident.finding.missing_evidence.push(
                    "Provider event time unavailable; CSV arrival is not used for correlation."
                        .into(),
                );
                continue;
            };
            let start = at.saturating_sub(INCIDENT_RADIUS_MS);
            let end = at.saturating_add(INCIDENT_RADIUS_MS);
            // Include the complete coarse sensor window intersecting the incident
            // window, not merely the single nearest sample.
            let context: Vec<_> = self
                .ticks
                .iter()
                .filter(|t| t.at >= start && t.at.saturating_sub(t.interval) <= end)
                .collect();
            incident.context_samples = context.len();
            incident.finding = investigate(&context, true);
            let before = context.iter().any(|t| t.at < at);
            let after = context.iter().any(|t| t.at >= end);
            incident.context_state = if now >= end {
                "complete"
            } else {
                "collecting_after"
            }
            .into();
            if !before || !after {
                incident.finding.missing_evidence.push("Before/after context is incomplete; pause, capture delay or buffer retention may limit this investigation.".into());
                if incident.finding.confidence == Confidence::Moderate {
                    incident.finding.confidence = Confidence::Weak;
                }
            }
        }
        let coverage = |domain: &str, status: &str, detail: &str| DomainCoverage {
            domain: domain.into(),
            status: status.into(),
            detail: detail.into(),
        };
        AtlasReport {
            timestamp_ms: now,
            summary: NO_CAUSE.into(),
            findings: vec![finding],
            sensors,
            domains: vec![
                coverage(
                    "CPU",
                    "context_only",
                    "GetSystemTimes aggregate; per-core and attributed scheduling evidence unavailable.",
                ),
                coverage(
                    "GPU / VRAM",
                    "partial",
                    "WDDM selected-adapter context; game VRAM usage/residency unavailable.",
                ),
                coverage(
                    "Memory / paging",
                    "partial",
                    "System memory/commit/page reads; conservative pressure investigation; process paging unavailable.",
                ),
                coverage(
                    "Storage",
                    "unavailable",
                    "No latency, device mapping or physical I/O source integrated.",
                ),
                coverage(
                    "Thermal / power",
                    "context_only",
                    "Existing power-scheme controls; no thermal/throttle source integrated.",
                ),
                coverage(
                    "Streaming",
                    "unavailable",
                    "OBS render/encoder/network statistics are not connected.",
                ),
            ],
            incidents: self.incidents.iter().cloned().collect(),
            retained_samples: self.ticks.len(),
            evaluation_micros: started.elapsed().as_micros() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stutter::{StutterEvent, StutterSeverity};
    use crate::telemetry::{CollectionStatus, OsMemoryState};

    fn metric(id: &str, value: f64, at: u64) -> Metric {
        Metric::available(
            id,
            value,
            if id.contains("percent") {
                "percent"
            } else {
                "bytes"
            },
            "synthetic replay (test only)",
            at,
            1_000,
        )
    }
    fn snapshot(at: u64, pressured: bool) -> MemorySnapshot {
        let m = |id, v| metric(id, v, at);
        MemorySnapshot {
            sequence: at,
            timestamp_ms: at,
            sample_interval_ms: 1_000,
            collection_status: CollectionStatus::Monitoring,
            collection_detail: None,
            os_memory_state: OsMemoryState::Neutral,
            total_physical_bytes: m("total_physical_bytes", 100.0),
            available_physical_bytes: m(
                "available_physical_bytes",
                if pressured { 4.0 } else { 50.0 },
            ),
            non_available_physical_bytes: m("non_available_physical_bytes", 50.0),
            physical_load_percent: m("physical_load_percent", 50.0),
            commit_used_bytes: m("commit_used_bytes", 50.0),
            commit_limit_bytes: m("commit_limit_bytes", 100.0),
            commit_percent: m("commit_percent", if pressured { 95.0 } else { 50.0 }),
            standby_bytes: m("standby_bytes", 30.0),
            modified_bytes: m("modified_bytes", 1.0),
            free_zero_bytes: m("free_zero_bytes", 20.0),
            page_reads_per_second: m("page_reads_per_second", if pressured { 100.0 } else { 0.0 }),
            process_count: m("process_count", 100.0),
            thread_count: m("thread_count", 1000.0),
        }
    }
    fn gpu(at: u64) -> GpuTelemetry {
        GpuTelemetry {
            utilization: metric("gpu_utilization_percent", 100.0, at),
            selected_adapter: None,
            available_adapters: vec![],
            engine_breakdown: None,
            per_process_gpu_percent: Default::default(),
        }
    }
    fn frames() -> FpsCaptureStatus {
        FpsCaptureStatus {
            active: true,
            provider_available: true,
            provider_name: "synthetic replay (test only)".into(),
            target_pid: Some(42),
            target_executable: Some("fixture.exe".into()),
            target_creation_stamp: Some("123456789123456789".into()),
            capture_session_id: Some("fixture-session".into()),
            fps: metric("fps", 60.0, 1000),
            frame_time_ms: metric("frame_time_ms", 16.6, 1000),
            frame_time_p99_ms: Some(16.6),
            recent_stutters: vec![],
            stutter_summary: None,
            status_detail: "test fixture".into(),
        }
    }
    fn event(at: Option<u64>) -> StutterEvent {
        StutterEvent {
            id: "fixture-event".into(),
            at_ms: at,
            observed_ms: 5_000,
            provider_qpc: Some("100000".into()),
            provider_seconds: Some(2.0),
            frame_time_ms: 100.0,
            baseline_p95_ms: 16.6,
            ratio: 6.0,
            dropped: false,
            severity: StutterSeverity::Severe,
            note: "synthetic".into(),
        }
    }
    fn observe(
        engine: &mut AtlasEngine,
        at: u64,
        pressured: bool,
        f: &FpsCaptureStatus,
    ) -> AtlasReport {
        engine.observe(
            &snapshot(at, pressured),
            &metric("cpu_percent", 100.0, at),
            &gpu(at),
            f,
            at,
        )
    }

    #[test]
    fn utilization_and_large_cache_never_establish_a_bottleneck() {
        let mut e = AtlasEngine::default();
        let report = observe(&mut e, 1000, false, &frames());
        assert_eq!(report.findings[0].id, "healthy-cache");
        assert_eq!(report.summary, NO_CAUSE);
        assert!(
            report.findings[0]
                .recommended_test
                .contains("No memory intervention")
        );
        assert!(
            report
                .domains
                .iter()
                .any(|d| d.domain == "Storage" && d.status == "unavailable")
        );
    }
    #[test]
    fn sustained_pressure_is_observation_and_recovery_is_contradictory() {
        let mut e = AtlasEngine::default();
        let f = frames();
        for at in [1000, 2000] {
            assert_ne!(
                observe(&mut e, at, true, &f).findings[0].id,
                "memory-paging-pressure"
            );
        }
        let report = observe(&mut e, 3000, true, &f);
        assert_eq!(report.findings[0].id, "memory-paging-pressure");
        assert_eq!(report.findings[0].confidence, Confidence::Moderate);
        assert_eq!(report.findings[0].evidence.len(), 12);
        let recovered = observe(&mut e, 4000, false, &f);
        assert_eq!(recovered.findings[0].confidence, Confidence::Inconclusive);
        assert!(!recovered.findings[0].contradictory_evidence.is_empty());
    }
    #[test]
    fn evidence_preserves_duration_floor_at_half_second_cadence() {
        let mut e = AtlasEngine::default();
        let f = frames();
        let mut result = None;
        for at in [1000, 1500, 2000, 2500, 3000] {
            let mut snapshot = snapshot(at, true);
            snapshot.sample_interval_ms = 500;
            result = Some(e.observe(
                &snapshot,
                &metric("cpu_percent", 50.0, at),
                &gpu(at),
                &f,
                at,
            ));
        }
        let report = result.unwrap();
        let evidence = &report.findings[0].evidence;
        assert_eq!(report.findings[0].id, "memory-paging-pressure");
        let first = evidence
            .iter()
            .filter_map(|r| r.event_timestamp_ms)
            .min()
            .unwrap();
        let last = evidence
            .iter()
            .filter_map(|r| r.event_timestamp_ms)
            .max()
            .unwrap();
        assert!(last - first >= 2_000);
    }
    #[test]
    fn normalization_rejects_invalid_missing_stale_and_future() {
        for (value, stamp, status) in [
            (f64::NAN, 1000, SensorStatus::Error),
            (-1.0, 1000, SensorStatus::Error),
            (101.0, 1000, SensorStatus::Error),
            (20.0, 100, SensorStatus::Stale),
            (20.0, 6000, SensorStatus::Error),
        ] {
            let r = SensorReading::normalize(
                &metric("cpu_percent", value, stamp),
                "cpu",
                None,
                5000,
                "fixture",
            );
            assert_eq!(r.status, status);
            assert_eq!(r.value, None);
        }
        let mut absent = metric("cpu_percent", 10.0, 5000);
        absent.status = MetricStatus::Unavailable;
        let r = SensorReading::normalize(&absent, "cpu", None, 5000, "fixture");
        assert_eq!(r.value, None);
        assert_eq!(r.status, SensorStatus::Unavailable);
    }
    #[test]
    fn interruption_clock_reset_and_impossible_memory_fail_closed() {
        let mut e = AtlasEngine::default();
        let f = frames();
        for at in [1000, 2000, 3000] {
            observe(&mut e, at, true, &f);
        }
        assert_eq!(observe(&mut e, 20000, true, &f).retained_samples, 1);
        assert_eq!(observe(&mut e, 1000, true, &f).retained_samples, 1);
        let mut s = snapshot(2000, true);
        s.available_physical_bytes.value = Some(200.0);
        let report = e.observe(&s, &metric("cpu_percent", 10.0, 2000), &gpu(2000), &f, 2000);
        assert_ne!(report.findings[0].id, "memory-paging-pressure");
    }
    #[test]
    fn incidents_use_event_windows_not_arrival_and_preserve_creation_identity() {
        let mut e = AtlasEngine::default();
        let mut f = frames();
        for at in [1000, 2000, 3000, 4000] {
            observe(&mut e, at, true, &f);
        }
        f.recent_stutters.push(event(Some(3000)));
        let report = observe(&mut e, 5000, true, &f);
        let incident = &report.incidents[0];
        assert_eq!(incident.event_timestamp_ms, Some(3000));
        assert_eq!(incident.observation_timestamp_ms, 5000);
        assert!(incident.target_identity.ends_with("123456789123456789"));
        assert_eq!(incident.finding.confidence, Confidence::Weak);
        assert!(incident.context_samples > 1);
        let complete = observe(&mut e, 6000, true, &f);
        assert_eq!(complete.incidents.len(), 1); // dedup repeated provider snapshots
        assert_eq!(complete.incidents[0].context_state, "complete");
        f.capture_session_id = Some("new-session-pid-reused".into());
        f.target_creation_stamp = Some("999".into());
        assert_eq!(observe(&mut e, 7000, true, &f).incidents.len(), 2);
    }
    #[test]
    fn missing_timestamp_or_identity_cannot_correlate() {
        let mut e = AtlasEngine::default();
        let mut f = frames();
        f.recent_stutters.push(event(None));
        let report = observe(&mut e, 5000, true, &f);
        assert_eq!(report.incidents[0].context_state, "unavailable");
        assert_eq!(
            report.incidents[0].finding.confidence,
            Confidence::InsufficientData
        );
        f.target_creation_stamp = None;
        f.capture_session_id = Some("missing-identity".into());
        assert_eq!(observe(&mut e, 6000, true, &f).incidents.len(), 1);
    }
    #[test]
    fn bounded_replay_and_profile() {
        let mut e = AtlasEngine::default();
        let mut f = frames();
        let mut times = Vec::new();
        let mut report = observe(&mut e, 1000, true, &f);
        for i in 2..=1000_u64 {
            f.recent_stutters = vec![event(Some(i * 1000))];
            f.recent_stutters[0].id = format!("event-{i}");
            report = observe(&mut e, i * 1000, i % 10 < 7, &f);
            times.push(report.evaluation_micros);
        }
        assert_eq!(report.retained_samples, 90);
        assert_eq!(report.incidents.len(), 64);
        assert_eq!(e.seen.len(), 128);
        times.sort_unstable();
        let bytes = serde_json::to_vec(&report).unwrap().len();
        println!(
            "ATLAS_SYNTHETIC_PROFILE ticks=1000 p50_us={} p95_us={} max_us={} serialized_report_bytes={bytes}",
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times.last().unwrap()
        );
        assert!(bytes < 1_048_576); // bounded transport budget; not process private bytes
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_shared_sensor_pipeline_is_read_only_and_attributed() {
        use crate::performance::{CpuSensor, GpuSensor, PresentMonController};
        use crate::telemetry::{Collector, PlatformCollector, timestamp_ms};
        let mut memory = PlatformCollector::new();
        let mut cpu = CpuSensor::default();
        let mut gpu = GpuSensor::default();
        let mut provider = PresentMonController::new();
        let mut atlas = AtlasEngine::default();
        for i in 0..3 {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            let snapshot = memory.collect(i + 1, 1000);
            let cpu = cpu.collect(1000, true);
            let gpu = gpu.collect(1000, true, "auto");
            let capture = provider.status(1000, false); // never starts ETW or UAC
            let report = atlas.observe(&snapshot, &cpu, &gpu, &capture, timestamp_ms());
            assert_eq!(report.retained_samples, i as usize + 1);
            let total = report
                .sensors
                .iter()
                .find(|s| s.sensor_id == "total_physical_bytes")
                .unwrap();
            assert_eq!(total.source, "GlobalMemoryStatusEx");
            assert_eq!(total.status, SensorStatus::Available);
            assert_eq!(total.target_identity, "system");
            assert!(report.incidents.is_empty());
            if i == 2 {
                for sensor in &report.sensors {
                    println!(
                        "ATLAS_NATIVE_SENSOR id={} value={:?} status={:?} source={} device={:?} at={} interval_ms={}",
                        sensor.sensor_id,
                        sensor.value,
                        sensor.status,
                        sensor.source,
                        sensor.device_identity,
                        sensor.observation_timestamp_ms,
                        sensor.sampling_interval_ms
                    );
                }
                println!(
                    "ATLAS_NATIVE_CONTEXT adapter={:?} presentmon_installed={} summary={}",
                    gpu.selected_adapter.map(|a| a.name),
                    capture.provider_available,
                    report.summary
                );
            }
        }
    }
}
