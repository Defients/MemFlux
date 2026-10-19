use crate::cleaner::SessionCleanerOverride;
use crate::powershift::PowerShiftMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_PROTECTED_WORKLOADS: &[&str] = &[
    "obs64.exe",
    "obs32.exe",
    "discord.exe",
    "audiodg.exe",
    "voicemeeter.exe",
    "voicemeeterpro.exe",
    "voicemeeter8.exe",
    "nvidia broadcast.exe",
];

const NON_GAME_EXECUTABLES: &[&str] = &[
    "memflux.exe",
    "explorer.exe",
    "dwm.exe",
    "searchhost.exe",
    "searchapp.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "applicationframehost.exe",
    "systemsettings.exe",
    "taskmgr.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "conhost.exe",
    "windowsterminal.exe",
    "openconsole.exe",
    "code.exe",
    "devenv.exe",
    "windsurf.exe",
    "cursor.exe",
    "chrome.exe",
    "msedge.exe",
    "msedgewebview2.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "obs64.exe",
    "obs32.exe",
    "discord.exe",
    "slack.exe",
    "teams.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "battle.net.exe",
    "galaxyclient.exe",
    "eadesktop.exe",
    "ubisoftconnect.exe",
    "riotclientservices.exe",
    "audiodg.exe",
    "svchost.exe",
    "csrss.exe",
    "lsass.exe",
    "services.exe",
    "wininit.exe",
    "winlogon.exe",
    "sihost.exe",
    "ctfmon.exe",
    "RuntimeBroker.exe",
    "dllhost.exe",
    "notepad.exe",
    "snippingtool.exe",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GamePreset {
    #[default]
    BalancedGaming,
    PerformanceGaming,
    GamingAndStreaming,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GameSessionLifecycle {
    #[default]
    Idle,
    CandidateDetected,
    ConfirmedGame,
    ProfilePending,
    Active,
    TemporarilyBackgrounded,
    Restoring,
    Completed,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub id: String,
    pub name: String,
    pub executable: String,
    pub preset: GamePreset,
    pub powershift_mode: PowerShiftMode,
    pub custom_power_scheme_guid: Option<String>,
    pub cleaner_override: SessionCleanerOverride,
    pub auto_fps_capture: bool,
    pub approved_power_intervention: bool,
    #[serde(default)]
    pub protected_applications: Vec<String>,
    #[serde(default)]
    pub session_logging: bool,
}

impl GameProfile {
    #[allow(dead_code)]
    pub fn from_preset(id: String, name: String, executable: String, preset: GamePreset) -> Self {
        let exe_normalized = executable.trim().to_ascii_lowercase();
        match preset {
            GamePreset::BalancedGaming => Self {
                id,
                name,
                executable: exe_normalized,
                preset,
                powershift_mode: PowerShiftMode::Balanced,
                custom_power_scheme_guid: None,
                cleaner_override: SessionCleanerOverride::KeepGlobal,
                auto_fps_capture: false,
                approved_power_intervention: true,
                protected_applications: DEFAULT_PROTECTED_WORKLOADS
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                session_logging: true,
            },
            GamePreset::PerformanceGaming => Self {
                id,
                name,
                executable: exe_normalized,
                preset,
                powershift_mode: PowerShiftMode::Performance,
                custom_power_scheme_guid: None,
                cleaner_override: SessionCleanerOverride::EnableDuringSession,
                auto_fps_capture: true,
                approved_power_intervention: true,
                protected_applications: DEFAULT_PROTECTED_WORKLOADS
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                session_logging: true,
            },
            GamePreset::GamingAndStreaming => Self {
                id,
                name,
                executable: exe_normalized,
                preset,
                powershift_mode: PowerShiftMode::Balanced,
                custom_power_scheme_guid: None,
                cleaner_override: SessionCleanerOverride::PauseDuringSession,
                auto_fps_capture: false,
                approved_power_intervention: true,
                protected_applications: DEFAULT_PROTECTED_WORKLOADS
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                session_logging: true,
            },
            GamePreset::Custom => Self {
                id,
                name,
                executable: exe_normalized,
                preset,
                powershift_mode: PowerShiftMode::CurrentSystemMode,
                custom_power_scheme_guid: None,
                cleaner_override: SessionCleanerOverride::KeepGlobal,
                auto_fps_capture: false,
                approved_power_intervention: false,
                protected_applications: DEFAULT_PROTECTED_WORKLOADS
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                session_logging: true,
            },
        }
    }

    pub fn matches_executable(&self, candidate_path: &str) -> bool {
        let candidate = candidate_path.trim().to_ascii_lowercase();
        let target = self.executable.trim().to_ascii_lowercase();
        if target.is_empty() || candidate.is_empty() {
            return false;
        }
        if target.contains(['\\', '/']) {
            candidate == target
        } else {
            candidate.rsplit(['\\', '/']).next() == Some(target.as_str())
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetingProcessInfo {
    pub pid: u32,
    pub executable_name: String,
    pub cpu_percent: f64,
    pub working_set_bytes: u64,
    pub gpu_percent: f64,
    pub is_protected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateApplication {
    pub pid: u32,
    pub executable_name: String,
    pub executable_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFocusStatus {
    pub enabled: bool,
    pub lifecycle: GameSessionLifecycle,
    pub session_id: Option<String>,
    pub active_pid: Option<u32>,
    pub active_executable_name: Option<String>,
    pub active_executable_path: Option<String>,
    pub selected_profile: Option<GameProfile>,
    pub candidate: Option<CandidateApplication>,
    pub session_started_ms: Option<u64>,
    pub session_duration_ms: u64,
    pub active_power_scheme_name: Option<String>,
    pub active_power_scheme_guid: Option<String>,
    pub power_intervention_active: bool,
    pub cleaner_override: SessionCleanerOverride,
    pub streaming_detected: bool,
    pub active_protected_workloads: Vec<String>,
    pub competing_workloads: Vec<CompetingProcessInfo>,
    pub recommendations: Vec<String>,
    pub restoration_state: String,
    pub last_completed_summary: Option<String>,
}

impl Default for GameFocusStatus {
    fn default() -> Self {
        Self {
            enabled: true,
            lifecycle: GameSessionLifecycle::Idle,
            session_id: None,
            active_pid: None,
            active_executable_name: None,
            active_executable_path: None,
            selected_profile: None,
            candidate: None,
            session_started_ms: None,
            session_duration_ms: 0,
            active_power_scheme_name: None,
            active_power_scheme_guid: None,
            power_intervention_active: false,
            cleaner_override: SessionCleanerOverride::KeepGlobal,
            streaming_detected: false,
            active_protected_workloads: Vec::new(),
            competing_workloads: Vec::new(),
            recommendations: Vec::new(),
            restoration_state: "No session changes active".into(),
            last_completed_summary: None,
        }
    }
}

pub fn is_non_game_executable(path_or_name: &str) -> bool {
    let lower = path_or_name.trim().to_ascii_lowercase();
    let filename = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
    if NON_GAME_EXECUTABLES
        .iter()
        .any(|blocked| blocked.eq_ignore_ascii_case(filename))
    {
        return true;
    }
    lower.contains(r"\windows\system32\")
        || lower.contains(r"\windows\syswow64\")
        || lower.contains(r"\windows\systemapps\")
        || lower.contains(r"\windows\immersivecontrolpanel\")
}

pub fn is_protected_workload(filename: &str, custom_protected: &[String]) -> bool {
    let lower = filename.trim().to_ascii_lowercase();
    let base = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
    DEFAULT_PROTECTED_WORKLOADS
        .iter()
        .any(|p| p.eq_ignore_ascii_case(base))
        || custom_protected.iter().any(|p| {
            let p_low = p.trim().to_ascii_lowercase();
            p_low == lower || p_low == base
        })
}

#[derive(Default)]
pub struct WorkloadAnalyzer {
    prev_process_cpu_ticks: HashMap<u32, u64>,
    prev_total_cpu_ticks: u64,
}

impl WorkloadAnalyzer {
    pub fn sample_workloads(
        &mut self,
        active_game_pid: Option<u32>,
        custom_protected: &[String],
        per_process_gpu: &HashMap<u32, f64>,
    ) -> (Vec<String>, Vec<CompetingProcessInfo>, Vec<String>) {
        platform::sample_workloads(self, active_game_pid, custom_protected, per_process_gpu)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{CompetingProcessInfo, WorkloadAnalyzer, is_protected_workload};
    use std::collections::HashMap;
    use std::mem::size_of;
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, GetProcessTimes, GetSystemTimes, OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    fn ft_to_u64(ft: FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
    }

    pub fn sample_workloads(
        analyzer: &mut WorkloadAnalyzer,
        active_game_pid: Option<u32>,
        custom_protected: &[String],
        per_process_gpu: &HashMap<u32, f64>,
    ) -> (Vec<String>, Vec<CompetingProcessInfo>, Vec<String>) {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let total_cpu_ticks =
            if unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }
                .is_ok()
            {
                ft_to_u64(kernel).saturating_add(ft_to_u64(user))
            } else {
                0
            };
        let system_delta = total_cpu_ticks.saturating_sub(analyzer.prev_total_cpu_ticks);
        analyzer.prev_total_cpu_ticks = total_cpu_ticks;

        let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return (Vec::new(), Vec::new(), Vec::new());
        };

        let self_pid = unsafe { GetCurrentProcessId() };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut next_cpu_map = HashMap::new();
        let mut protected_running = Vec::new();
        let mut candidates = Vec::new();

        if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
            loop {
                let pid = entry.th32ProcessID;
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                let protected = is_protected_workload(&exe_name, custom_protected);
                if protected
                    && !protected_running
                        .iter()
                        .any(|p: &String| p.eq_ignore_ascii_case(&exe_name))
                {
                    protected_running.push(exe_name.clone());
                }

                if pid > 4
                    && pid != self_pid
                    && Some(pid) != active_game_pid
                    && let Ok(proc_handle) =
                        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
                {
                    let (mut c_time, mut e_time, mut k_time, mut u_time) = (
                        FILETIME::default(),
                        FILETIME::default(),
                        FILETIME::default(),
                        FILETIME::default(),
                    );
                    let mut cpu_pct = 0.0;
                    if unsafe {
                        GetProcessTimes(
                            proc_handle,
                            &mut c_time,
                            &mut e_time,
                            &mut k_time,
                            &mut u_time,
                        )
                    }
                    .is_ok()
                    {
                        let proc_ticks = ft_to_u64(k_time).saturating_add(ft_to_u64(u_time));
                        next_cpu_map.insert(pid, proc_ticks);
                        if system_delta > 0
                            && let Some(prev_ticks) = analyzer.prev_process_cpu_ticks.get(&pid)
                        {
                            let delta = proc_ticks.saturating_sub(*prev_ticks);
                            cpu_pct =
                                ((delta as f64 / system_delta as f64) * 100.0).clamp(0.0, 100.0);
                        }
                    }

                    let mut mem_counters = PROCESS_MEMORY_COUNTERS {
                        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                        ..Default::default()
                    };
                    let working_set = if unsafe {
                        GetProcessMemoryInfo(
                            proc_handle,
                            &mut mem_counters,
                            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                        )
                    }
                    .is_ok()
                    {
                        mem_counters.WorkingSetSize as u64
                    } else {
                        0
                    };
                    let _ = unsafe { CloseHandle(proc_handle) };

                    let gpu_pct = per_process_gpu.get(&pid).copied().unwrap_or(0.0);
                    if cpu_pct >= 1.5
                        || working_set >= 250 * 1024 * 1024
                        || gpu_pct >= 2.0
                        || protected
                    {
                        candidates.push(CompetingProcessInfo {
                            pid,
                            executable_name: exe_name,
                            cpu_percent: cpu_pct,
                            working_set_bytes: working_set,
                            gpu_percent: gpu_pct,
                            is_protected: protected,
                        });
                    }
                }

                if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                    break;
                }
            }
        }
        let _ = unsafe { CloseHandle(snapshot) };
        analyzer.prev_process_cpu_ticks = next_cpu_map;

        candidates.sort_by(|a, b| {
            let score_a = a.cpu_percent * 3.0
                + a.gpu_percent * 2.0
                + (a.working_set_bytes as f64 / (1024.0 * 1024.0 * 512.0));
            let score_b = b.cpu_percent * 3.0
                + b.gpu_percent * 2.0
                + (b.working_set_bytes as f64 / (1024.0 * 1024.0 * 512.0));
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(6);

        let mut recommendations = Vec::new();
        if !protected_running.is_empty() {
            recommendations.push(format!(
                "Protected streaming/audio workloads detected ({}); GameFocus is preserving their scheduling and memory stability.",
                protected_running.join(", ")
            ));
        }
        for item in &candidates {
            if item.is_protected {
                continue;
            }
            if item.cpu_percent >= 5.0 {
                recommendations.push(format!(
                    "Background application '{}' (PID {}) is using {:.1}% CPU time while the session is active.",
                    item.executable_name, item.pid, item.cpu_percent
                ));
            } else if item.gpu_percent >= 5.0 {
                recommendations.push(format!(
                    "Background application '{}' (PID {}) is using {:.1}% GPU engine capacity.",
                    item.executable_name, item.pid, item.gpu_percent
                ));
            } else if item.working_set_bytes >= 750 * 1024 * 1024 {
                recommendations.push(format!(
                    "Background application '{}' (PID {}) holds {:.0} MiB of physical working set.",
                    item.executable_name,
                    item.pid,
                    item.working_set_bytes as f64 / 1024.0 / 1024.0
                ));
            }
        }

        (protected_running, candidates, recommendations)
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::{CompetingProcessInfo, WorkloadAnalyzer};
    use std::collections::HashMap;

    pub fn sample_workloads(
        _analyzer: &mut WorkloadAnalyzer,
        _active_game_pid: Option<u32>,
        _custom_protected: &[String],
        _per_process_gpu: &HashMap<u32, f64>,
    ) -> (Vec<String>, Vec<CompetingProcessInfo>, Vec<String>) {
        (Vec::new(), Vec::new(), Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_game_blocklist_rejects_browsers_launchers_and_system_processes() {
        assert!(is_non_game_executable(
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"
        ));
        assert!(is_non_game_executable(
            "C:\\Program Files\\obs-studio\\bin\\64bit\\obs64.exe"
        ));
        assert!(is_non_game_executable(
            "C:\\Program Files (x86)\\Steam\\steam.exe"
        ));
        assert!(is_non_game_executable("C:\\Windows\\System32\\svchost.exe"));
        assert!(is_non_game_executable("memflux.exe"));
        assert!(!is_non_game_executable(
            "C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe"
        ));
    }

    #[test]
    fn game_profile_matches_filename_and_full_path_accurately() {
        let profile = GameProfile::from_preset(
            "p1".into(),
            "Elden Ring".into(),
            "eldenring.exe".into(),
            GamePreset::PerformanceGaming,
        );
        assert!(profile.matches_executable(
            "C:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe"
        ));
        assert!(!profile.matches_executable("C:\\Games\\not_eldenring.exe"));
    }

    #[test]
    fn streaming_protection_identifies_obs_and_custom_apps() {
        assert!(is_protected_workload("obs64.exe", &[]));
        assert!(is_protected_workload(
            "C:\\Program Files\\Discord\\Discord.exe",
            &[]
        ));
        assert!(is_protected_workload(
            "customcapture.exe",
            &["customcapture.exe".to_string()]
        ));
        assert!(!is_protected_workload("randomapp.exe", &[]));
    }
}
