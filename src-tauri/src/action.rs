use serde::Serialize;
use std::time::Instant;

pub const OPERATION_PURGE_STANDBY: &str = "purge_standby_list";
const HELPER_ARGUMENT: &str = "--memflux-elevated-purge-standby";
const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;
const STATUS_PRIVILEGE_NOT_HELD: u32 = 0xC000_0061;
const SUPPORTED_BUILDS: &[u32] = &[26340];

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
    pub mechanism: String,
    pub explanation: String,
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
    HelperLaunchFailed,
    HelperHandleUnavailable,
    HelperWaitTimedOut,
    HelperStillRunning,
    HelperWaitFailed,
    HelperExitCodeFailed,
    Unsupported,
}

pub fn capability() -> ActionCapability {
    platform::capability()
}

pub fn execute_standby_purge() -> NativeActionResult {
    platform::execute_standby_purge()
}

pub fn classify_outcome(native: &NativeActionResult, reduction_bytes: Option<f64>) -> &'static str {
    if native.lifecycle == NativeActionLifecycle::ElevationCancelled {
        "elevation_cancelled"
    } else if native.lifecycle == NativeActionLifecycle::HelperWaitTimedOut {
        "helper_wait_timed_out"
    } else if native.lifecycle == NativeActionLifecycle::HelperStillRunning {
        "skipped_helper_pending"
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

pub fn elevated_helper_exit_code() -> Option<u32> {
    if std::env::args_os().count() != 2
        || std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(HELPER_ARGUMENT))
    {
        return None;
    }
    Some(platform::run_helper())
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_CANCELLED, ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LUID,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::Security::{
        AdjustTokenPrivileges, GetTokenInformation, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
        SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_ELEVATION, TOKEN_PRIVILEGES,
        TOKEN_QUERY, TokenElevation,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::core::{PCSTR, PCWSTR};

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

    // Process handles can be waited on and closed from different threads. Access
    // is serialized by PENDING_HELPER; ownership is never duplicated.
    unsafe impl Send for OwnedHandle {}
    static PENDING_HELPER: Mutex<Option<OwnedHandle>> = Mutex::new(None);
    static UNOBSERVABLE_HELPER: AtomicBool = AtomicBool::new(false);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                let _ = unsafe { CloseHandle(self.0) };
            }
        }
    }

    pub fn capability() -> ActionCapability {
        let build = os_build();
        let export_present = native_export_present();
        let build_supported = build.is_some_and(|value| SUPPORTED_BUILDS.contains(&value));
        let elevated = is_elevated().unwrap_or(false);

        if !export_present {
            return unsupported(
                build,
                "mechanism_unavailable",
                "ntdll.dll does not export NtSetSystemInformation on this configuration.",
            );
        }
        if !build_supported {
            return unsupported(
                build,
                "unverified",
                "This Windows build is outside MemFlux's validated standby-purge allowlist. Monitoring remains available.",
            );
        }

        ActionCapability {
            status: if elevated {
                "ready"
            } else {
                "requires_elevation"
            }
            .into(),
            compatibility_status: "confirmed_supported".into(),
            supported: true,
            requires_elevation: !elevated,
            privilege_state: if elevated {
                "elevated_not_yet_enabled"
            } else {
                "elevation_required"
            }
            .into(),
            mechanism_available: true,
            os_build: build,
            mechanism:
                "NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)"
                    .into(),
            explanation: if elevated {
                "Validated native standby purge is ready in this elevated process.".into()
            } else {
                "Validated native standby purge is available. Windows will request administrator approval for the narrow one-shot helper.".into()
            },
        }
    }

    fn unsupported(
        build: Option<u32>,
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
            mechanism: "NtSetSystemInformation/SystemMemoryListInformation".into(),
            explanation: explanation.into(),
        }
    }

    pub fn execute_standby_purge() -> NativeActionResult {
        let capability = capability();
        if !capability.supported {
            return NativeActionResult {
                lifecycle: NativeActionLifecycle::Unsupported,
                nt_status: None,
                request_succeeded: false,
                elevation_used: false,
                late_completion_possible: false,
                duration_ms: 0,
                detail: capability.explanation,
            };
        }

        let started = Instant::now();
        let already_elevated = is_elevated().unwrap_or(false);
        let execution = if already_elevated {
            let status = perform_native_purge();
            HelperExecution::NativeCompleted(status)
        } else {
            launch_elevated_helper()
        };
        execution.into_result(started, already_elevated)
    }

    pub fn run_helper() -> u32 {
        if !capability().supported {
            return 0xC000_00BB; // STATUS_NOT_SUPPORTED
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
            0xC000_00BB => "This Windows configuration reported STATUS_NOT_SUPPORTED.".into(),
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
            mechanism: "Windows native memory-list API".into(),
            explanation: "Standby-list purging is supported only on validated Windows builds."
                .into(),
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

    pub fn run_helper() -> u32 {
        0xC000_00BB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_mode_requires_exactly_one_fixed_argument() {
        assert_eq!(HELPER_ARGUMENT, "--memflux-elevated-purge-standby");
        assert_eq!(SYSTEM_MEMORY_LIST_INFORMATION, 80);
        assert_eq!(MEMORY_PURGE_STANDBY_LIST, 4);
    }

    #[test]
    fn capability_never_claims_an_unlisted_build() {
        let capability = capability();
        if capability.supported {
            assert!(SUPPORTED_BUILDS.contains(&capability.os_build.expect("supported build")));
        }
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
