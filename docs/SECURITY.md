# Security and Privacy

## Cleaner threat boundary

MemFlux monitors local Windows memory state and can request one full standby-list purge. It does not trim process working sets, manipulate timers, contact a network service, or accept untrusted remote input.

## Implemented controls

- Standard-user main process; no requested-execution-level elevation.
- One-shot UAC launch of the same executable with exactly `--memflux-elevated-purge-standby`.
- Helper accepts no arbitrary command or filesystem path, enables only `SeProfileSingleProcessPrivilege`, performs one native request, and returns raw `NTSTATUS` as its process exit code.
- Helper launch, UAC cancellation, wait timeout/failure, exit-code retrieval, native status, and measurement are represented separately. A wait timeout carries no `NTSTATUS`, does not claim success, and warns that the helper may finish later.
- No service, kernel driver, scheduled task, registry tweak, or privileged helper.
- No arbitrary shell, process, filesystem, HTTP, or updater capability.
- Tauri main-window capability is limited to `core:default`.
- Explicit native commands accept only settings, pause state, or read requests.
- Settings and bounded action history use fixed app-config paths, schema/version checks, numeric range validation, safe defaults, and atomic replacement.
- Invalid settings remain preserved for diagnosis and are not silently overwritten at startup.
- Logs describe application state and native failures; they do not intentionally record secrets or process contents.
- No account, analytics, or uploaded telemetry.

## Compatibility controls

The private memory-list class/command is enabled only on an explicit tested-build allowlist. Missing export, unlisted build, invalid telemetry, a concurrent action, elevation denial, missing privilege, timeout, and failed `NTSTATUS` each produce distinct non-success outcomes.

The following remain prohibited: kernel drivers, security-feature disabling, command execution from frontend input, arbitrary filesystem access by an elevated helper, insecure deserialization, process injection, game/anti-cheat tampering, and fabricated success.

## Known gaps

- Installer signing and update authenticity are not implemented.
- Log retention policy needs validation before beta.
- A dedicated penetration/security review has not occurred.
- Controlled UAC-denial and helper-timeout paths still need native interaction testing.
- Automatic native execution is disabled until an explicitly authorized session helper with authenticated, fixed-operation IPC is implemented and reviewed. Background policy evaluation never opens UAC.

