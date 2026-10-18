# Changelog

## 0.2.0-alpha — 2026-10-18

### Added

- Real full standby-list purge through `NtSetSystemInformation` information class 80, command 4.
- Exact `SeProfileSingleProcessPrivilege` enablement and raw `NTSTATUS` propagation.
- Narrow one-shot UAC helper mode with no arbitrary command or file protocol.
- Explicit Windows-build capability allowlist and unsupported state.
- Serialized manual purge with pre/post standby, free/zero, and available measurements.
- Prominent CosmoTech cleaner control, live operation state, accurate outcome panel, and bounded session history.

### Verified

- On Windows 11 Home Insider Preview build 26340, an elevated helper returned `0x00000000`; standby measured 233,820,160 bytes before and 25,538,560 bytes after 750 ms, while Free & Zero measured 10,254,319,616 before and 10,510,528,512 after.
- An unelevated helper invocation returned `STATUS_PRIVILEGE_NOT_HELD (0xC0000061)`.

### Not yet complete

- Automatic threshold cleaning, cooldown/frequency limits, process exclusions, persistent action history, and stable Windows build validation remain Milestone 2 work.
- UAC rejection and native UI interaction paths have not yet been directly automated.

## 0.1.0-alpha — 2026-10-18

### Added

- Tauri 2 + React + strict TypeScript + Rust foundation.
- Documented Windows memory and commit telemetry with PDH sensor degradation.
- OS memory-resource notifications and deterministic handle cleanup.
- Explainable monitoring-only Adaptive Guardian with sustained-pressure hysteresis.
- Rust-owned adaptive sampler independent of UI rendering.
- Versioned local settings with validation, safe fallback, and atomic replacement.
- System tray, close-to-tray, pause/resume, explicit exit, and single-instance focus.
- Midnight CosmoTech™ Overview, Memory Explorer, Guardian, and Settings views.
- Lightweight SVG trend and non-overlapping memory composition.
- Initial unit tests, architecture/security/memory/API/testing/performance documentation, and live implementation plan.

### Safety boundary

- Standby clearing, working-set trimming, timer manipulation, elevation, and automatic intervention are intentionally absent.
- No performance improvement is claimed.

