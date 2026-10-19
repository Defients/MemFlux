use serde::{Deserialize, Serialize};
use std::time::Instant;

pub const OPERATION_PURGE_STANDBY: &str = "purge_standby_list";
const HELPER_ARGUMENT: &str = "--memflux-elevated-purge-standby";
const SESSION_HELPER_ARGUMENT: &str = "--memflux-elevated-session-helper";
const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;
const STATUS_PRIVILEGE_NOT_HELD: u32 = 0xC000_0061;
const STATUS_ACCESS_DENIED: u32 = 0xC000_0022;
const STATUS_NOT_SUPPORTED: u32 = 0xC000_00BB;
pub const SUPPORTED_BUILDS: &[u32] = &[26340];
pub const SESSION_PROTOCOL_VERSION: u32 = 1;
const MAX_IPC_MESSAGE_BYTES: usize = 2048;
const SESSION_MAX_LIFETIME_MS: u32 = 4 * 60 * 60 * 1_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityEntry {
    pub windows_edition: String,
    pub build: u32,
    pub revision: Option<u32>,
    pub architecture: String,
    pub security_configuration: String,
    pub privilege_context: String,
    pub native_mechanism_available: bool,
    pub native_request_result: String,
    pub observed_memory_behavior: String,
    pub error_handling: String,
    pub test_date: String,
    pub test_method: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCapability {
    pub status: String,
    pub compatibility_status: String,
    pub supported: bool,
    pub requires_elevation: bool,
    pub privilege_state: String,
    pub mechanism_available: bool,
    pub os_build: Option<u32>,
    pub os_revision: Option<u32>,
    pub os_edition: Option<String>,
    pub architecture: String,
    pub mechanism: String,
    pub explanation: String,
    pub compatibility_matrix: Vec<CompatibilityEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanerSessionStatus {
    pub authorized: bool,
    pub mode: String,
    pub helper_pid: Option<u32>,
    pub session_id: Option<String>,
    pub started_timestamp_ms: Option<u64>,
    pub expires_timestamp_ms: Option<u64>,
    pub detail: String,
}

impl Default for CleanerSessionStatus {
    fn default() -> Self {
        Self {
            authorized: false,
            mode: "unauthorized".into(),
            helper_pid: None,
            session_id: None,
            started_timestamp_ms: None,
            expires_timestamp_ms: None,
            detail: "Automatic standby cleaning is not authorized for this session. Manual UAC authorization is required to enable unattended cleaning.".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOperation {
    Ping,
    PurgeStandbyList,
    Shutdown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionIpcRequest {
    pub protocol_version: u32,
    pub session_id: String,
    pub request_id: u64,
    pub operation: SessionOperation,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionIpcResponse {
    pub protocol_version: u32,
    pub session_id: String,
    pub request_id: u64,
    pub accepted: bool,
    pub nt_status: Option<u32>,
    pub helper_pid: u32,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct NativeActionResult {
    pub lifecycle: NativeActionLifecycle,
    pub nt_status: Option<u32>,
    pub request_succeeded: bool,
    pub elevation_used: bool,
    pub late_completion_possible: bool,
    pub duration_ms: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeActionLifecycle {
    NativeCompleted,
    ElevationCancelled,
    AuthorizationRequired,
    HelperLaunchFailed,
    HelperHandleUnavailable,
    HelperWaitTimedOut,
    HelperStillRunning,
    HelperWaitFailed,
    HelperExitCodeFailed,
    SessionIpcFailed,
    Unsupported,
}

pub fn capability() -> ActionCapability {
    platform::capability()
}

pub fn execute_standby_purge() -> NativeActionResult {
    platform::execute_standby_purge()
}

pub fn execute_automatic_standby_purge() -> NativeActionResult {
    platform::execute_automatic_standby_purge()
}

pub fn authorize_cleaner_session() -> CleanerSessionStatus {
    platform::authorize_cleaner_session()
}

pub fn revoke_cleaner_session() -> CleanerSessionStatus {
    platform::revoke_cleaner_session()
}

pub fn cleaner_session_status() -> CleanerSessionStatus {
    platform::cleaner_session_status()
}

pub fn compatibility_matrix() -> Vec<CompatibilityEntry> {
    vec![
        CompatibilityEntry {
            windows_edition: "Windows 11 Home Insider Preview".into(),
            build: 26340,
            revision: Some(9750),
            architecture: "x86_64".into(),
            security_configuration: "Standard UAC filtered token; HVCI/Defender default".into(),
            privilege_context: "Unelevated monitor + SeProfileSingleProcessPrivilege in helper".into(),
            native_mechanism_available: true,
            native_request_result: "STATUS_SUCCESS (0x00000000) elevated; STATUS_PRIVILEGE_NOT_HELD (0xC0000061) unelevated".into(),
            observed_memory_behavior: "Standby 233,820,160 -> 25,538,560 bytes; Free & Zero 10,254,319,616 -> 10,510,528,512 bytes after 750 ms".into(),
            error_handling: "Verified privilege denial, timeout isolation, and duplicate-launch guard".into(),
            test_date: "2026-10-18".into(),
            test_method: "Controlled native helper invocation with PDH pre/post verification".into(),
            status: "confirmed_supported".into(),
        },
        CompatibilityEntry {
            windows_edition: "Windows 11 24H2 (Stable)".into(),
            build: 26100,
            revision: None,
            architecture: "x86_64".into(),
            security_configuration: "Standard UAC".into(),
            privilege_context: "SeProfileSingleProcessPrivilege required".into(),
            native_mechanism_available: true,
            native_request_result: "Pending controlled host runtime test".into(),
            observed_memory_behavior: "Unverified on physical/VM 26100 target".into(),
            error_handling: "Action gated off by allowlist until runtime evidence is recorded".into(),
            test_date: "Pending".into(),
            test_method: "Awaiting representative Windows 11 24H2 test host".into(),
            status: "unverified".into(),
        },
        CompatibilityEntry {
            windows_edition: "Windows 11 23H2 / 22H2 (Stable)".into(),
            build: 22631,
            revision: None,
            architecture: "x86_64".into(),
            security_configuration: "Standard UAC".into(),
            privilege_context: "SeProfileSingleProcessPrivilege required".into(),
            native_mechanism_available: true,
            native_request_result: "Pending controlled host runtime test".into(),
            observed_memory_behavior: "Unverified on physical/VM 22631 target".into(),
            error_handling: "Action gated off by allowlist until runtime evidence is recorded".into(),
            test_date: "Pending".into(),
            test_method: "Awaiting representative Windows 11 23H2 test host".into(),
            status: "unverified".into(),
        },
        CompatibilityEntry {
            windows_edition: "Windows 10 22H2 (Stable)".into(),
            build: 19045,
            revision: None,
            architecture: "x86_64".into(),
            security_configuration: "Standard UAC".into(),
            privilege_context: "SeProfileSingleProcessPrivilege required".into(),
            native_mechanism_available: true,
            native_request_result: "Pending controlled host runtime test".into(),
            observed_memory_behavior: "Unverified on physical/VM 19045 target".into(),
            error_handling: "Action gated off by allowlist until runtime evidence is recorded".into(),
            test_date: "Pending".into(),
            test_method: "Awaiting representative Windows 10 22H2 test host".into(),
            status: "unverified".into(),
        },
    ]
}

pub fn classify_outcome(native: &NativeActionResult, reduction_bytes: Option<f64>) -> &'static str {
    if native.lifecycle == NativeActionLifecycle::ElevationCancelled {
        "elevation_cancelled"
    } else if native.lifecycle == NativeActionLifecycle::AuthorizationRequired {
        "authorization_required"
    } else if native.lifecycle == NativeActionLifecycle::HelperWaitTimedOut {
        "helper_wait_timed_out"
    } else if native.lifecycle == NativeActionLifecycle::HelperStillRunning {
        "skipped_helper_pending"
    } else if native.lifecycle == NativeActionLifecycle::SessionIpcFailed {
        "session_ipc_failed"
    } else if native.lifecycle != NativeActionLifecycle::NativeCompleted {
        "helper_failed"
    } else if !native.request_succeeded {
        "native_failed"
    } else if reduction_bytes.is_none() {
        "request_succeeded_measurement_unavailable"
    } else if reduction_bytes.is_some_and(|bytes| bytes >= 1024.0 * 1024.0) {
        "request_succeeded_reduction_observed"
    } else {
        "request_succeeded_no_measurable_reduction"
    }
}

pub fn validate_session_helper_args(
    controller_pid_str: &str,
    pipe_name: &str,
    session_id: &str,
) -> Option<(u32, String, String)> {
    let controller_pid = controller_pid_str.parse::<u32>().ok()?;
    if controller_pid == 0 {
        return None;
    }
    let suffix = pipe_name.strip_prefix(r"\\.\pipe\MemFluxSession_")?;
    if suffix.is_empty()
        || suffix.len() > 96
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    if session_id.len() < 16
        || session_id.len() > 64
        || !session_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    Some((controller_pid, pipe_name.to_owned(), session_id.to_owned()))
}

pub fn elevated_helper_exit_code() -> Option<u32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() == 2 && args[1].as_os_str() == std::ffi::OsStr::new(HELPER_ARGUMENT) {
        return Some(platform::run_helper());
    }
    if args.len() == 5 && args[1].as_os_str() == std::ffi::OsStr::new(SESSION_HELPER_ARGUMENT) {
        let pid_str = args[2].to_str()?;
        let pipe_name = args[3].to_str()?;
        let session_id = args[4].to_str()?;
        let (controller_pid, pipe_name, session_id) =
            validate_session_helper_args(pid_str, pipe_name, session_id)?;
        return Some(platform::run_session_helper(
            controller_pid,
            &pipe_name,
            &session_id,
        ));
    }
    None
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_CANCELLED, ERROR_NOT_ALL_ASSIGNED, ERROR_PIPE_CONNECTED, GetLastError,
        HANDLE, HLOCAL, LUID, LocalFree, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        AdjustTokenPrivileges, GetTokenInformation, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
        PSECURITY_DESCRIPTOR, SE_PRIVILEGE_ENABLED, SECURITY_ATTRIBUTES, TOKEN_ADJUST_PRIVILEGES,
        TOKEN_ELEVATION, TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_GENERIC_READ,
        FILE_GENERIC_WRITE, FILE_SHARE_NONE, FlushFileBuffers, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
        ReadFile, WriteFile,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
        GetNamedPipeServerProcessId, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_MESSAGE, PIPE_WAIT, WaitNamedPipeW,
    };
    use windows::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, GetProcessId, OpenProcess,
        OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::core::{PCSTR, PCWSTR, PWSTR};

    #[repr(C)]
    struct RtlOsVersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform_id: u32,
        service_pack: [u16; 128],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtSetSystemInformation(
            class: u32,
            information: *const core::ffi::c_void,
            length: u32,
        ) -> i32;
        fn RtlGetVersion(version: *mut RtlOsVersionInfo) -> i32;
    }

    struct OwnedHandle(HANDLE);

    unsafe impl Send for OwnedHandle {}
    static PENDING_HELPER: Mutex<Option<OwnedHandle>> = Mutex::new(None);
    static UNOBSERVABLE_HELPER: AtomicBool = AtomicBool::new(false);
    static SESSION_REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

    struct ActiveSession {
        process: OwnedHandle,
        helper_pid: u32,
        pipe_name: String,
        session_id: String,
        started_timestamp_ms: u64,
        expires_timestamp_ms: u64,
        started_instant: Instant,
    }

    static ACTIVE_SESSION: Mutex<Option<ActiveSession>> = Mutex::new(None);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                let _ = unsafe { CloseHandle(self.0) };
            }
        }
    }

    pub fn capability() -> ActionCapability {
        let build = os_build();
        let (edition, revision) = os_registry_details();
        let export_present = native_export_present();
        let build_supported = build.is_some_and(|value| SUPPORTED_BUILDS.contains(&value));
        let elevated = is_elevated().unwrap_or(false);
        let session = cleaner_session_status();

        if !export_present {
            return unsupported(
                build,
                revision,
                edition,
                "mechanism_unavailable",
                "ntdll.dll does not export NtSetSystemInformation on this configuration.",
            );
        }
        if !build_supported {
            return unsupported(
                build,
                revision,
                edition,
                "unverified",
                "This Windows build is outside MemFlux's validated standby-purge allowlist. Monitoring remains available while native cache purging stays disabled until build qualification is recorded.",
            );
        }

        let status = if elevated || session.authorized {
            "ready"
        } else {
            "requires_elevation"
        };

        ActionCapability {
            status: status.into(),
            compatibility_status: "confirmed_supported".into(),
            supported: true,
            requires_elevation: !elevated && !session.authorized,
            privilege_state: if elevated {
                "elevated_in_process".into()
            } else if session.authorized {
                "authorized_session_helper".into()
            } else {
                "elevation_required".into()
            },
            mechanism_available: true,
            os_build: build,
            os_revision: revision,
            os_edition: edition,
            architecture: std::env::consts::ARCH.into(),
            mechanism:
                "NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)"
                    .into(),
            explanation: if elevated {
                "Validated native standby purge is ready in this elevated process.".into()
            } else if session.authorized {
                "Validated native standby purge is ready via the authenticated elevated session helper.".into()
            } else {
                "Validated native standby purge is available. Windows will request administrator approval for the narrow helper.".into()
            },
            compatibility_matrix: compatibility_matrix(),
        }
    }

    fn unsupported(
        build: Option<u32>,
        revision: Option<u32>,
        edition: Option<String>,
        compatibility_status: &str,
        explanation: &str,
    ) -> ActionCapability {
        ActionCapability {
            status: compatibility_status.into(),
            compatibility_status: compatibility_status.into(),
            supported: false,
            requires_elevation: false,
            privilege_state: "not_evaluated".into(),
            mechanism_available: compatibility_status != "mechanism_unavailable",
            os_build: build,
            os_revision: revision,
            os_edition: edition,
            architecture: std::env::consts::ARCH.into(),
            mechanism: "NtSetSystemInformation/SystemMemoryListInformation".into(),
            explanation: explanation.into(),
            compatibility_matrix: compatibility_matrix(),
        }
    }

    pub fn execute_standby_purge() -> NativeActionResult {
        let cap = capability();
        if !cap.supported {
            return NativeActionResult {
                lifecycle: NativeActionLifecycle::Unsupported,
                nt_status: None,
                request_succeeded: false,
                elevation_used: false,
                late_completion_possible: false,
                duration_ms: 0,
                detail: cap.explanation,
            };
        }

        let started = Instant::now();
        let already_elevated = is_elevated().unwrap_or(false);
        if already_elevated {
            let status = perform_native_purge();
            return HelperExecution::NativeCompleted(status).into_result(started, true);
        }

        // If an authenticated session helper is active, use it directly without a new UAC prompt
        if let Some(result) = try_execute_via_active_session(started) {
            return result;
        }

        let execution = launch_elevated_helper();
        execution.into_result(started, false)
    }

    pub fn execute_automatic_standby_purge() -> NativeActionResult {
        let cap = capability();
        if !cap.supported {
            return NativeActionResult {
                lifecycle: NativeActionLifecycle::Unsupported,
                nt_status: None,
                request_succeeded: false,
                elevation_used: false,
                late_completion_possible: false,
                duration_ms: 0,
                detail: cap.explanation,
            };
        }

        let started = Instant::now();
        if is_elevated().unwrap_or(false) {
            let status = perform_native_purge();
            return HelperExecution::NativeCompleted(status).into_result(started, true);
        }

        if let Some(result) = try_execute_via_active_session(started) {
            return result;
        }

        NativeActionResult {
            lifecycle: NativeActionLifecycle::AuthorizationRequired,
            nt_status: None,
            request_succeeded: false,
            elevation_used: false,
            late_completion_possible: false,
            duration_ms: started.elapsed().as_millis() as u64,
            detail: "Automatic standby cleaning requires an explicitly authorized session helper; MemFlux never triggers background UAC prompts.".into(),
        }
    }

    pub fn cleaner_session_status() -> CleanerSessionStatus {
        if !capability_supported_only() {
            return CleanerSessionStatus {
                authorized: false,
                mode: "unsupported".into(),
                helper_pid: None,
                session_id: None,
                started_timestamp_ms: None,
                expires_timestamp_ms: None,
                detail: "Automatic cleaning is unavailable because this Windows build is not in the validated allowlist.".into(),
            };
        }
        if is_elevated().unwrap_or(false) {
            return CleanerSessionStatus {
                authorized: true,
                mode: "in_process_elevated".into(),
                helper_pid: Some(unsafe { GetCurrentProcessId() }),
                session_id: Some("in-process".into()),
                started_timestamp_ms: None,
                expires_timestamp_ms: None,
                detail: "Process is already elevated; automatic cleaning is authorized.".into(),
            };
        }
        let mut guard = ACTIVE_SESSION
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(session) = guard.as_ref() {
            let signaled = unsafe { WaitForSingleObject(session.process.0, 0) } == WAIT_OBJECT_0;
            let expired =
                session.started_instant.elapsed().as_millis() >= SESSION_MAX_LIFETIME_MS as u128;
            if !signaled && !expired {
                return CleanerSessionStatus {
                    authorized: true,
                    mode: "authenticated_named_pipe_helper".into(),
                    helper_pid: Some(session.helper_pid),
                    session_id: Some(session.session_id.clone()),
                    started_timestamp_ms: Some(session.started_timestamp_ms),
                    expires_timestamp_ms: Some(session.expires_timestamp_ms),
                    detail: format!(
                        "Authenticated elevated session helper active (PID {}). Only fixed memory-list purge operations are accepted.",
                        session.helper_pid
                    ),
                };
            }
            *guard = None;
        }
        CleanerSessionStatus::default()
    }

    pub fn authorize_cleaner_session() -> CleanerSessionStatus {
        if !capability_supported_only() {
            return cleaner_session_status();
        }
        if is_elevated().unwrap_or(false) {
            return cleaner_session_status();
        }
        let current_status = cleaner_session_status();
        if current_status.authorized {
            return current_status;
        }

        let controller_pid = unsafe { GetCurrentProcessId() };
        let now_ms = crate::telemetry::timestamp_ms();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(now_ms as u128);
        let session_id = format!("{controller_pid:08x}{nonce:024x}");
        let pipe_name = format!(r"\\.\pipe\MemFluxSession_{controller_pid}_{now_ms}");

        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                return CleanerSessionStatus {
                    authorized: false,
                    mode: "error".into(),
                    helper_pid: None,
                    session_id: None,
                    started_timestamp_ms: None,
                    expires_timestamp_ms: None,
                    detail: format!("Could not resolve MemFlux executable path: {error}"),
                };
            }
        };

        let file: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
        let params_text =
            format!("{SESSION_HELPER_ARGUMENT} {controller_pid} {pipe_name} {session_id}");
        let parameters: Vec<u16> = params_text.encode_utf16().chain(Some(0)).collect();

        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: 0,
            ..unsafe { zeroed() }
        };

        if let Err(error) = unsafe { ShellExecuteExW(&mut info) } {
            let cancelled = error.code().0 as u32 == (0x8007_0000 | ERROR_CANCELLED.0);
            return CleanerSessionStatus {
                authorized: false,
                mode: if cancelled { "cancelled" } else { "error" }.into(),
                helper_pid: None,
                session_id: None,
                started_timestamp_ms: None,
                expires_timestamp_ms: None,
                detail: if cancelled {
                    "Administrator approval for the session helper was declined.".into()
                } else {
                    format!("Could not launch elevated session helper: {error}")
                },
            };
        }

        let process = OwnedHandle(info.hProcess);
        if process.0.is_invalid() {
            return CleanerSessionStatus {
                authorized: false,
                mode: "error".into(),
                helper_pid: None,
                session_id: None,
                started_timestamp_ms: None,
                expires_timestamp_ms: None,
                detail: "Windows did not return a process handle for the elevated session helper."
                    .into(),
            };
        }
        let helper_pid = unsafe { GetProcessId(process.0) };

        // Perform authenticated Ping handshake to verify pipe security and peer identity
        match send_session_request(
            &pipe_name,
            &session_id,
            helper_pid,
            SessionOperation::Ping,
            5_000,
        ) {
            Ok(response) if response.accepted && response.helper_pid == helper_pid => {
                let expires_ms = now_ms.saturating_add(SESSION_MAX_LIFETIME_MS as u64);
                let mut guard = ACTIVE_SESSION
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                *guard = Some(ActiveSession {
                    process,
                    helper_pid,
                    pipe_name,
                    session_id: session_id.clone(),
                    started_timestamp_ms: now_ms,
                    expires_timestamp_ms: expires_ms,
                    started_instant: Instant::now(),
                });
                CleanerSessionStatus {
                    authorized: true,
                    mode: "authenticated_named_pipe_helper".into(),
                    helper_pid: Some(helper_pid),
                    session_id: Some(session_id),
                    started_timestamp_ms: Some(now_ms),
                    expires_timestamp_ms: Some(expires_ms),
                    detail: format!(
                        "Authenticated elevated session helper active (PID {helper_pid})."
                    ),
                }
            }
            Ok(response) => CleanerSessionStatus {
                authorized: false,
                mode: "error".into(),
                helper_pid: None,
                session_id: None,
                started_timestamp_ms: None,
                expires_timestamp_ms: None,
                detail: format!("Session helper handshake rejected: {}", response.detail),
            },
            Err(error) => CleanerSessionStatus {
                authorized: false,
                mode: "error".into(),
                helper_pid: None,
                session_id: None,
                started_timestamp_ms: None,
                expires_timestamp_ms: None,
                detail: format!("Session helper IPC handshake failed: {error}"),
            },
        }
    }

    pub fn revoke_cleaner_session() -> CleanerSessionStatus {
        let existing = {
            let mut guard = ACTIVE_SESSION
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            guard.take()
        };
        if let Some(session) = existing {
            let _ = send_session_request(
                &session.pipe_name,
                &session.session_id,
                session.helper_pid,
                SessionOperation::Shutdown,
                2_000,
            );
            let _ = unsafe { WaitForSingleObject(session.process.0, 2_000) };
        }
        CleanerSessionStatus {
            authorized: false,
            mode: "unauthorized".into(),
            helper_pid: None,
            session_id: None,
            started_timestamp_ms: None,
            expires_timestamp_ms: None,
            detail: "Elevated cleaning session was revoked and shut down.".into(),
        }
    }

    fn try_execute_via_active_session(started: Instant) -> Option<NativeActionResult> {
        let (pipe_name, session_id, helper_pid) = {
            let mut guard = ACTIVE_SESSION
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let session = guard.as_ref()?;
            let signaled = unsafe { WaitForSingleObject(session.process.0, 0) } == WAIT_OBJECT_0;
            let expired =
                session.started_instant.elapsed().as_millis() >= SESSION_MAX_LIFETIME_MS as u128;
            if signaled || expired {
                *guard = None;
                return None;
            }
            (
                session.pipe_name.clone(),
                session.session_id.clone(),
                session.helper_pid,
            )
        };

        match send_session_request(
            &pipe_name,
            &session_id,
            helper_pid,
            SessionOperation::PurgeStandbyList,
            15_000,
        ) {
            Ok(response) => {
                let status = response.nt_status.unwrap_or(STATUS_NOT_SUPPORTED);
                Some(NativeActionResult {
                    lifecycle: NativeActionLifecycle::NativeCompleted,
                    nt_status: Some(status),
                    request_succeeded: response.accepted && nt_success(status),
                    elevation_used: true,
                    late_completion_possible: false,
                    duration_ms: started.elapsed().as_millis() as u64,
                    detail: response.detail,
                })
            }
            Err(error) => {
                let mut guard = ACTIVE_SESSION
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                *guard = None;
                Some(NativeActionResult {
                    lifecycle: NativeActionLifecycle::SessionIpcFailed,
                    nt_status: None,
                    request_succeeded: false,
                    elevation_used: true,
                    late_completion_possible: false,
                    duration_ms: started.elapsed().as_millis() as u64,
                    detail: format!("Session helper IPC failed and session was closed: {error}"),
                })
            }
        }
    }

    pub(super) fn send_session_request(
        pipe_name: &str,
        session_id: &str,
        expected_server_pid: u32,
        operation: SessionOperation,
        timeout_ms: u32,
    ) -> Result<SessionIpcResponse, String> {
        let pipe_wide: Vec<u16> = pipe_name.encode_utf16().chain(Some(0)).collect();
        let deadline = Instant::now() + std::time::Duration::from_millis(timeout_ms as u64);

        let raw_handle = loop {
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(pipe_wide.as_ptr()),
                    (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            };
            match handle {
                Ok(h) if !h.is_invalid() => break h,
                _ => {
                    if Instant::now() >= deadline {
                        return Err("Timed out connecting to session helper named pipe.".into());
                    }
                    let _ = unsafe { WaitNamedPipeW(PCWSTR(pipe_wide.as_ptr()), 100) };
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
        };
        let pipe = OwnedHandle(raw_handle);

        // Verify server process identity
        let mut server_pid = 0_u32;
        unsafe { GetNamedPipeServerProcessId(pipe.0, &mut server_pid) }
            .map_err(|e| format!("GetNamedPipeServerProcessId failed: {e}"))?;
        if server_pid != expected_server_pid {
            return Err(format!(
                "Pipe server PID mismatch: expected {expected_server_pid}, got {server_pid}"
            ));
        }
        verify_peer_executable(server_pid)?;

        let request = SessionIpcRequest {
            protocol_version: SESSION_PROTOCOL_VERSION,
            session_id: session_id.to_owned(),
            request_id: SESSION_REQUEST_SEQ.fetch_add(1, Ordering::Relaxed),
            operation,
        };
        let payload =
            serde_json::to_vec(&request).map_err(|e| format!("Serialize IPC request: {e}"))?;
        if payload.len() > MAX_IPC_MESSAGE_BYTES {
            return Err("IPC request exceeds maximum message size.".into());
        }

        let mut written = 0_u32;
        unsafe { WriteFile(pipe.0, Some(&payload), Some(&mut written), None) }
            .map_err(|e| format!("WriteFile to session pipe failed: {e}"))?;
        let _ = unsafe { FlushFileBuffers(pipe.0) };

        let mut buffer = vec![0_u8; MAX_IPC_MESSAGE_BYTES];
        let mut read = 0_u32;
        unsafe { ReadFile(pipe.0, Some(&mut buffer), Some(&mut read), None) }
            .map_err(|e| format!("ReadFile from session pipe failed: {e}"))?;

        let response: SessionIpcResponse = serde_json::from_slice(&buffer[..read as usize])
            .map_err(|e| format!("Invalid IPC response JSON: {e}"))?;
        if response.protocol_version != SESSION_PROTOCOL_VERSION
            || response.session_id != session_id
            || response.request_id != request.request_id
        {
            return Err("Session IPC response failed protocol/session validation.".into());
        }
        Ok(response)
    }

    pub fn run_session_helper(controller_pid: u32, pipe_name: &str, session_id: &str) -> u32 {
        if !capability_supported_only() {
            return STATUS_NOT_SUPPORTED;
        }
        if verify_peer_executable(controller_pid).is_err() {
            return STATUS_ACCESS_DENIED;
        }
        run_session_helper_loop(controller_pid, pipe_name, session_id, perform_native_purge)
    }

    pub(super) fn run_session_helper_loop(
        controller_pid: u32,
        pipe_name: &str,
        session_id: &str,
        purge_fn: fn() -> u32,
    ) -> u32 {
        // Open SYNCHRONIZE handle to controller so controller crash terminates helper
        let Ok(controller_raw) =
            (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, controller_pid) })
        else {
            return STATUS_ACCESS_DENIED;
        };
        let controller_handle = OwnedHandle(controller_raw);
        let shutdown_flag = std::sync::Arc::new(AtomicBool::new(false));
        let watchdog_flag = shutdown_flag.clone();
        let pipe_name_for_watchdog = pipe_name.to_owned();

        std::thread::spawn(move || {
            let controller_handle = controller_handle;
            let _ = unsafe { WaitForSingleObject(controller_handle.0, SESSION_MAX_LIFETIME_MS) };
            watchdog_flag.store(true, Ordering::SeqCst);
            // Unblock ConnectNamedPipe by briefly opening the pipe if still waiting
            let wide: Vec<u16> = pipe_name_for_watchdog
                .encode_utf16()
                .chain(Some(0))
                .collect();
            if let Ok(dummy) = unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            } {
                let _ = OwnedHandle(dummy);
            }
        });

        let Some(security_descriptor) = build_pipe_security_descriptor() else {
            return STATUS_ACCESS_DENIED;
        };
        let security_attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security_descriptor.0.0,
            bInheritHandle: false.into(),
        };

        let pipe_wide: Vec<u16> = pipe_name.encode_utf16().chain(Some(0)).collect();
        let raw_pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(pipe_wide.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_IPC_MESSAGE_BYTES as u32,
                MAX_IPC_MESSAGE_BYTES as u32,
                5_000,
                Some(&security_attributes),
            )
        };
        if raw_pipe.is_invalid() {
            return STATUS_ACCESS_DENIED;
        }
        let pipe = OwnedHandle(raw_pipe);
        drop(security_descriptor);

        let helper_pid = unsafe { GetCurrentProcessId() };

        while !shutdown_flag.load(Ordering::SeqCst) {
            let connected = match unsafe { ConnectNamedPipe(pipe.0, None) } {
                Ok(()) => true,
                Err(err) => err.code().0 as u32 == (0x8007_0000 | ERROR_PIPE_CONNECTED.0),
            };
            if !connected || shutdown_flag.load(Ordering::SeqCst) {
                let _ = unsafe { DisconnectNamedPipe(pipe.0) };
                break;
            }

            let mut client_pid = 0_u32;
            let valid_client = unsafe { GetNamedPipeClientProcessId(pipe.0, &mut client_pid) }
                .is_ok()
                && client_pid == controller_pid
                && verify_peer_executable(client_pid).is_ok();

            if !valid_client {
                let _ = unsafe { DisconnectNamedPipe(pipe.0) };
                continue;
            }

            let mut buffer = vec![0_u8; MAX_IPC_MESSAGE_BYTES];
            let mut bytes_read = 0_u32;
            let read_ok =
                unsafe { ReadFile(pipe.0, Some(&mut buffer), Some(&mut bytes_read), None) }.is_ok()
                    && bytes_read > 0
                    && (bytes_read as usize) <= MAX_IPC_MESSAGE_BYTES;

            if read_ok {
                let (response, should_exit) = match serde_json::from_slice::<SessionIpcRequest>(
                    &buffer[..bytes_read as usize],
                ) {
                    Ok(req)
                        if req.protocol_version == SESSION_PROTOCOL_VERSION
                            && req.session_id == session_id =>
                    {
                        match req.operation {
                            SessionOperation::Ping => (
                                SessionIpcResponse {
                                    protocol_version: SESSION_PROTOCOL_VERSION,
                                    session_id: session_id.to_owned(),
                                    request_id: req.request_id,
                                    accepted: true,
                                    nt_status: None,
                                    helper_pid,
                                    detail: "Authenticated session helper ready.".into(),
                                },
                                false,
                            ),
                            SessionOperation::PurgeStandbyList => {
                                let status = purge_fn();
                                (
                                    SessionIpcResponse {
                                        protocol_version: SESSION_PROTOCOL_VERSION,
                                        session_id: session_id.to_owned(),
                                        request_id: req.request_id,
                                        accepted: nt_success(status),
                                        nt_status: Some(status),
                                        helper_pid,
                                        detail: status_detail(status),
                                    },
                                    false,
                                )
                            }
                            SessionOperation::Shutdown => (
                                SessionIpcResponse {
                                    protocol_version: SESSION_PROTOCOL_VERSION,
                                    session_id: session_id.to_owned(),
                                    request_id: req.request_id,
                                    accepted: true,
                                    nt_status: None,
                                    helper_pid,
                                    detail: "Session helper shutting down.".into(),
                                },
                                true,
                            ),
                        }
                    }
                    _ => (
                        SessionIpcResponse {
                            protocol_version: SESSION_PROTOCOL_VERSION,
                            session_id: session_id.to_owned(),
                            request_id: 0,
                            accepted: false,
                            nt_status: Some(STATUS_ACCESS_DENIED),
                            helper_pid,
                            detail: "Rejected invalid session helper IPC request.".into(),
                        },
                        false,
                    ),
                };

                if let Ok(bytes) = serde_json::to_vec(&response) {
                    let mut written = 0_u32;
                    let _ = unsafe { WriteFile(pipe.0, Some(&bytes), Some(&mut written), None) };
                    let _ = unsafe { FlushFileBuffers(pipe.0) };
                }
                let _ = unsafe { DisconnectNamedPipe(pipe.0) };
                if should_exit {
                    shutdown_flag.store(true, Ordering::SeqCst);
                    break;
                }
            } else {
                let _ = unsafe { DisconnectNamedPipe(pipe.0) };
            }
        }
        0
    }

    struct OwnedSecurityDescriptor(PSECURITY_DESCRIPTOR);
    impl Drop for OwnedSecurityDescriptor {
        fn drop(&mut self) {
            if !self.0.0.is_null() {
                let _ = unsafe { LocalFree(Some(HLOCAL(self.0.0))) };
            }
        }
    }

    fn build_pipe_security_descriptor() -> Option<OwnedSecurityDescriptor> {
        let sddl = match current_user_sid_string() {
            Some(user_sid) => format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{user_sid})"),
            None => "D:P(A;;GA;;;SY)(A;;GA;;;BA)".to_string(),
        };
        let sddl_wide: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl_wide.as_ptr()),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )
        }
        .ok()?;
        Some(OwnedSecurityDescriptor(sd))
    }

    fn current_user_sid_string() -> Option<String> {
        let mut raw_token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) }.ok()?;
        let token = OwnedHandle(raw_token);
        let mut needed = 0_u32;
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut needed) };
        if needed == 0 {
            return None;
        }
        let mut buffer = vec![0_u8; needed as usize];
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                needed,
                &mut needed,
            )
        }
        .ok()?;
        let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        let mut sid_ptr = PWSTR::null();
        unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid_ptr) }.ok()?;
        let result = unsafe { sid_ptr.to_string() }.ok();
        let _ = unsafe { LocalFree(Some(HLOCAL(sid_ptr.0.cast()))) };
        result
    }

    fn executable_path_for_pid(pid: u32) -> Result<PathBuf, String> {
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|e| format!("OpenProcess({pid}) failed: {e}"))?;
        let handle = OwnedHandle(raw);
        let mut buffer = vec![0_u16; 32_768];
        let mut size = buffer.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle.0,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        }
        .map_err(|e| format!("QueryFullProcessImageNameW({pid}) failed: {e}"))?;
        let text = String::from_utf16(&buffer[..size as usize])
            .map_err(|e| format!("Invalid UTF-16 path: {e}"))?;
        Ok(PathBuf::from(text))
    }

    fn verify_peer_executable(peer_pid: u32) -> Result<(), String> {
        let self_exe =
            std::env::current_exe().map_err(|e| format!("Could not read current_exe: {e}"))?;
        let peer_exe = executable_path_for_pid(peer_pid)?;
        if self_exe
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&peer_exe.as_os_str().to_string_lossy())
        {
            Ok(())
        } else {
            Err(format!(
                "Peer executable mismatch: expected {}, got {}",
                self_exe.display(),
                peer_exe.display()
            ))
        }
    }

    fn capability_supported_only() -> bool {
        native_export_present() && os_build().is_some_and(|b| SUPPORTED_BUILDS.contains(&b))
    }

    pub fn run_helper() -> u32 {
        if !capability_supported_only() {
            return STATUS_NOT_SUPPORTED;
        }
        perform_native_purge()
    }

    fn perform_native_purge() -> u32 {
        if enable_profile_privilege().is_err() {
            return STATUS_PRIVILEGE_NOT_HELD;
        }
        let command = MEMORY_PURGE_STANDBY_LIST;
        unsafe {
            NtSetSystemInformation(
                SYSTEM_MEMORY_LIST_INFORMATION,
                (&command as *const u32).cast(),
                size_of::<u32>() as u32,
            ) as u32
        }
    }

    fn enable_profile_privilege() -> Result<(), String> {
        let mut raw_token = HANDLE::default();
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut raw_token,
            )
        }
        .map_err(|error| format!("OpenProcessToken failed: {error}"))?;
        let token = OwnedHandle(raw_token);

        let mut luid = LUID::default();
        let privilege: Vec<u16> = "SeProfileSingleProcessPrivilege"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe { LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(privilege.as_ptr()), &mut luid) }
            .map_err(|error| format!("LookupPrivilegeValueW failed: {error}"))?;

        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        unsafe {
            windows::Win32::Foundation::SetLastError(windows::Win32::Foundation::WIN32_ERROR(0));
            AdjustTokenPrivileges(token.0, false, Some(&privileges), 0, None, None)
        }
        .map_err(|error| format!("AdjustTokenPrivileges failed: {error}"))?;
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_NOT_ALL_ASSIGNED {
            return Err(
                "The process token does not contain SeProfileSingleProcessPrivilege.".into(),
            );
        }
        Ok(())
    }

    enum HelperExecution {
        NativeCompleted(u32),
        ElevationCancelled,
        LaunchFailed(String),
        HandleUnavailable,
        WaitTimedOut,
        StillRunning,
        WaitFailed(u32),
        ExitCodeFailed(String),
    }

    impl HelperExecution {
        fn into_result(self, started: Instant, already_elevated: bool) -> NativeActionResult {
            let (lifecycle, nt_status, request_succeeded, late_completion_possible, detail) =
                match self {
                    Self::NativeCompleted(status) => (
                        NativeActionLifecycle::NativeCompleted,
                        Some(status),
                        nt_success(status),
                        false,
                        status_detail(status),
                    ),
                    Self::ElevationCancelled => (
                        NativeActionLifecycle::ElevationCancelled,
                        None,
                        false,
                        false,
                        "Administrator approval was declined or cancelled.".into(),
                    ),
                    Self::LaunchFailed(error) => (
                        NativeActionLifecycle::HelperLaunchFailed,
                        None,
                        false,
                        false,
                        error,
                    ),
                    Self::HandleUnavailable => (
                        NativeActionLifecycle::HelperHandleUnavailable,
                        None,
                        false,
                        true,
                        "Windows launched the helper but did not return a process handle; completion cannot be observed.".into(),
                    ),
                    Self::WaitTimedOut => (
                        NativeActionLifecycle::HelperWaitTimedOut,
                        None,
                        false,
                        true,
                        "The elevated helper did not finish within 60 seconds. It was not terminated and may still complete; no native result is available.".into(),
                    ),
                    Self::StillRunning => (
                        NativeActionLifecycle::HelperStillRunning,
                        None,
                        false,
                        true,
                        "A previous helper has not been observed to exit. A duplicate request was blocked; the previous helper may still complete.".into(),
                    ),
                    Self::WaitFailed(code) => (
                        NativeActionLifecycle::HelperWaitFailed,
                        None,
                        false,
                        true,
                        format!("Waiting for the elevated helper failed with Win32 wait result 0x{code:08X}; the helper may still complete."),
                    ),
                    Self::ExitCodeFailed(error) => (
                        NativeActionLifecycle::HelperExitCodeFailed,
                        None,
                        false,
                        false,
                        error,
                    ),
                };
            NativeActionResult {
                lifecycle: lifecycle.clone(),
                nt_status,
                request_succeeded,
                elevation_used: !already_elevated
                    && !matches!(
                        lifecycle,
                        NativeActionLifecycle::ElevationCancelled
                            | NativeActionLifecycle::HelperLaunchFailed
                    ),
                late_completion_possible,
                duration_ms: started.elapsed().as_millis() as u64,
                detail,
            }
        }
    }

    fn launch_elevated_helper() -> HelperExecution {
        if UNOBSERVABLE_HELPER.load(Ordering::Acquire) {
            return HelperExecution::StillRunning;
        }
        let mut pending = PENDING_HELPER
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(process) = pending.as_ref() {
            if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_OBJECT_0 {
                return HelperExecution::StillRunning;
            }
            let mut status = 0;
            if unsafe { GetExitCodeProcess(process.0, &mut status) }.is_ok() {
                log::info!(
                    "Previous helper completed late: native status 0x{status:08X}; no attributable post-action measurement is available."
                );
            }
            *pending = None;
        }
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                return HelperExecution::LaunchFailed(format!(
                    "Could not resolve the MemFlux executable: {error}"
                ));
            }
        };
        let file: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
        let parameters: Vec<u16> = HELPER_ARGUMENT.encode_utf16().chain(Some(0)).collect();
        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: 0,
            ..unsafe { zeroed() }
        };
        if let Err(error) = unsafe { ShellExecuteExW(&mut info) } {
            let cancelled = error.code().0 as u32 == (0x8007_0000 | ERROR_CANCELLED.0);
            return if cancelled {
                HelperExecution::ElevationCancelled
            } else {
                HelperExecution::LaunchFailed(format!(
                    "Could not launch the elevated helper: {error}"
                ))
            };
        }
        let process = OwnedHandle(info.hProcess);
        if process.0.is_invalid() {
            UNOBSERVABLE_HELPER.store(true, Ordering::Release);
            return HelperExecution::HandleUnavailable;
        }
        let wait = unsafe { WaitForSingleObject(process.0, 60_000) };
        if wait == WAIT_TIMEOUT {
            *pending = Some(process);
            return HelperExecution::WaitTimedOut;
        }
        if wait != WAIT_OBJECT_0 {
            *pending = Some(process);
            return HelperExecution::WaitFailed(wait.0);
        }
        let mut exit_code = 0_u32;
        if let Err(error) = unsafe { GetExitCodeProcess(process.0, &mut exit_code) } {
            return HelperExecution::ExitCodeFailed(format!(
                "Could not read the completed helper result: {error}"
            ));
        }
        HelperExecution::NativeCompleted(exit_code)
    }

    fn is_elevated() -> Result<bool, String> {
        let mut raw_token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) }
            .map_err(|error| error.to_string())?;
        let token = OwnedHandle(raw_token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0_u32;
        unsafe {
            GetTokenInformation(
                token.0,
                TokenElevation,
                Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned,
            )
        }
        .map_err(|error| error.to_string())?;
        Ok(elevation.TokenIsElevated != 0)
    }

    fn os_build() -> Option<u32> {
        let mut version = RtlOsVersionInfo {
            size: size_of::<RtlOsVersionInfo>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform_id: 0,
            service_pack: [0; 128],
        };
        let status = unsafe { RtlGetVersion(&mut version) };
        (status >= 0).then_some(version.build)
    }

    fn os_registry_details() -> (Option<String>, Option<u32>) {
        let subkey: Vec<u16> = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            )
        }
        .is_err()
        {
            return (None, None);
        }

        let edition_name: Vec<u16> = "ProductName".encode_utf16().chain(Some(0)).collect();
        let mut buf = vec![0_u8; 256];
        let mut len = buf.len() as u32;
        let edition = if unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(edition_name.as_ptr()),
                None,
                None,
                Some(buf.as_mut_ptr()),
                Some(&mut len),
            )
        }
        .is_ok()
            && len >= 2
        {
            let words = unsafe {
                std::slice::from_raw_parts(buf.as_ptr() as *const u16, (len as usize) / 2)
            };
            let trimmed = words.split(|&c| c == 0).next().unwrap_or(words);
            String::from_utf16(trimmed).ok()
        } else {
            None
        };

        let ubr_name: Vec<u16> = "UBR".encode_utf16().chain(Some(0)).collect();
        let mut ubr: u32 = 0;
        let mut ubr_len = size_of::<u32>() as u32;
        let revision = if unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(ubr_name.as_ptr()),
                None,
                None,
                Some((&mut ubr as *mut u32).cast()),
                Some(&mut ubr_len),
            )
        }
        .is_ok()
        {
            Some(ubr)
        } else {
            None
        };

        let _ = unsafe { RegCloseKey(hkey) };
        (edition, revision)
    }

    fn native_export_present() -> bool {
        let module = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")) };
        module
            .ok()
            .and_then(|module| unsafe {
                GetProcAddress(module, PCSTR(c"NtSetSystemInformation".as_ptr().cast()))
            })
            .is_some()
    }

    fn nt_success(status: u32) -> bool {
        (status as i32) >= 0
    }

    fn status_detail(status: u32) -> String {
        match status {
            0 => "Windows accepted the standby-list purge request (STATUS_SUCCESS).".into(),
            STATUS_PRIVILEGE_NOT_HELD => "Windows rejected the request because SeProfileSingleProcessPrivilege was not available (STATUS_PRIVILEGE_NOT_HELD).".into(),
            STATUS_NOT_SUPPORTED => "This Windows configuration reported STATUS_NOT_SUPPORTED.".into(),
            0xC000_0120 => "The elevation request was cancelled.".into(),
            _ if nt_success(status) => format!("Windows accepted the native request with NTSTATUS 0x{status:08X}."),
            _ => format!("Windows rejected the native request with NTSTATUS 0x{status:08X}."),
        }
    }

    #[cfg(test)]
    mod lifecycle_tests {
        use super::*;

        #[test]
        fn transport_conversion_does_not_invent_native_status() {
            for execution in [
                HelperExecution::WaitTimedOut,
                HelperExecution::StillRunning,
                HelperExecution::WaitFailed(u32::MAX),
                HelperExecution::ElevationCancelled,
                HelperExecution::ExitCodeFailed("fixture".into()),
            ] {
                let result = execution.into_result(Instant::now(), false);
                assert!(!result.request_succeeded);
                assert_eq!(result.nt_status, None);
            }
            let result = HelperExecution::NativeCompleted(STATUS_PRIVILEGE_NOT_HELD)
                .into_result(Instant::now(), false);
            assert_eq!(result.nt_status, Some(STATUS_PRIVILEGE_NOT_HELD));
            assert!(!result.request_succeeded);
        }

        #[test]
        fn authenticated_named_pipe_session_verifies_peer_and_executes_fixed_operations() {
            let pid = unsafe { GetCurrentProcessId() };
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let pipe_name = format!(r"\\.\pipe\MemFluxSession_{pid}_{nonce}");
            let session_id = format!("{pid:08x}{nonce:024x}");

            let server_pipe = pipe_name.clone();
            let server_session = session_id.clone();
            let handle = std::thread::spawn(move || {
                run_session_helper_loop(pid, &server_pipe, &server_session, || 0)
            });

            let ping =
                send_session_request(&pipe_name, &session_id, pid, SessionOperation::Ping, 3_000)
                    .expect("ping handshake");
            assert!(ping.accepted);
            assert_eq!(ping.helper_pid, pid);

            let purge = send_session_request(
                &pipe_name,
                &session_id,
                pid,
                SessionOperation::PurgeStandbyList,
                3_000,
            )
            .expect("purge command");
            assert!(purge.accepted);
            assert_eq!(purge.nt_status, Some(0));

            let shutdown = send_session_request(
                &pipe_name,
                &session_id,
                pid,
                SessionOperation::Shutdown,
                3_000,
            )
            .expect("shutdown command");
            assert!(shutdown.accepted);

            assert_eq!(handle.join().expect("server thread exit"), 0);
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub fn capability() -> ActionCapability {
        ActionCapability {
            status: "confirmed_unsupported".into(),
            compatibility_status: "confirmed_unsupported".into(),
            supported: false,
            requires_elevation: false,
            privilege_state: "not_available".into(),
            mechanism_available: false,
            os_build: None,
            os_revision: None,
            os_edition: None,
            architecture: std::env::consts::ARCH.into(),
            mechanism: "Windows native memory-list API".into(),
            explanation: "Standby-list purging is supported only on validated Windows builds."
                .into(),
            compatibility_matrix: compatibility_matrix(),
        }
    }

    pub fn execute_standby_purge() -> NativeActionResult {
        NativeActionResult {
            lifecycle: NativeActionLifecycle::Unsupported,
            nt_status: None,
            request_succeeded: false,
            elevation_used: false,
            late_completion_possible: false,
            duration_ms: 0,
            detail: capability().explanation,
        }
    }

    pub fn execute_automatic_standby_purge() -> NativeActionResult {
        execute_standby_purge()
    }

    pub fn authorize_cleaner_session() -> CleanerSessionStatus {
        CleanerSessionStatus::default()
    }

    pub fn revoke_cleaner_session() -> CleanerSessionStatus {
        CleanerSessionStatus::default()
    }

    pub fn cleaner_session_status() -> CleanerSessionStatus {
        CleanerSessionStatus::default()
    }

    pub fn run_helper() -> u32 {
        STATUS_NOT_SUPPORTED
    }

    pub fn run_session_helper(_controller_pid: u32, _pipe_name: &str, _session_id: &str) -> u32 {
        STATUS_NOT_SUPPORTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_mode_requires_exact_arguments() {
        assert_eq!(HELPER_ARGUMENT, "--memflux-elevated-purge-standby");
        assert_eq!(SESSION_HELPER_ARGUMENT, "--memflux-elevated-session-helper");
        assert_eq!(SYSTEM_MEMORY_LIST_INFORMATION, 80);
        assert_eq!(MEMORY_PURGE_STANDBY_LIST, 4);
        assert!(
            validate_session_helper_args(
                "1234",
                r"\\.\pipe\MemFluxSession_1234_99",
                "0123456789abcdef0123456789abcdef"
            )
            .is_some()
        );
        assert!(
            validate_session_helper_args(
                "0",
                r"\\.\pipe\MemFluxSession_1234_99",
                "0123456789abcdef0123456789abcdef"
            )
            .is_none()
        );
        assert!(
            validate_session_helper_args(
                "1234",
                r"\\.\pipe\OtherPipe",
                "0123456789abcdef0123456789abcdef"
            )
            .is_none()
        );
        assert!(
            validate_session_helper_args("1234", r"\\.\pipe\MemFluxSession_1234_99", "not-hex")
                .is_none()
        );
    }

    #[test]
    fn capability_never_claims_an_unlisted_build() {
        let capability = capability();
        if capability.supported {
            assert!(SUPPORTED_BUILDS.contains(&capability.os_build.expect("supported build")));
        }
        assert!(!capability.compatibility_matrix.is_empty());
    }

    #[test]
    fn native_success_and_measured_change_are_classified_separately() {
        let completed = |status| NativeActionResult {
            lifecycle: NativeActionLifecycle::NativeCompleted,
            nt_status: Some(status),
            request_succeeded: (status as i32) >= 0,
            elevation_used: true,
            late_completion_possible: false,
            duration_ms: 1,
            detail: String::new(),
        };
        assert_eq!(
            classify_outcome(&completed(0), Some(32.0 * 1024.0 * 1024.0)),
            "request_succeeded_reduction_observed"
        );
        assert_eq!(
            classify_outcome(&completed(0), Some(0.0)),
            "request_succeeded_no_measurable_reduction"
        );
        assert_eq!(
            classify_outcome(&completed(STATUS_PRIVILEGE_NOT_HELD), None),
            "native_failed"
        );
    }

    #[test]
    fn wait_timeout_can_never_be_native_success() {
        let timeout = NativeActionResult {
            lifecycle: NativeActionLifecycle::HelperWaitTimedOut,
            nt_status: None,
            request_succeeded: false,
            elevation_used: true,
            late_completion_possible: true,
            duration_ms: 60_000,
            detail: String::new(),
        };
        assert_eq!(classify_outcome(&timeout, None), "helper_wait_timed_out");
        assert!(timeout.nt_status.is_none());
        assert!(!timeout.request_succeeded);
    }
}
