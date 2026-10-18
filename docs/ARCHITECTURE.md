# Architecture

## Status

Implemented for Phase 1 alpha; later intervention, history, and research subsystems are planned.

## Process model

MemFlux is a standard-user Tauri 2 desktop application. React renders the dashboard in WebView2 while one Rust background thread owns Windows collection and Guardian evaluation. There is no service, driver, helper process, local server, or privileged main process.

```text
Win32 + PDH
    ↓
PlatformCollector (Rust)
    ↓
MemorySnapshot with per-metric provenance
    ↓
GuardianEvaluator (deterministic, monitoring-only)
    ↓
TelemetryFrame cached in native state and emitted to the main window
    ↓
React views and bounded in-memory trend history
```

## Modules

- `src-tauri/src/telemetry`: cross-platform contract and Windows adapter. Optional counters fail independently.
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
- There is no command for memory actions, process changes, shell execution, arbitrary file access, or network access.
- Tauri capabilities grant only `core:default` to the main window.

## Data model

Every metric contains an identifier, optional value, unit, source, timestamp, sampling interval, status, and optional detail. A missing or warming sensor is never represented as numeric zero.

Historical persistence is intentionally absent in Phase 1. The frontend keeps at most 90 recent frames in memory; closing the process discards them.

