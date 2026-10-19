# MemFlux

**Standby Memory Control + Adaptive Intelligence · Powered by CosmoTech™**

MemFlux is a local-first Windows standby-memory cleaning utility. It performs a real, privilege-gated Windows standby-list purge, measures the system before and after, and keeps monitoring and Guardian diagnostics around that core action.

The current cleaner milestone is enabled only on the explicitly validated Windows build. Monitoring remains available without elevation; a manual purge launches the same binary in a narrow one-shot helper mode through Windows UAC.

## Working cleaner milestone

- Prominent **PURGE STANDBY MEMORY** control.
- Native `NtSetSystemInformation` call using information class 80 and `MemoryPurgeStandbyList` command 4.
- One-shot UAC helper that accepts only the fixed standby-purge operation and enables only `SeProfileSingleProcessPrivilege`.
- Explicit capability detection and a tested-build allowlist; unsupported builds keep monitoring but disable the action.
- Serialized actions with real standby, free/zero, and available-memory snapshots before and after a 750 ms settling interval.
- Typed helper lifecycle, optional raw `NTSTATUS`, request success, measured reduction, duration, elevation path, and bounded persistent history.
- Helper timeout is never interpreted as native success; late completion remains explicitly possible and is reported.
- Honest success-with-no-measurable-reduction reporting.

## Monitoring foundation

V4 adds a compact sidebar HUD with real CPU and existing RAM utilization, warm-up/stale states, a persisted show/hide preference, and keyboard-accessible compact disclosure. CPU is sampled with the native pipeline only when the HUD/dashboard is visible. GPU, FPS and frame time display unavailable; no capture engine is running. Foreground executable observation is now connected to cleaner exclusion evaluation using standard-user query rights.

- Native `GlobalMemoryStatusEx` and `GetPerformanceInfo` collection.
- Documented Windows low/high memory resource notifications.
- PDH English counters for standby classes, free/zero pages, modified pages, and hard page reads.
- Per-metric source, timestamp, interval, status, and failure detail.
- Explainable Guardian pressure assessment kept separate from cleaner control.
- Adaptive foreground/background sampling owned by Rust.
- Versioned, validated, atomically replaced local settings.
- System tray open, pause/resume, close-to-tray, explicit exit, and single instance.
- Responsive Midnight CosmoTech™ Overview, Memory Explorer, Guardian, and Settings views.

## P0 cleaner evolution

- Automatic-cleaner settings are now versioned and persisted, remain off by default, and expose direct standby and Free & Zero thresholds, polling, cooldown, minimum interval, hourly limit, cleaner-only pause, and executable exclusions.
- A deterministic policy evaluator enforces both threshold conditions and the configured safeguards independently of Guardian.
- Unattended native execution is intentionally still disabled. A background threshold must not launch surprise UAC prompts; the next slice is an explicitly authorized, narrowly scoped session helper with authenticated local IPC.

## Not implemented yet

- Secure unattended automatic purge execution, independent cleaner scheduling, and monotonic cooldown reconciliation (policy, foreground exclusion observation and persistence are implemented; native automatic execution is not).
- Low-priority-only standby purge, pending independent native validation.
- Timer-resolution changes.
- Game overlays, PresentMon, performance sessions, and performance-improvement claims.
- Windows versions other than those actually tested and recorded in the compatibility document.

## Development

Requirements: Windows 10/11 x64, WebView2, Node.js, Rust MSVC, Visual Studio C++ Build Tools, and a Windows SDK.

```powershell
npm install
npm run build
npm test
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri dev
```

Create a native debug executable without an installer:

```powershell
npm run tauri build -- --debug --no-bundle
```

See [MEMFLUX_IMPLEMENTATION_PLAN.md](MEMFLUX_IMPLEMENTATION_PLAN.md) and [MEMFLUX_PERFORMANCE_EVOLUTION_V3_PLAN.md](MEMFLUX_PERFORMANCE_EVOLUTION_V3_PLAN.md) for live milestone status, measurable gates, current risks, and actual validation results.

## Safety and privacy

Monitoring runs as a standard user. Manual cleaning requests UAC only for a one-shot helper in the same executable. The helper accepts no arbitrary shell command or path, installs no service or driver, and returns only its native status through the process exit code. MemFlux uploads no analytics or system telemetry. See [docs/SECURITY.md](docs/SECURITY.md).

## Project maturity

See [MEMFLUX_OMNIFLUX_V4_IMPLEMENTATION_PLAN.md](MEMFLUX_OMNIFLUX_V4_IMPLEMENTATION_PLAN.md) for the current incremental roadmap. The CPU/RAM HUD is an initial slice; GPU, FPS, trusted-counter comparison, resource profiling and visual QA remain required for the full HUD gate.

P0 hardening alpha. Manual native purging is runtime-verified only on Windows 11 Home Insider Preview build 26340; other builds remain action-disabled until tested. Automatic policy is implemented but unattended authorization/execution is incomplete. Clearing cache is not a memory-leak fix and is not evidence of higher FPS or lower latency.

