use crate::telemetry::{Metric, MetricStatus, timestamp_ms};

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
    Some((total - idle) as f64 / total as f64 * 100.0)
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

pub fn foreground_executable() -> Option<String> {
    platform::foreground_executable()
}

#[cfg(target_os = "windows")]
mod platform {
    use super::CpuTimes;
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Threading::{
        GetActiveProcessorGroupCount, GetSystemTimes, OpenProcess, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

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

    pub fn foreground_executable() -> Option<String> {
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
        String::from_utf16(&buffer[..size as usize]).ok()
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::CpuTimes;
    pub(super) fn cpu_times() -> Result<CpuTimes, String> {
        Err("Windows CPU telemetry unavailable on this platform.".into())
    }
    pub fn foreground_executable() -> Option<String> {
        None
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
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_cpu_sensor_warms_up_and_returns_real_bounded_delta() {
        let mut sensor = CpuSensor::default();
        let first = sensor.collect(100, true);
        if first.status == MetricStatus::Unavailable {
            return;
        }
        assert_eq!(first.status, MetricStatus::WarmingUp);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let next = sensor.collect(100, true);
        assert!((0.0..=100.0).contains(&next.value.expect("real CPU delta")));
        assert_eq!(sensor.collect(100, false).value, None);
        assert_eq!(sensor.collect(100, true).status, MetricStatus::WarmingUp);
    }
}
