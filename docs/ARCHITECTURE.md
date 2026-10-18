# Architecture

## Status

Monitoring foundation and Milestone 1 manual cleaner are implemented. Automatic policy and persistent history are planned.

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

- `src-tauri/src/telemetry`: cross-platform contract and Windows adapter. Optional counters fail independently.
- `src-tauri/src/action.rs`: build capability, exact privilege enablement, native call, and fixed one-shot elevated helper.
- `src-tauri/src/guardian.rs`: deterministic pressure classifier, hysteresis, explanations, and tests.
- `src-tauri/src/settings.rs`: schema version 1, range validation, safe defaults, and atomic Windows replacement.
- `src-tauri/src/app.rs`: lifecycle, commands, native state, sampler, tray, and single-instance policy.
- `src/hooks/useMemFlux.ts`: typed event/command bridge with bounded 90-sample UI history.
- `src/App.tsx`: Overview, Memory Explorer, Adaptive Guardian, and Settings vertical slice.
- `src/styles.css`: semantic Midnight CosmoTech™ tokens and responsive/accessibility states.

## Concurrency and lifecycle

- The collector is created and used only on the named `memflux-telemetry` thread.
- A condition variable waits between samples; settings, pause, resume, and shutdown wake it. There is no busy-wait loop.
- The cadence is selected from validated settings based on main-window visibility.
- The latest complete frame is held in a read/write lock for initial UI hydration.
- Guardian state is reset while paused, preventing old consecutive samples from carrying across a pause.
- Close hides to tray only when configured. Tray Exit sets shutdown and exits. Single-instance launch focuses the existing window.

## Trust boundaries

- React can invoke only explicit Tauri commands with typed JSON arguments.
- The native runtime validates and persists settings.
- The only privileged command is a fixed standby-list purge. The helper accepts no path, payload, threshold, shell text, or arbitrary operation.
- Tauri capabilities grant only `core:default` to the main window.

## Data model

Every metric contains an identifier, optional value, unit, source, timestamp, sampling interval, status, and optional detail. A missing or warming sensor is never represented as numeric zero.

Historical persistence is intentionally absent in Phase 1. The frontend keeps at most 90 recent frames in memory; closing the process discards them.

