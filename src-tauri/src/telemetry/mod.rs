use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::PlatformCollector;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Available,
    Unavailable,
    Error,
    WarmingUp,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub id: String,
    pub value: Option<f64>,
    pub unit: String,
    pub source: String,
    pub timestamp_ms: u64,
    pub sample_interval_ms: u64,
    pub status: MetricStatus,
    pub detail: Option<String>,
}

impl Metric {
    pub fn fresh_value(&self, now_ms: u64) -> Option<f64> {
        (self.status == MetricStatus::Available
            && self.timestamp_ms <= now_ms
            && now_ms - self.timestamp_ms <= self.sample_interval_ms.saturating_mul(3).max(3_500))
        .then_some(self.value)
        .flatten()
        .filter(|v| v.is_finite() && *v >= 0.0)
    }

    pub fn available(
        id: &str,
        value: f64,
        unit: &str,
        source: &str,
        timestamp_ms: u64,
        sample_interval_ms: u64,
    ) -> Self {
        Self {
            id: id.into(),
            value: Some(value),
            unit: unit.into(),
            source: source.into(),
            timestamp_ms,
            sample_interval_ms,
            status: MetricStatus::Available,
            detail: None,
        }
    }

    pub fn missing(
        id: &str,
        unit: &str,
        source: &str,
        timestamp_ms: u64,
        sample_interval_ms: u64,
        status: MetricStatus,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            value: None,
            unit: unit.into(),
            source: source.into(),
            timestamp_ms,
            sample_interval_ms,
            status,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    Monitoring,
    Degraded,
    Error,
    Paused,
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    Unsupported,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OsMemoryState {
    Low,
    High,
    Neutral,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub sample_interval_ms: u64,
    pub collection_status: CollectionStatus,
    pub collection_detail: Option<String>,
    pub os_memory_state: OsMemoryState,
    pub total_physical_bytes: Metric,
    pub available_physical_bytes: Metric,
    pub non_available_physical_bytes: Metric,
    pub physical_load_percent: Metric,
    pub commit_used_bytes: Metric,
    pub commit_limit_bytes: Metric,
    pub commit_percent: Metric,
    pub standby_bytes: Metric,
    pub modified_bytes: Metric,
    pub free_zero_bytes: Metric,
    pub page_reads_per_second: Metric,
    pub process_count: Metric,
    pub thread_count: Metric,
}

pub trait Collector {
    fn collect(&mut self, sequence: u64, sample_interval_ms: u64) -> MemorySnapshot;
}

pub fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub fn percent(numerator: f64, denominator: f64) -> Option<f64> {
    if numerator.is_finite() && denominator.is_finite() && denominator > 0.0 {
        Some((numerator / denominator * 100.0).clamp(0.0, 100.0))
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub struct PlatformCollector;

#[cfg(not(target_os = "windows"))]
impl PlatformCollector {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "windows"))]
impl Collector for PlatformCollector {
    fn collect(&mut self, sequence: u64, sample_interval_ms: u64) -> MemorySnapshot {
        let timestamp = timestamp_ms();
        let metric = |id: &str, unit: &str| {
            Metric::missing(
                id,
                unit,
                "Windows API",
                timestamp,
                sample_interval_ms,
                MetricStatus::Unavailable,
                "MemFlux telemetry is supported on Windows only.",
            )
        };
        MemorySnapshot {
            sequence,
            timestamp_ms: timestamp,
            sample_interval_ms,
            collection_status: CollectionStatus::Unsupported,
            collection_detail: Some("MemFlux telemetry is supported on Windows only.".into()),
            os_memory_state: OsMemoryState::Unavailable,
            total_physical_bytes: metric("total_physical_bytes", "bytes"),
            available_physical_bytes: metric("available_physical_bytes", "bytes"),
            non_available_physical_bytes: metric("non_available_physical_bytes", "bytes"),
            physical_load_percent: metric("physical_load_percent", "percent"),
            commit_used_bytes: metric("commit_used_bytes", "bytes"),
            commit_limit_bytes: metric("commit_limit_bytes", "bytes"),
            commit_percent: metric("commit_percent", "percent"),
            standby_bytes: metric("standby_bytes", "bytes"),
            modified_bytes: metric("modified_bytes", "bytes"),
            free_zero_bytes: metric("free_zero_bytes", "bytes"),
            page_reads_per_second: metric("page_reads_per_second", "reads_per_second"),
            process_count: metric("process_count", "count"),
            thread_count: metric("thread_count", "count"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::percent;

    #[test]
    fn percent_is_bounded_and_rejects_invalid_denominator() {
        assert_eq!(percent(25.0, 100.0), Some(25.0));
        assert_eq!(percent(110.0, 100.0), Some(100.0));
        assert_eq!(percent(-1.0, 100.0), Some(0.0));
        assert_eq!(percent(1.0, 0.0), None);
    }
}
