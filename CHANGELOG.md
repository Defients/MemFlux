# Changelog

## Unreleased — OmniFlux V4 first slice

- Added V4 repository audit, phase ledger, dependency gates and continuation checklist.
- Added real GetSystemTimes CPU utilization with correct idle accounting, warm-up, invalid delta/reset handling and multi-group unavailable state.
- Added CPU/RAM sidebar HUD without changing navigation widths; paused/stale metrics render unavailable. GPU/FPS/frame time remain explicitly absent.
- Added persisted HUD visibility preference (settings schema 3); CPU collection suspends when hidden/disabled/monitor-paused.
- Connected foreground executable lookup to cleaner exclusions using limited process-query rights and rechecking foreground ownership.
- Added CPU arithmetic/live-native tests and HUD rendering/freshness tests. Native automatic execution, independent cleaner scheduling, GPU and FPS capture remain incomplete.
- Verified 23 native tests, 4 frontend tests, strict lint/format, frontend build, native debug build and optimized no-bundle release build. Resource overhead and native visual QA remain unmeasured/unverified.

## Unreleased — Performance Evolution V3 P0 hardening

### Corrected

- Replaced overloaded helper/native integers with typed lifecycle outcomes.
- `WAIT_TIMEOUT (0x00000102)` can no longer be reported as an accepted native purge.
- Timeout and wait failures explicitly report possible late helper completion without claiming termination.
- Raw `NTSTATUS` is present only when the in-process call returns or the helper completed and its exit code was retrieved.
- Timed-out helper handles remain owned; duplicate requests are blocked until exit is observed. Late completion is logged without claiming measured freed bytes.
- Concurrent history replacement and settings persistence are serialized; same-millisecond action IDs remain distinct.
- Missing post-action counters have a measurement-unavailable outcome.

### Added

- V3 audit and phased implementation plan.
- Compatibility states separating confirmed support, unverified builds, privilege state, and missing mechanism.
- Versioned, opt-in automatic-cleaner settings for direct standby/Free & Zero thresholds, polling, cooldown, minimum interval, hourly rate, pause, and exclusions.
- Deterministic cleaner policy with threshold, authorization, exclusion, cooldown, and rate-limit tests.
- Bounded action-history persistence through an atomic local journal.
- Settings interface for the automatic policy with an explicit warning that unattended execution is not yet enabled.

### Verification

- 21 Rust tests pass, including production helper-result conversion, timeout regression, exact exclusion matching, historical cooldown, settings validation, persistent history, Guardian, and live Windows telemetry.
- Rust format and Clippy with warnings denied pass.
- TypeScript/Vite production build and 2 frontend tests pass.
- Optimized no-bundle Tauri release build passes; rebuilt standard-user helper returns `0xC0000061` (`STATUS_PRIVILEGE_NOT_HELD`). Successful elevated purge and native timeout paths were not rerun.

### Incomplete

- Secure elevated session-helper IPC and native automatic execution remain P0-C work. Background evaluation does not prompt for UAC or claim an automatic purge.
- Foreground executable observation for exclusions, stable Windows 10/11 qualification, native timeout/UAC rejection tests, and P1 features remain unimplemented or unverified.
- Hidden dashboard event emission is suspended; resource savings have not been measured.

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

