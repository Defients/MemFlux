# Architecture

## Status

V4.1 Core Completion implemented: standard-user monitoring (memory, CPU, DXGI/WDDM GPU, optional non-injecting PresentMon FPS), manual and session-authorized automatic standby cleaning, independent monotonic cleaner scheduling, durable quota accounting, Optimization Rewind journal, PowerShift Win32 power-scheme control, GameFocus session management, and Central Optimization Coordinator.

## Process model

MemFlux is a standard-user Tauri 2 desktop application. React renders the Midnight CosmoTech™ interface in WebView2 while two coordinated Rust background threads (`memflux-telemetry` and `memflux-cleaner`) own native Windows collection, GameFocus/PowerShift coordination, and cleaner policy evaluation.

- Manual purging launches the same executable in a one-shot helper mode (`--memflux-elevated-purge-standby`) or uses an already active authenticated session helper.
- Unattended automatic purging uses an explicitly authorized elevated session helper (`--memflux-elevated-session-helper`) communicating over a local Windows Named Pipe with restrictive SDDL permissions, `PIPE_REJECT_REMOTE_CLIENTS`, verified peer PID and executable image path, and a parent-process `SYNCHRONIZE` watchdog that terminates the helper immediately if the controller exits or crashes.

```text
Win32 Memory + PDH + GetSystemTimes + DXGI/WDDM GPU + PowrProf
    ↓
memflux-telemetry thread (Rust)
    ↓
MemorySnapshot + CpuSensor + GpuSensor + OptimizationCoordinator (GameFocus / PowerShift / Rewind)
    ↓
TelemetryFrame cached in native state and emitted to visible main window

memflux-cleaner thread (independent monotonic Instant scheduler)
    ↓
CleanerPolicy + durable cleaner-quota.json + foreground exclusion check
    ↓
Authenticated Named Pipe Session Helper → NtSetSystemInformation(80, 4) → 750 ms settle → ActionHistory
```

## Modules

- `src-tauri/src/action.rs`: build capability & compatibility matrix, `SeProfileSingleProcessPrivilege` enablement, `NtSetSystemInformation(80, 4)` call, one-shot UAC helper, and authenticated Named Pipe session helper.
- `src-tauri/src/cleaner.rs`: dual-threshold policy (`standby >= threshold && free_zero <= threshold`), monotonic + persistent quota ledger, exclusion matching, temporary GameFocus session overrides, and truthful status summaries.
- `src-tauri/src/performance.rs`: `GetSystemTimes` CPU sensor, DXGI adapter discovery, WDDM `\GPU Engine(*)\Utilization Percentage` busiest-engine and per-PID aggregator, foreground window/process inspector, and non-injecting PresentMon ETW capture controller.
- `src-tauri/src/rewind.rs`: versioned atomic `rewind-journal.json` with compare-before-restore conflict detection and startup crash recovery.
- `src-tauri/src/powershift.rs`: documented Win32 `PowerEnumerate`, `PowerGetActiveScheme`, `PowerSetActiveScheme`, `PowerReadFriendlyName`, and `GetSystemPowerStatus` controller.
- `src-tauri/src/gamefocus.rs`: conservative game identification, presets/profiles, Alt+Tab debounced session lifecycle, streaming workload protection, and background process contention analyzer.
- `src-tauri/src/coordinator.rs`: Central Optimization Coordinator serializing GameFocus, PowerShift, Rewind, and cleaner overrides.
- `src-tauri/src/guardian.rs`: diagnostic pressure classifier with 3-sample hysteresis, independent of cleaner execution.
- `src-tauri/src/settings.rs`: schema-v4 validated settings with atomic `MoveFileExW` replacement.
- `src-tauri/src/app.rs`: Tauri commands, state management, tray lifecycle, single-instance policy, and background threads.
