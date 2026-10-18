use super::{
    CollectionStatus, Collector, MemorySnapshot, Metric, MetricStatus, OsMemoryState, percent,
    timestamp_ms,
};
use std::mem::size_of;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateMemoryResourceNotification, HighMemoryResourceNotification,
    LowMemoryResourceNotification, QueryMemoryResourceNotification,
};
use windows::Win32::System::Performance::{
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterValue, PdhOpenQueryW,
};
use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::core::{BOOL, PCWSTR};

const PDH_SUCCESS: u32 = 0;

struct ResourceNotifications {
    low: Option<HANDLE>,
    high: Option<HANDLE>,
    setup_error: Option<String>,
}

impl ResourceNotifications {
    fn new() -> Self {
        let low = unsafe { CreateMemoryResourceNotification(LowMemoryResourceNotification) };
        let high = unsafe { CreateMemoryResourceNotification(HighMemoryResourceNotification) };

        let setup_error = match (&low, &high) {
            (Ok(_), Ok(_)) => None,
            (low_result, high_result) => Some(format!(
                "Memory resource notifications unavailable (low: {}; high: {}).",
                format_result(low_result),
                format_result(high_result)
            )),
        };

        Self {
            low: low.ok(),
            high: high.ok(),
            setup_error,
        }
    }

    fn state(&self) -> (OsMemoryState, Option<String>) {
        let (Some(low), Some(high)) = (self.low, self.high) else {
            return (OsMemoryState::Unavailable, self.setup_error.clone());
        };

        let mut low_state = BOOL(0);
        let mut high_state = BOOL(0);
        let low_result = unsafe { QueryMemoryResourceNotification(low, &mut low_state) };
        let high_result = unsafe { QueryMemoryResourceNotification(high, &mut high_state) };

        if let Err(error) = low_result {
            return (
                OsMemoryState::Unavailable,
                Some(format!("Low-memory notification query failed: {error}")),
            );
        }
        if let Err(error) = high_result {
            return (
                OsMemoryState::Unavailable,
                Some(format!("High-memory notification query failed: {error}")),
            );
        }

        if low_state.as_bool() {
            (OsMemoryState::Low, None)
        } else if high_state.as_bool() {
            (OsMemoryState::High, None)
        } else {
            (OsMemoryState::Neutral, None)
        }
    }
}

impl Drop for ResourceNotifications {
    fn drop(&mut self) {
        for handle in [self.low.take(), self.high.take()].into_iter().flatten() {
            if let Err(error) = unsafe { CloseHandle(handle) } {
                log::warn!("Failed to close memory resource notification handle: {error}");
            }
        }
    }
}

fn format_result<T>(result: &windows::core::Result<T>) -> String {
    match result {
        Ok(_) => "ok".into(),
        Err(error) => error.to_string(),
    }
}

#[derive(Clone, Copy)]
struct CounterHandle {
    handle: Option<PDH_HCOUNTER>,
    registration_status: u32,
}

impl CounterHandle {
    fn unavailable_status(&self) -> String {
        format!(
            "PDH counter registration status 0x{:08X}",
            self.registration_status
        )
    }
}

struct PdhCounters {
    query: Option<PDH_HQUERY>,
    query_status: u32,
    standby_reserve: CounterHandle,
    standby_normal: CounterHandle,
    standby_core: CounterHandle,
    modified: CounterHandle,
    free_zero: CounterHandle,
    page_reads: CounterHandle,
    collections: u64,
}

impl PdhCounters {
    fn new() -> Self {
        let mut query = PDH_HQUERY::default();
        let query_status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
        if query_status != PDH_SUCCESS {
            return Self {
                query: None,
                query_status,
                standby_reserve: failed_counter(query_status),
                standby_normal: failed_counter(query_status),
                standby_core: failed_counter(query_status),
                modified: failed_counter(query_status),
                free_zero: failed_counter(query_status),
                page_reads: failed_counter(query_status),
                collections: 0,
            };
        }

        let add = |path: &str| add_english_counter(query, path);
        Self {
            query: Some(query),
            query_status,
            standby_reserve: add("\\Memory\\Standby Cache Reserve Bytes"),
            standby_normal: add("\\Memory\\Standby Cache Normal Priority Bytes"),
            standby_core: add("\\Memory\\Standby Cache Core Bytes"),
            modified: add("\\Memory\\Modified Page List Bytes"),
            free_zero: add("\\Memory\\Free & Zero Page List Bytes"),
            page_reads: add("\\Memory\\Page Reads/sec"),
            collections: 0,
        }
    }

    fn collect(&mut self) -> Result<(), String> {
        let Some(query) = self.query else {
            return Err(format!(
                "PDH query unavailable (status 0x{:08X}).",
                self.query_status
            ));
        };
        let status = unsafe { PdhCollectQueryData(query) };
        if status == PDH_SUCCESS {
            self.collections += 1;
            Ok(())
        } else {
            Err(format!("PDH collection failed with status 0x{status:08X}."))
        }
    }

    fn metric(
        &self,
        counter: CounterHandle,
        id: &str,
        unit: &str,
        timestamp: u64,
        interval: u64,
        requires_two_samples: bool,
    ) -> Metric {
        if counter.registration_status != PDH_SUCCESS {
            return Metric::missing(
                id,
                unit,
                "PDH English counter",
                timestamp,
                interval,
                MetricStatus::Unavailable,
                counter.unavailable_status(),
            );
        }
        if requires_two_samples && self.collections < 2 {
            return Metric::missing(
                id,
                unit,
                "PDH English counter",
                timestamp,
                interval,
                MetricStatus::WarmingUp,
                "This rate counter needs two samples.",
            );
        }
        let Some(handle) = counter.handle else {
            return Metric::missing(
                id,
                unit,
                "PDH English counter",
                timestamp,
                interval,
                MetricStatus::Unavailable,
                "Counter handle was not created.",
            );
        };

        let mut value = PDH_FMT_COUNTERVALUE::default();
        let status =
            unsafe { PdhGetFormattedCounterValue(handle, PDH_FMT_DOUBLE, None, &mut value) };
        if status != PDH_SUCCESS {
            return Metric::missing(
                id,
                unit,
                "PDH English counter",
                timestamp,
                interval,
                MetricStatus::Error,
                format!("PDH formatting failed with status 0x{status:08X}."),
            );
        }
        if value.CStatus != PDH_CSTATUS_VALID_DATA && value.CStatus != PDH_CSTATUS_NEW_DATA {
            return Metric::missing(
                id,
                unit,
                "PDH English counter",
                timestamp,
                interval,
                MetricStatus::Unavailable,
                format!("PDH sample status 0x{:08X}.", value.CStatus),
            );
        }
        let number = unsafe { value.Anonymous.doubleValue };
        if !number.is_finite() || number < 0.0 {
            return Metric::missing(
                id,
                unit,
                "PDH English counter",
                timestamp,
                interval,
                MetricStatus::Error,
                "PDH returned a non-finite or negative value.",
            );
        }
        Metric::available(id, number, unit, "PDH English counter", timestamp, interval)
    }
}

impl Drop for PdhCounters {
    fn drop(&mut self) {
        if let Some(query) = self.query.take() {
            let status = unsafe { PdhCloseQuery(query) };
            if status != PDH_SUCCESS {
                log::warn!("Failed to close PDH query: 0x{status:08X}");
            }
        }
    }
}

fn failed_counter(status: u32) -> CounterHandle {
    CounterHandle {
        handle: None,
        registration_status: status,
    }
}

fn add_english_counter(query: PDH_HQUERY, path: &str) -> CounterHandle {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut counter = PDH_HCOUNTER::default();
    let status = unsafe { PdhAddEnglishCounterW(query, PCWSTR(wide.as_ptr()), 0, &mut counter) };
    CounterHandle {
        handle: (status == PDH_SUCCESS).then_some(counter),
        registration_status: status,
    }
}

pub struct PlatformCollector {
    pdh: PdhCounters,
    notifications: ResourceNotifications,
}

impl PlatformCollector {
    pub fn new() -> Self {
        Self {
            pdh: PdhCounters::new(),
            notifications: ResourceNotifications::new(),
        }
    }
}

impl Collector for PlatformCollector {
    fn collect(&mut self, sequence: u64, sample_interval_ms: u64) -> MemorySnapshot {
        let timestamp = timestamp_ms();
        let pdh_collection_error = self.pdh.collect().err();
        let (os_memory_state, notification_error) = self.notifications.state();

        let mut memory = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        let memory_result = unsafe { GlobalMemoryStatusEx(&mut memory) };

        let mut performance = PERFORMANCE_INFORMATION {
            cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
            ..Default::default()
        };
        let performance_result = unsafe {
            GetPerformanceInfo(
                &mut performance,
                size_of::<PERFORMANCE_INFORMATION>() as u32,
            )
        };

        if let Err(error) = memory_result {
            let detail = format!("GlobalMemoryStatusEx failed: {error}");
            let missing = |id: &str, unit: &str| {
                Metric::missing(
                    id,
                    unit,
                    "GlobalMemoryStatusEx",
                    timestamp,
                    sample_interval_ms,
                    MetricStatus::Error,
                    detail.clone(),
                )
            };
            return MemorySnapshot {
                sequence,
                timestamp_ms: timestamp,
                sample_interval_ms,
                collection_status: CollectionStatus::Error,
                collection_detail: Some(detail.clone()),
                os_memory_state,
                total_physical_bytes: missing("total_physical_bytes", "bytes"),
                available_physical_bytes: missing("available_physical_bytes", "bytes"),
                non_available_physical_bytes: missing("non_available_physical_bytes", "bytes"),
                physical_load_percent: missing("physical_load_percent", "percent"),
                commit_used_bytes: missing("commit_used_bytes", "bytes"),
                commit_limit_bytes: missing("commit_limit_bytes", "bytes"),
                commit_percent: missing("commit_percent", "percent"),
                standby_bytes: self.pdh.metric(
                    self.pdh.standby_reserve,
                    "standby_bytes",
                    "bytes",
                    timestamp,
                    sample_interval_ms,
                    false,
                ),
                modified_bytes: self.pdh.metric(
                    self.pdh.modified,
                    "modified_bytes",
                    "bytes",
                    timestamp,
                    sample_interval_ms,
                    false,
                ),
                free_zero_bytes: self.pdh.metric(
                    self.pdh.free_zero,
                    "free_zero_bytes",
                    "bytes",
                    timestamp,
                    sample_interval_ms,
                    false,
                ),
                page_reads_per_second: self.pdh.metric(
                    self.pdh.page_reads,
                    "page_reads_per_second",
                    "reads_per_second",
                    timestamp,
                    sample_interval_ms,
                    true,
                ),
                process_count: missing("process_count", "count"),
                thread_count: missing("thread_count", "count"),
            };
        }

        let total = memory.ullTotalPhys as f64;
        let available = memory.ullAvailPhys as f64;
        let non_available = (memory.ullTotalPhys.saturating_sub(memory.ullAvailPhys)) as f64;
        let total_metric = Metric::available(
            "total_physical_bytes",
            total,
            "bytes",
            "GlobalMemoryStatusEx",
            timestamp,
            sample_interval_ms,
        );
        let available_metric = Metric::available(
            "available_physical_bytes",
            available,
            "bytes",
            "GlobalMemoryStatusEx",
            timestamp,
            sample_interval_ms,
        );
        let non_available_metric = Metric::available(
            "non_available_physical_bytes",
            non_available,
            "bytes",
            "derived: total - available",
            timestamp,
            sample_interval_ms,
        );
        let load_metric = Metric::available(
            "physical_load_percent",
            memory.dwMemoryLoad as f64,
            "percent",
            "GlobalMemoryStatusEx",
            timestamp,
            sample_interval_ms,
        );

        let (commit_used, commit_limit, commit_source, performance_detail) =
            if performance_result.is_ok() && performance.PageSize > 0 {
                (
                    performance.CommitTotal.saturating_mul(performance.PageSize) as f64,
                    performance.CommitLimit.saturating_mul(performance.PageSize) as f64,
                    "GetPerformanceInfo",
                    None,
                )
            } else {
                (
                    memory
                        .ullTotalPageFile
                        .saturating_sub(memory.ullAvailPageFile) as f64,
                    memory.ullTotalPageFile as f64,
                    "GlobalMemoryStatusEx fallback",
                    performance_result
                        .as_ref()
                        .err()
                        .map(|error| format!("GetPerformanceInfo failed: {error}")),
                )
            };

        let commit_used_metric = Metric::available(
            "commit_used_bytes",
            commit_used,
            "bytes",
            commit_source,
            timestamp,
            sample_interval_ms,
        );
        let commit_limit_metric = Metric::available(
            "commit_limit_bytes",
            commit_limit,
            "bytes",
            commit_source,
            timestamp,
            sample_interval_ms,
        );
        let commit_percent_metric = match percent(commit_used, commit_limit) {
            Some(value) => Metric::available(
                "commit_percent",
                value,
                "percent",
                "derived: commit used / limit",
                timestamp,
                sample_interval_ms,
            ),
            None => Metric::missing(
                "commit_percent",
                "percent",
                "derived: commit used / limit",
                timestamp,
                sample_interval_ms,
                MetricStatus::Error,
                "Commit limit was zero or invalid.",
            ),
        };

        let reserve = self.pdh.metric(
            self.pdh.standby_reserve,
            "standby_reserve_bytes",
            "bytes",
            timestamp,
            sample_interval_ms,
            false,
        );
        let normal = self.pdh.metric(
            self.pdh.standby_normal,
            "standby_normal_bytes",
            "bytes",
            timestamp,
            sample_interval_ms,
            false,
        );
        let core = self.pdh.metric(
            self.pdh.standby_core,
            "standby_core_bytes",
            "bytes",
            timestamp,
            sample_interval_ms,
            false,
        );
        let standby = match (reserve.value, normal.value, core.value) {
            (Some(reserve), Some(normal), Some(core)) => Metric::available(
                "standby_bytes",
                reserve + normal + core,
                "bytes",
                "PDH sum: standby reserve + normal + core",
                timestamp,
                sample_interval_ms,
            ),
            _ => Metric::missing(
                "standby_bytes",
                "bytes",
                "PDH English counters",
                timestamp,
                sample_interval_ms,
                MetricStatus::Unavailable,
                "One or more standby-class counters are unavailable; a partial sum is not shown.",
            ),
        };

        let process_count = if performance_result.is_ok() {
            Metric::available(
                "process_count",
                performance.ProcessCount as f64,
                "count",
                "GetPerformanceInfo",
                timestamp,
                sample_interval_ms,
            )
        } else {
            Metric::missing(
                "process_count",
                "count",
                "GetPerformanceInfo",
                timestamp,
                sample_interval_ms,
                MetricStatus::Unavailable,
                performance_detail
                    .clone()
                    .unwrap_or_else(|| "GetPerformanceInfo unavailable.".into()),
            )
        };
        let thread_count = if performance_result.is_ok() {
            Metric::available(
                "thread_count",
                performance.ThreadCount as f64,
                "count",
                "GetPerformanceInfo",
                timestamp,
                sample_interval_ms,
            )
        } else {
            Metric::missing(
                "thread_count",
                "count",
                "GetPerformanceInfo",
                timestamp,
                sample_interval_ms,
                MetricStatus::Unavailable,
                performance_detail
                    .clone()
                    .unwrap_or_else(|| "GetPerformanceInfo unavailable.".into()),
            )
        };

        let mut details = Vec::new();
        if let Some(error) = pdh_collection_error {
            details.push(error);
        }
        if let Some(error) = notification_error {
            details.push(error);
        }
        if let Some(error) = performance_detail {
            details.push(error);
        }
        if standby.value.is_none() {
            details.push("Standby composition is unavailable.".into());
        }

        MemorySnapshot {
            sequence,
            timestamp_ms: timestamp,
            sample_interval_ms,
            collection_status: if details.is_empty() {
                CollectionStatus::Monitoring
            } else {
                CollectionStatus::Degraded
            },
            collection_detail: (!details.is_empty()).then(|| details.join(" ")),
            os_memory_state,
            total_physical_bytes: total_metric,
            available_physical_bytes: available_metric,
            non_available_physical_bytes: non_available_metric,
            physical_load_percent: load_metric,
            commit_used_bytes: commit_used_metric,
            commit_limit_bytes: commit_limit_metric,
            commit_percent: commit_percent_metric,
            standby_bytes: standby,
            modified_bytes: self.pdh.metric(
                self.pdh.modified,
                "modified_bytes",
                "bytes",
                timestamp,
                sample_interval_ms,
                false,
            ),
            free_zero_bytes: self.pdh.metric(
                self.pdh.free_zero,
                "free_zero_bytes",
                "bytes",
                timestamp,
                sample_interval_ms,
                false,
            ),
            page_reads_per_second: self.pdh.metric(
                self.pdh.page_reads,
                "page_reads_per_second",
                "reads_per_second",
                timestamp,
                sample_interval_ms,
                true,
            ),
            process_count,
            thread_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_windows_collector_returns_real_core_metrics() {
        let mut collector = PlatformCollector::new();
        let snapshot = collector.collect(1, 1_000);
        let total = snapshot
            .total_physical_bytes
            .value
            .expect("total physical memory should be available on Windows");
        let available = snapshot
            .available_physical_bytes
            .value
            .expect("available physical memory should be available on Windows");
        let commit_limit = snapshot
            .commit_limit_bytes
            .value
            .expect("commit limit should be available on Windows");
        assert!(total > 0.0);
        assert!((0.0..=total).contains(&available));
        assert!(commit_limit > 0.0);
        assert_eq!(snapshot.total_physical_bytes.source, "GlobalMemoryStatusEx");
    }
}
