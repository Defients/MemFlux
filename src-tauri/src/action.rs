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
    pub supported: bool,
    pub requires_elevation: bool,
    pub os_build: Option<u32>,
    pub mechanism: String,
    pub explanation: String,
}

#[derive(Clone, Debug)]
pub struct NativeActionResult {
    pub nt_status: u32,
    pub request_succeeded: bool,
    pub elevated: bool,
    pub duration_ms: u64,
    pub detail: String,
}

pub fn capability() -> ActionCapability {
    platform::capability()
}

pub fn execute_standby_purge() -> NativeActionResult {
    platform::execute_standby_purge()
}

pub fn classify_outcome(
    request_succeeded: bool,
    nt_status: u32,
    reduction_bytes: Option<f64>,
) -> &'static str {
    if !request_succeeded {
        if nt_status == 0xC000_0120 {
            "elevation_cancelled"
        } else {
            "native_failed"
        }
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
                "ntdll.dll does not export NtSetSystemInformation on this configuration.",
            );
        }
        if !build_supported {
            return unsupported(
                build,
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
            supported: true,
            requires_elevation: !elevated,
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

    fn unsupported(build: Option<u32>, explanation: &str) -> ActionCapability {
        ActionCapability {
            status: "unsupported".into(),
            supported: false,
            requires_elevation: false,
            os_build: build,
            mechanism: "NtSetSystemInformation/SystemMemoryListInformation".into(),
            explanation: explanation.into(),
        }
    }

    pub fn execute_standby_purge() -> NativeActionResult {
        let capability = capability();
        if !capability.supported {
            return NativeActionResult {
                nt_status: 0,
                request_succeeded: false,
                elevated: false,
                duration_ms: 0,
                detail: capability.explanation,
            };
        }

        let started = Instant::now();
        let already_elevated = is_elevated().unwrap_or(false);
        let (status, detail) = if already_elevated {
            let status = perform_native_purge();
            (status, status_detail(status))
        } else {
            launch_elevated_helper()
        };

        NativeActionResult {
            nt_status: status,
            request_succeeded: nt_success(status),
            elevated: already_elevated || nt_success(status),
            duration_ms: started.elapsed().as_millis() as u64,
            detail,
        }
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

    fn launch_elevated_helper() -> (u32, String) {
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                return (
                    0xC000_0001,
                    format!("Could not resolve the MemFlux executable: {error}"),
                );
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
                (
                    0xC000_0120,
                    "Administrator approval was declined or cancelled.".into(),
                )
            } else {
                (
                    0xC000_0001,
                    format!("Could not launch the elevated helper: {error}"),
                )
            };
        }
        let process = OwnedHandle(info.hProcess);
        if process.0.is_invalid() {
            return (
                0xC000_0001,
                "Windows did not return an elevated helper process handle.".into(),
            );
        }
        let wait = unsafe { WaitForSingleObject(process.0, 60_000) };
        if wait == WAIT_TIMEOUT {
            return (
                0x0000_0102,
                "The elevated helper did not finish within 60 seconds.".into(),
            );
        }
        if wait != WAIT_OBJECT_0 {
            return (
                0xC000_0001,
                format!("Waiting for the elevated helper failed (0x{:08X}).", wait.0),
            );
        }
        let mut exit_code = 0_u32;
        if let Err(error) = unsafe { GetExitCodeProcess(process.0, &mut exit_code) } {
            return (
                0xC000_0001,
                format!("Could not read the helper result: {error}"),
            );
        }
        (exit_code, status_detail(exit_code))
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
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub fn capability() -> ActionCapability {
        ActionCapability {
            status: "unsupported".into(),
            supported: false,
            requires_elevation: false,
            os_build: None,
            mechanism: "Windows native memory-list API".into(),
            explanation: "Standby-list purging is supported only on validated Windows builds."
                .into(),
        }
    }

    pub fn execute_standby_purge() -> NativeActionResult {
        NativeActionResult {
            nt_status: 0,
            request_succeeded: false,
            elevated: false,
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
        assert_eq!(
            classify_outcome(true, 0, Some(32.0 * 1024.0 * 1024.0)),
            "request_succeeded_reduction_observed"
        );
        assert_eq!(
            classify_outcome(true, 0, Some(0.0)),
            "request_succeeded_no_measurable_reduction"
        );
        assert_eq!(
            classify_outcome(false, STATUS_PRIVILEGE_NOT_HELD, None),
            "native_failed"
        );
        assert_eq!(
            classify_outcome(false, 0xC000_0120, None),
            "elevation_cancelled"
        );
    }
}
