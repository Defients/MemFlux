use crate::telemetry::{Metric, MetricStatus, timestamp_ms};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
struct CpuTimes {
    idle: u64,
    kernel: u64,
    user: u64,
}

fn cpu_delta(previous: CpuTimes, next: CpuTimes) -> Option<f64> {
    let idle = next.idle.checked_sub(previous.idle)?;
    let total = next
        .kernel
        .checked_sub(previous.kernel)?
        .checked_add(next.user.checked_sub(previous.user)?)?;
    if total == 0 || idle > total {
        return None;
    }
    Some(((total - idle) as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapterInfo {
    pub luid_key: String,
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_memory_bytes: u64,
    pub shared_system_memory_bytes: u64,
    pub is_hardware: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuEngineBreakdown {
    pub busiest_engine_type: String,
    pub three_d_percent: f64,
    pub compute_percent: f64,
    pub copy_percent: f64,
    pub video_decode_percent: f64,
    pub video_encode_percent: f64,
    pub other_percent: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTelemetry {
    pub utilization: Metric,
    pub selected_adapter: Option<GpuAdapterInfo>,
    pub available_adapters: Vec<GpuAdapterInfo>,
    pub engine_breakdown: Option<GpuEngineBreakdown>,
    #[serde(skip)]
    pub per_process_gpu_percent: HashMap<u32, f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsCaptureStatus {
    pub active: bool,
    pub provider_available: bool,
    pub provider_name: String,
    pub target_pid: Option<u32>,
    pub target_executable: Option<String>,
    pub fps: Metric,
    pub frame_time_ms: Metric,
    pub frame_time_p99_ms: Option<f64>,
    pub status_detail: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedGpuEngineInstance {
    pub pid: u32,
    pub luid_key: String,
    pub physical_engine_key: String,
    pub engine_category: &'static str,
}

pub fn parse_gpu_engine_instance(instance_name: &str) -> Option<ParsedGpuEngineInstance> {
    let lower = instance_name.trim().to_ascii_lowercase();
    let pid_rest = lower.strip_prefix("pid_")?;
    let (pid_str, after_pid) = pid_rest.split_once("_luid_")?;
    let pid = pid_str.parse::<u32>().ok()?;
    let (luid_part, after_luid) = after_pid.split_once("_phys_")?;
    let engtype_raw = after_luid
        .rsplit_once("_engtype_")
        .map(|(_, t)| t)
        .unwrap_or("other");
    let category = classify_engine_type(engtype_raw);
    Some(ParsedGpuEngineInstance {
        pid,
        luid_key: luid_part.to_owned(),
        physical_engine_key: format!("phys_{after_luid}"),
        engine_category: category,
    })
}

pub fn classify_engine_type(raw: &str) -> &'static str {
    let lower = raw.to_ascii_lowercase();
    if lower == "3d" || lower.contains("graphics") {
        "3D"
    } else if lower.contains("compute") {
        "Compute"
    } else if lower.contains("copy") {
        "Copy"
    } else if lower.contains("videodecode") || lower.contains("decode") {
        "VideoDecode"
    } else if lower.contains("videoencode") || lower.contains("encode") {
        "VideoEncode"
    } else {
        "Other"
    }
}

pub fn aggregate_gpu_engine_samples(
    samples: &[(String, f64)],
    target_luid: &str,
) -> (Option<(f64, GpuEngineBreakdown)>, HashMap<u32, f64>) {
    // Map (luid_key, physical_engine_key) -> (engine_category, summed_utilization_across_pids)
    let mut engine_totals: HashMap<(String, String), (&'static str, f64)> = HashMap::new();
    // Map (pid, luid_key, physical_engine_key) -> summed_utilization
    let mut pid_engine_totals: HashMap<(u32, String, String), f64> = HashMap::new();

    for (name, value) in samples {
        if !value.is_finite() || *value < 0.0 {
            continue;
        }
        let Some(parsed) = parse_gpu_engine_instance(name) else {
            continue;
        };
        let clamped = value.clamp(0.0, 100.0);
        let entry = engine_totals
            .entry((parsed.luid_key.clone(), parsed.physical_engine_key.clone()))
            .or_insert((parsed.engine_category, 0.0));
        entry.1 = (entry.1 + clamped).clamp(0.0, 100.0);

        if parsed.pid > 0 {
            let pid_entry = pid_engine_totals
                .entry((parsed.pid, parsed.luid_key, parsed.physical_engine_key))
                .or_insert(0.0);
            *pid_entry = (*pid_entry + clamped).clamp(0.0, 100.0);
        }
    }

    let mut per_process_gpu: HashMap<u32, f64> = HashMap::new();
    for ((pid, _, _), val) in pid_engine_totals {
        let entry = per_process_gpu.entry(pid).or_insert(0.0);
        if val > *entry {
            *entry = val;
        }
    }

    let target_lower = target_luid.to_ascii_lowercase();
    let mut breakdown = GpuEngineBreakdown {
        busiest_engine_type: "3D".into(),
        ..Default::default()
    };
    let mut matched_any = false;
    let mut busiest_val = 0.0_f64;
    let mut busiest_cat = "3D";

    for ((luid, _), (category, val)) in &engine_totals {
        if !target_lower.is_empty() && luid != &target_lower {
            continue;
        }
        matched_any = true;
        let v = val.clamp(0.0, 100.0);
        match *category {
            "3D" => breakdown.three_d_percent = breakdown.three_d_percent.max(v),
            "Compute" => breakdown.compute_percent = breakdown.compute_percent.max(v),
            "Copy" => breakdown.copy_percent = breakdown.copy_percent.max(v),
            "VideoDecode" => breakdown.video_decode_percent = breakdown.video_decode_percent.max(v),
            "VideoEncode" => breakdown.video_encode_percent = breakdown.video_encode_percent.max(v),
            _ => breakdown.other_percent = breakdown.other_percent.max(v),
        }
        if v > busiest_val {
            busiest_val = v;
            busiest_cat = *category;
        }
    }

    if !matched_any {
        return (None, per_process_gpu);
    }
    breakdown.busiest_engine_type = busiest_cat.into();
    (Some((busiest_val, breakdown)), per_process_gpu)
}

#[derive(Default)]
pub struct CpuSensor {
    previous: Option<CpuTimes>,
}

impl CpuSensor {
    pub fn collect(&mut self, interval: u64, enabled: bool) -> Metric {
        let timestamp = timestamp_ms();
        let missing = |status, detail: String| {
            Metric::missing(
                "cpu_utilization_percent",
                "percent",
                "GetSystemTimes",
                timestamp,
                interval,
                status,
                detail,
            )
        };
        if !enabled {
            self.previous = None;
            return missing(
                MetricStatus::Unavailable,
                "CPU collection suspended while dashboard is hidden or HUD disabled.".into(),
            );
        }
        let current = match platform::cpu_times() {
            Ok(value) => value,
            Err(error) => {
                self.previous = None;
                return missing(MetricStatus::Unavailable, error);
            }
        };
        let previous = self.previous.replace(current);
        match previous {
            None => missing(
                MetricStatus::WarmingUp,
                "CPU utilization requires two samples.".into(),
            ),
            Some(previous) => match cpu_delta(previous, current) {
                Some(value) => Metric::available(
                    "cpu_utilization_percent",
                    value,
                    "percent",
                    "GetSystemTimes: busy / (kernel + user), idle included in kernel",
                    timestamp,
                    interval,
                ),
                None => missing(
                    MetricStatus::WarmingUp,
                    "CPU timing counter reset or invalid/zero delta; baseline refreshed.".into(),
                ),
            },
        }
    }
}

pub struct GpuSensor {
    inner: platform::PlatformGpuSensor,
}

impl Default for GpuSensor {
    fn default() -> Self {
        Self {
            inner: platform::PlatformGpuSensor::new(),
        }
    }
}

impl GpuSensor {
    pub fn collect(&mut self, interval: u64, enabled: bool, preferred_luid: &str) -> GpuTelemetry {
        self.inner.collect(interval, enabled, preferred_luid)
    }

    #[cfg(test)]
    pub fn adapters(&self) -> Vec<GpuAdapterInfo> {
        self.inner.adapters()
    }
}

struct PresentMonRollingWindow {
    frame_times_ms: Vec<f64>,
    last_sample_timestamp_ms: u64,
}

pub struct PresentMonController {
    binary_path: Option<PathBuf>,
    target_pid: Option<u32>,
    target_executable: Option<String>,
    child: Option<Child>,
    rolling: Arc<Mutex<PresentMonRollingWindow>>,
}

impl Default for PresentMonController {
    fn default() -> Self {
        Self::new()
    }
}

impl PresentMonController {
    pub fn new() -> Self {
        Self {
            binary_path: detect_presentmon_binary(),
            target_pid: None,
            target_executable: None,
            child: None,
            rolling: Arc::new(Mutex::new(PresentMonRollingWindow {
                frame_times_ms: Vec::new(),
                last_sample_timestamp_ms: 0,
            })),
        }
    }

    pub fn start_capture(
        &mut self,
        pid: u32,
        executable_name: String,
        permitted: bool,
    ) -> Result<FpsCaptureStatus, String> {
        self.stop_capture();
        if !permitted {
            return Err("FPS capture is disabled in HUD settings.".into());
        }
        let Some(binary) = self.binary_path.clone() else {
            return Err(
                "PresentMon capture binary is not installed on this system (set MEMFLUX_PRESENTMON_PATH or install Intel PresentMon)."
                    .into(),
            );
        };
        if !platform::is_process_alive(pid) {
            return Err(format!("Target process PID {pid} is not running."));
        }

        let mut child = Command::new(&binary)
            .args([
                "--process_id",
                &pid.to_string(),
                "--output_stdout",
                "--no_top",
                "--stop_existing_session",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start PresentMon capture: {e}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "PresentMon stdout pipe unavailable.".to_string())?;
        let rolling = self.rolling.clone();
        if let Ok(mut guard) = rolling.lock() {
            guard.frame_times_ms.clear();
            guard.last_sample_timestamp_ms = 0;
        }

        std::thread::Builder::new()
            .name("memflux-presentmon-reader".into())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                let mut ms_between_presents_idx: Option<usize> = None;
                for line in reader.lines().map_while(Result::ok) {
                    if ms_between_presents_idx.is_none() {
                        let cols: Vec<&str> = line.split(',').collect();
                        ms_between_presents_idx = cols.iter().position(|c| {
                            c.eq_ignore_ascii_case("MsBetweenPresents")
                                || c.eq_ignore_ascii_case("msBetweenPresents")
                        });
                        continue;
                    }
                    if let Some(idx) = ms_between_presents_idx
                        && let Some(val_str) = line.split(',').nth(idx)
                        && let Ok(ms) = val_str.trim().parse::<f64>()
                        && ms.is_finite()
                        && (0.1..=1000.0).contains(&ms)
                        && let Ok(mut guard) = rolling.lock()
                    {
                        if guard.frame_times_ms.len() >= 240 {
                            guard.frame_times_ms.remove(0);
                        }
                        guard.frame_times_ms.push(ms);
                        guard.last_sample_timestamp_ms = timestamp_ms();
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn PresentMon reader thread: {e}"))?;

        self.target_pid = Some(pid);
        self.target_executable = Some(executable_name);
        self.child = Some(child);
        Ok(self.status(1_000, permitted))
    }

    pub fn stop_capture(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.target_pid = None;
        self.target_executable = None;
        if let Ok(mut guard) = self.rolling.lock() {
            guard.frame_times_ms.clear();
            guard.last_sample_timestamp_ms = 0;
        }
    }

    pub fn status(&mut self, interval: u64, permitted: bool) -> FpsCaptureStatus {
        let now = timestamp_ms();
        if !permitted {
            self.stop_capture();
            return unavailable_capture(
                now,
                interval,
                self.binary_path.is_some(),
                "FPS capture is disabled by user preference.",
            );
        }
        let Some(_binary) = &self.binary_path else {
            return unavailable_capture(
                now,
                interval,
                false,
                "PresentMon capture binary is not installed on this system.",
            );
        };

        if let Some(pid) = self.target_pid
            && !platform::is_process_alive(pid)
        {
            self.stop_capture();
            return unavailable_capture(
                now,
                interval,
                true,
                "Captured game process exited; capture session stopped cleanly.",
            );
        }

        if let Some(child) = &mut self.child
            && let Ok(Some(_)) = child.try_wait()
        {
            self.stop_capture();
            return unavailable_capture(
                now,
                interval,
                true,
                "PresentMon session ended or required administrator/Performance Log Users permission.",
            );
        }

        let Some(pid) = self.target_pid else {
            return unavailable_capture(
                now,
                interval,
                true,
                "No FPS capture session active. Select a game to start capture.",
            );
        };

        let (samples, last_ts) = {
            let guard = self
                .rolling
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            (guard.frame_times_ms.clone(), guard.last_sample_timestamp_ms)
        };

        if samples.is_empty() || now.saturating_sub(last_ts) > 3_500 {
            let status = if last_ts == 0 {
                MetricStatus::WarmingUp
            } else {
                MetricStatus::Unavailable
            };
            let detail = if last_ts == 0 {
                "Waiting for presented frames from target process."
            } else {
                "Frame stream is stale (no recent presents observed)."
            };
            return FpsCaptureStatus {
                active: true,
                provider_available: true,
                provider_name: "Intel PresentMon ETW (non-injecting)".into(),
                target_pid: Some(pid),
                target_executable: self.target_executable.clone(),
                fps: Metric::missing(
                    "fps",
                    "fps",
                    "PresentMon MsBetweenPresents",
                    now,
                    interval,
                    status.clone(),
                    detail,
                ),
                frame_time_ms: Metric::missing(
                    "frame_time_ms",
                    "ms",
                    "PresentMon MsBetweenPresents",
                    now,
                    interval,
                    status,
                    detail,
                ),
                frame_time_p99_ms: None,
                status_detail: detail.into(),
            };
        }

        let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
        let fps_val = if mean_ms > 0.0 { 1000.0 / mean_ms } else { 0.0 };
        let mut sorted = samples;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p99_idx = ((sorted.len() as f64 * 0.99).floor() as usize).min(sorted.len() - 1);
        let p99_ms = sorted[p99_idx];

        FpsCaptureStatus {
            active: true,
            provider_available: true,
            provider_name: "Intel PresentMon ETW (non-injecting)".into(),
            target_pid: Some(pid),
            target_executable: self.target_executable.clone(),
            fps: Metric::available(
                "fps",
                fps_val,
                "fps",
                "PresentMon: 1000 / mean(MsBetweenPresents)",
                now,
                interval,
            ),
            frame_time_ms: Metric::available(
                "frame_time_ms",
                mean_ms,
                "ms",
                "PresentMon: mean(MsBetweenPresents)",
                now,
                interval,
            ),
            frame_time_p99_ms: Some(p99_ms),
            status_detail: format!("Capturing PID {pid} ({:.1} ms P99)", p99_ms),
        }
    }
}

impl Drop for PresentMonController {
    fn drop(&mut self) {
        self.stop_capture();
    }
}

fn unavailable_capture(
    now: u64,
    interval: u64,
    provider_available: bool,
    detail: &str,
) -> FpsCaptureStatus {
    FpsCaptureStatus {
        active: false,
        provider_available,
        provider_name: "Intel PresentMon ETW (non-injecting)".into(),
        target_pid: None,
        target_executable: None,
        fps: Metric::missing(
            "fps",
            "fps",
            "PresentMon",
            now,
            interval,
            MetricStatus::Unavailable,
            detail,
        ),
        frame_time_ms: Metric::missing(
            "frame_time_ms",
            "ms",
            "PresentMon",
            now,
            interval,
            MetricStatus::Unavailable,
            detail,
        ),
        frame_time_p99_ms: None,
        status_detail: detail.into(),
    }
}

fn detect_presentmon_binary() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("MEMFLUX_PRESENTMON_PATH") {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Some(path);
        }
    }
    let candidates = [
        r"C:\Program Files\Intel\PresentMon\PresentMon.exe",
        r"C:\Program Files\Intel\PresentMon\PresentMon-2.0.0-x64.exe",
    ];
    for candidate in candidates {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let exe = dir.join("PresentMon.exe");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

pub fn foreground_executable() -> Option<String> {
    platform::foreground_executable()
}

pub fn foreground_window_process() -> Option<(u32, String)> {
    platform::foreground_window_process()
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{
        CpuTimes, GpuAdapterInfo, GpuTelemetry, Metric, MetricStatus, aggregate_gpu_engine_samples,
        timestamp_ms,
    };
    use std::collections::HashMap;
    use windows::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, FILETIME, STILL_ACTIVE};
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };
    use windows::Win32::System::Performance::{
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
        PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
        PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    };
    use windows::Win32::System::Threading::{
        GetActiveProcessorGroupCount, GetExitCodeProcess, GetSystemTimes, OpenProcess,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::{PCWSTR, PWSTR};

    const PDH_SUCCESS: u32 = 0;
    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    pub(super) fn cpu_times() -> Result<CpuTimes, String> {
        if unsafe { GetActiveProcessorGroupCount() } != 1 {
            return Err("Overall CPU unavailable on multi-processor-group systems; group-aware collection required.".into());
        }
        let (mut idle, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }
            .map_err(|error| error.to_string())?;
        let ticks =
            |time: FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
        Ok(CpuTimes {
            idle: ticks(idle),
            kernel: ticks(kernel),
            user: ticks(user),
        })
    }

    pub fn foreground_window_process() -> Option<(u32, String)> {
        let window = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        if window.is_invalid()
            || unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) } == 0
            || pid == 0
        {
            return None;
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut buffer = vec![0_u16; 32_768];
        let mut size = buffer.len() as u32;
        let result = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        };
        let _ = unsafe { CloseHandle(process) };
        let mut current_pid = 0;
        if result.is_err()
            || unsafe { GetForegroundWindow() } != window
            || unsafe { GetWindowThreadProcessId(window, Some(&mut current_pid)) } == 0
            || current_pid != pid
        {
            return None;
        }
        let path = String::from_utf16(&buffer[..size as usize]).ok()?;
        Some((pid, path))
    }

    pub fn foreground_executable() -> Option<String> {
        foreground_window_process().map(|(_, path)| path)
    }

    pub fn is_process_alive(pid: u32) -> bool {
        let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
        else {
            return false;
        };
        let mut exit_code = 0_u32;
        let ok = unsafe { GetExitCodeProcess(handle, &mut exit_code) }.is_ok()
            && exit_code == STILL_ACTIVE.0 as u32;
        let _ = unsafe { CloseHandle(handle) };
        ok
    }

    pub fn enumerate_dxgi_adapters() -> Vec<GpuAdapterInfo> {
        let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
            return Vec::new();
        };
        let mut adapters = Vec::new();
        let mut index = 0_u32;
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
            index += 1;
            let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
                continue;
            };
            let len = desc
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.Description.len());
            let name = String::from_utf16_lossy(&desc.Description[..len])
                .trim()
                .to_string();
            let is_software = (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0;
            let luid_key = format!(
                "0x{:08x}_0x{:08x}",
                desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
            );
            adapters.push(GpuAdapterInfo {
                luid_key,
                name,
                vendor_id: desc.VendorId,
                device_id: desc.DeviceId,
                dedicated_video_memory_bytes: desc.DedicatedVideoMemory as u64,
                shared_system_memory_bytes: desc.SharedSystemMemory as u64,
                is_hardware: !is_software,
            });
        }
        // Sort hardware adapters with dedicated VRAM first
        adapters.sort_by(|a, b| {
            b.is_hardware.cmp(&a.is_hardware).then_with(|| {
                b.dedicated_video_memory_bytes
                    .cmp(&a.dedicated_video_memory_bytes)
            })
        });
        adapters
    }

    pub struct PlatformGpuSensor {
        query: Option<PDH_HQUERY>,
        wildcard_counter: Option<PDH_HCOUNTER>,
        collections: u64,
        adapters: Vec<GpuAdapterInfo>,
    }

    impl PlatformGpuSensor {
        pub fn new() -> Self {
            let adapters = enumerate_dxgi_adapters();
            let mut query = PDH_HQUERY::default();
            let open_status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
            if open_status != PDH_SUCCESS {
                return Self {
                    query: None,
                    wildcard_counter: None,
                    collections: 0,
                    adapters,
                };
            }
            let path: Vec<u16> = "\\GPU Engine(*)\\Utilization Percentage"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut counter = PDH_HCOUNTER::default();
            let add_status =
                unsafe { PdhAddEnglishCounterW(query, PCWSTR(path.as_ptr()), 0, &mut counter) };
            Self {
                query: Some(query),
                wildcard_counter: (add_status == PDH_SUCCESS).then_some(counter),
                collections: 0,
                adapters,
            }
        }

        #[cfg(test)]
        pub fn adapters(&self) -> Vec<GpuAdapterInfo> {
            self.adapters.clone()
        }

        pub fn collect(
            &mut self,
            interval: u64,
            enabled: bool,
            preferred_luid: &str,
        ) -> GpuTelemetry {
            let timestamp = timestamp_ms();
            let selected = self.select_adapter(preferred_luid).cloned();
            let adapter_label = selected
                .as_ref()
                .map(|a| a.name.as_str())
                .unwrap_or("WDDM Adapter");

            if !enabled {
                self.collections = 0;
                return GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::Unavailable,
                        "GPU collection suspended while dashboard is hidden or HUD disabled.",
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            }

            let (Some(query), Some(counter)) = (self.query, self.wildcard_counter) else {
                return GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::Unavailable,
                        "WDDM GPU Engine performance counter is unavailable on this host.",
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            };

            let collect_status = unsafe { PdhCollectQueryData(query) };
            if collect_status != PDH_SUCCESS {
                return GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::Error,
                        format!("PDH GPU collection failed (0x{collect_status:08X})."),
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            }
            self.collections = self.collections.saturating_add(1);
            if self.collections < 2 {
                return GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::WarmingUp,
                        "GPU Engine rate counters require two samples.",
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            }

            let mut buffer_size = 0_u32;
            let mut item_count = 0_u32;
            let first_status = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut buffer_size,
                    &mut item_count,
                    None,
                )
            };
            if first_status != PDH_MORE_DATA
                && first_status != ERROR_MORE_DATA.0
                && first_status != PDH_SUCCESS
            {
                return GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::Unavailable,
                        format!(
                            "No active WDDM GPU Engine counter instances (status 0x{first_status:08X})."
                        ),
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            }
            if buffer_size == 0 {
                return GpuTelemetry {
                    utilization: Metric::available(
                        "gpu_utilization_percent",
                        0.0,
                        "percent",
                        &format!("PDH GPU Engine ({adapter_label}: Busiest engine 3D)"),
                        timestamp,
                        interval,
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            }

            let elem_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let capacity = (buffer_size as usize).div_ceil(elem_size) + 16;
            let mut buffer: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = vec![Default::default(); capacity];
            let mut actual_bytes = (capacity * elem_size) as u32;
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut actual_bytes,
                    &mut item_count,
                    Some(buffer.as_mut_ptr()),
                )
            };
            if status != PDH_SUCCESS {
                return GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::Error,
                        format!("PDH GPU formatting failed (0x{status:08X})."),
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: HashMap::new(),
                };
            }

            let mut raw_samples = Vec::with_capacity(item_count as usize);
            for item in buffer.iter().take(item_count as usize) {
                if item.FmtValue.CStatus != PDH_CSTATUS_VALID_DATA
                    && item.FmtValue.CStatus != PDH_CSTATUS_NEW_DATA
                {
                    continue;
                }
                let Ok(name) = (unsafe { item.szName.to_string() }) else {
                    continue;
                };
                let val = unsafe { item.FmtValue.Anonymous.doubleValue };
                if val.is_finite() && val >= 0.0 {
                    raw_samples.push((name, val));
                }
            }

            let target_luid = selected.as_ref().map(|a| a.luid_key.as_str()).unwrap_or("");
            let (aggregated, per_process) = aggregate_gpu_engine_samples(&raw_samples, target_luid);

            match aggregated {
                Some((busiest_percent, breakdown)) => {
                    let source = format!(
                        "WDDM GPU Engine ({}: Busiest engine {})",
                        adapter_label, breakdown.busiest_engine_type
                    );
                    let mut metric = Metric::available(
                        "gpu_utilization_percent",
                        busiest_percent,
                        "percent",
                        &source,
                        timestamp,
                        interval,
                    );
                    metric.detail = Some(format!(
                        "{} — Busiest engine: {} ({:.1}%). 3D {:.1}% · Compute {:.1}% · Copy {:.1}% · Decode {:.1}% · Encode {:.1}%",
                        adapter_label,
                        breakdown.busiest_engine_type,
                        busiest_percent,
                        breakdown.three_d_percent,
                        breakdown.compute_percent,
                        breakdown.copy_percent,
                        breakdown.video_decode_percent,
                        breakdown.video_encode_percent,
                    ));
                    GpuTelemetry {
                        utilization: metric,
                        selected_adapter: selected,
                        available_adapters: self.adapters.clone(),
                        engine_breakdown: Some(breakdown),
                        per_process_gpu_percent: per_process,
                    }
                }
                None => GpuTelemetry {
                    utilization: Metric::missing(
                        "gpu_utilization_percent",
                        "percent",
                        "PDH GPU Engine",
                        timestamp,
                        interval,
                        MetricStatus::Unavailable,
                        format!("No WDDM engine counters matched adapter {adapter_label}."),
                    ),
                    selected_adapter: selected,
                    available_adapters: self.adapters.clone(),
                    engine_breakdown: None,
                    per_process_gpu_percent: per_process,
                },
            }
        }

        fn select_adapter(&self, preferred_luid: &str) -> Option<&GpuAdapterInfo> {
            if !preferred_luid.is_empty()
                && preferred_luid != "auto"
                && let Some(found) = self
                    .adapters
                    .iter()
                    .find(|a| a.luid_key.eq_ignore_ascii_case(preferred_luid))
            {
                return Some(found);
            }
            self.adapters
                .iter()
                .find(|a| a.is_hardware)
                .or_else(|| self.adapters.first())
        }
    }

    impl Drop for PlatformGpuSensor {
        fn drop(&mut self) {
            if let Some(query) = self.query.take() {
                let _ = unsafe { PdhCloseQuery(query) };
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::{CpuTimes, GpuAdapterInfo, GpuTelemetry, Metric, MetricStatus, timestamp_ms};
    use std::collections::HashMap;

    pub(super) fn cpu_times() -> Result<CpuTimes, String> {
        Err("Windows CPU telemetry unavailable on this platform.".into())
    }
    pub fn foreground_executable() -> Option<String> {
        None
    }
    pub fn foreground_window_process() -> Option<(u32, String)> {
        None
    }
    pub fn is_process_alive(_pid: u32) -> bool {
        false
    }
    pub struct PlatformGpuSensor;
    impl PlatformGpuSensor {
        pub fn new() -> Self {
            Self
        }
        pub fn adapters(&self) -> Vec<GpuAdapterInfo> {
            Vec::new()
        }
        pub fn collect(
            &mut self,
            interval: u64,
            _enabled: bool,
            _preferred_luid: &str,
        ) -> GpuTelemetry {
            GpuTelemetry {
                utilization: Metric::missing(
                    "gpu_utilization_percent",
                    "percent",
                    "Windows WDDM",
                    timestamp_ms(),
                    interval,
                    MetricStatus::Unavailable,
                    "Windows GPU telemetry unavailable on this platform.",
                ),
                selected_adapter: None,
                available_adapters: Vec::new(),
                engine_breakdown: None,
                per_process_gpu_percent: HashMap::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_delta_accounts_for_idle_and_rejects_discontinuities() {
        let zero = CpuTimes {
            idle: 0,
            kernel: 0,
            user: 0,
        };
        assert_eq!(
            cpu_delta(
                zero,
                CpuTimes {
                    idle: 60,
                    kernel: 80,
                    user: 20
                }
            ),
            Some(40.0)
        );
        assert_eq!(cpu_delta(zero, zero), None);
        assert_eq!(cpu_delta(CpuTimes { idle: 10, ..zero }, zero), None);
        assert_eq!(
            cpu_delta(
                zero,
                CpuTimes {
                    idle: 101,
                    kernel: 80,
                    user: 20
                }
            ),
            None
        );
        assert_eq!(
            cpu_delta(
                zero,
                CpuTimes {
                    idle: 0,
                    kernel: u64::MAX,
                    user: 1
                }
            ),
            None
        );
    }

    #[test]
    fn gpu_engine_aggregation_sums_same_engine_across_pids_and_takes_max_across_engines() {
        let samples = vec![
            (
                "pid_100_luid_0x00000000_0x00017a11_phys_0_eng_0_engtype_3D".to_string(),
                35.0,
            ),
            (
                "pid_200_luid_0x00000000_0x00017a11_phys_0_eng_0_engtype_3D".to_string(),
                25.0,
            ),
            (
                "pid_100_luid_0x00000000_0x00017a11_phys_0_eng_6_engtype_VideoEncode".to_string(),
                45.0,
            ),
            (
                "pid_300_luid_0x00000000_0x00099999_phys_0_eng_0_engtype_3D".to_string(),
                90.0,
            ),
        ];
        let (agg, per_pid) = aggregate_gpu_engine_samples(&samples, "0x00000000_0x00017a11");
        let (busiest, breakdown) = agg.expect("matched target LUID");
        // 3D engine 0 has 35% + 25% = 60%. VideoEncode engine 6 has 45%.
        // Busiest engine is 60% (NOT 60% + 45% = 105%!)
        assert_eq!(busiest, 60.0);
        assert_eq!(breakdown.busiest_engine_type, "3D");
        assert_eq!(breakdown.three_d_percent, 60.0);
        assert_eq!(breakdown.video_encode_percent, 45.0);
        assert_eq!(per_pid.get(&100), Some(&45.0));
        assert_eq!(per_pid.get(&200), Some(&25.0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_cpu_and_gpu_sensors_warm_up_and_return_real_bounded_metrics() {
        let mut cpu = CpuSensor::default();
        let first_cpu = cpu.collect(100, true);
        if first_cpu.status != MetricStatus::Unavailable {
            assert_eq!(first_cpu.status, MetricStatus::WarmingUp);
            std::thread::sleep(std::time::Duration::from_millis(100));
            let next_cpu = cpu.collect(100, true);
            assert!((0.0..=100.0).contains(&next_cpu.value.expect("real CPU delta")));
            assert_eq!(cpu.collect(100, false).value, None);
            assert_eq!(cpu.collect(100, true).status, MetricStatus::WarmingUp);
        }

        let mut gpu = GpuSensor::default();
        assert!(
            !gpu.adapters().is_empty(),
            "DXGI should enumerate at least one adapter"
        );
        let first_gpu = gpu.collect(100, true, "auto");
        if first_gpu.utilization.status != MetricStatus::Unavailable {
            assert_eq!(first_gpu.utilization.status, MetricStatus::WarmingUp);
            std::thread::sleep(std::time::Duration::from_millis(120));
            let next_gpu = gpu.collect(100, true, "auto");
            if next_gpu.utilization.status == MetricStatus::Available {
                let val = next_gpu.utilization.value.expect("GPU utilization value");
                assert!((0.0..=100.0).contains(&val));
            }
        }
    }
}
