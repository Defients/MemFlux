# Architecture

## Status

Monitoring, manual cleaning, deterministic automatic policy evaluation, and persistent bounded action history are implemented. Unattended automatic authorization and native execution remain incomplete.

## Process model

MemFlux is a standard-user Tauri 2 desktop application. React renders the dashboard in WebView2 while one Rust background thread owns Windows collection and Guardian evaluation. Manual purging launches the same executable as a one-shot elevated helper with one exact argument. There is no service, driver, local server, or elevated main process.

```text
Win32 + PDH
    ↓
PlatformCollector (Rust)
    ↓
MemorySnapshot with per-metric provenance
    ↓
GuardianEvaluator (deterministic pressure assessment)
    ↓
TelemetryFrame cached in native state and emitted to the main window
    ↓
React views and bounded in-memory trend history

Manual command → serialize → pre-snapshot → fixed UAC helper → NTSTATUS → 750 ms settle → post-snapshot → bounded action history
```

## Modules

- `src-tauri/src/performance.rs`: CPU counter delta validation and standard-user foreground executable observation. CPU metric travels in the shared TelemetryFrame; no separate HUD collector thread.
- `src/components/PerformanceHud.tsx`: CPU/RAM sidebar presentation, explicit missing GPU/capture values, stale detection and compact disclosure. Schema-3 settings persist visibility.

- `src-tauri/src/telemetry`: cross-platform contract and Windows adapter. Optional counters fail independently.
- `src-tauri/src/action.rs`: build capability, exact privilege enablement, native call, and fixed one-shot elevated helper.
- `src-tauri/src/guardian.rs`: deterministic pressure classifier, hysteresis, explanations, and tests.
- `src-tauri/src/settings.rs`: schema version 2, monitoring/cleaner validation, safe defaults, and atomic Windows replacement.
- `src-tauri/src/cleaner.rs`: direct standby/Free & Zero threshold policy, cooldown, hourly limits, exclusion matching and authorization gating, separate from Guardian.
- `src-tauri/src/app.rs`: lifecycle, commands, native state, sampler, tray, and single-instance policy.
- `src/hooks/useMemFlux.ts`: typed event/command bridge with bounded 90-sample UI history.
- `src/App.tsx`: Overview, Memory Explorer, Adaptive Guardian, and Settings vertical slice.
- `src/styles.css`: semantic Midnight CosmoTech™ tokens and responsive/accessibility states.

## Concurrency and lifecycle

- The collector is created and used only on the named `memflux-telemetry` thread.
- A condition variable waits between samples; settings, pause, resume, and shutdown wake it. There is no busy-wait loop.
- The cadence is selected from validated settings based on main-window visibility.
- The latest complete frame is held in a read/write lock for initial UI hydration.
- Hidden windows receive no recurring telemetry events. The native sampler continues and Open dashboard emits the cached frame.
- Helper transport results are typed separately from native status. Timeout retains a process handle; a new purge is blocked until the previous helper is signaled. A launched helper with no observable handle blocks further launches for the process lifetime. No helper is forcibly terminated.
- The pending-helper guard is process-local. Parent exit does not terminate an already launched one-shot helper; cross-restart reconciliation remains unimplemented.
- History mutation and atomic persistence share the same lock to prevent concurrent writes or stale replacement.
- Guardian state is reset while paused, preventing old consecutive samples from carrying across a pause.
- Close hides to tray only when configured. Tray Exit sets shutdown and exits. Single-instance launch focuses the existing window.

## Trust boundaries

- React can invoke only explicit Tauri commands with typed JSON arguments.
- The native runtime validates and persists settings.
- The only privileged command is a fixed standby-list purge. The helper accepts no path, payload, threshold, shell text, or arbitrary operation.
- Tauri capabilities grant only `core:default` to the main window.

## Data model

Every metric contains an identifier, optional value, unit, source, timestamp, sampling interval, status, and optional detail. A missing or warming sensor is never represented as numeric zero.

The frontend keeps at most 90 telemetry frames in memory. Action records persist across restarts in `action-history.json` at the fixed application config directory, bounded to 100 records. Post-action measurement absence is distinct from an observed reduction of zero.

