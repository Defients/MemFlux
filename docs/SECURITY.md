# Security and Privacy

## Phase 1 threat boundary

MemFlux monitors local Windows memory state. It does not modify memory lists, trim other processes, manipulate timers, contact a network service, or accept untrusted remote input.

## Implemented controls

- Standard-user process; no requested-execution-level elevation.
- No service, kernel driver, scheduled task, registry tweak, or privileged helper.
- No arbitrary shell, process, filesystem, HTTP, or updater capability.
- Tauri main-window capability is limited to `core:default`.
- Explicit native commands accept only settings, pause state, or read requests.
- Settings use a fixed app-config path, strict JSON fields, schema/version checks, numeric range validation, safe defaults, and atomic replacement.
- Invalid settings remain preserved for diagnosis and are not silently overwritten at startup.
- Logs describe application state and native failures; they do not intentionally record secrets or process contents.
- No account, analytics, or uploaded telemetry.

## Future privileged action requirements

Any future standby action must be a separately reviewed capability. It must identify the exact mechanism, privilege, supported build set, helper identity and IPC ACL, command allowlist, validation, cooldown, before/after data, and truthful failure result. The UI process must remain unelevated.

The following remain prohibited: kernel drivers, security-feature disabling, command execution from frontend input, arbitrary filesystem access by an elevated helper, insecure deserialization, process injection, game/anti-cheat tampering, and fabricated success.

## Known gaps

- Installer signing and update authenticity are not implemented.
- Log retention policy needs validation before beta.
- A dedicated penetration/security review has not occurred.
- Stable Windows compatibility and controlled UAC-denial paths cannot be tested until a privileged helper legitimately exists.

