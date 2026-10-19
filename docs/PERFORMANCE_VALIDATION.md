# Performance Validation

## Targets, not claims

- Hidden/tray CPU: below 0.5% average on representative modern hardware.
- Combined resident working set: exploratory 50–100 MB budget.
- No busy wait; hidden cadence defaults to 5 seconds.
- Bounded in-memory history and no Phase 1 sample database.
- Responsive startup and deterministic resource cleanup.

## Design controls already implemented

- One condition-variable-driven sampler thread.
- One PDH query reused across samples.
- Visibility-based 1-second foreground and 5-second background defaults.
- Ninety-frame maximum frontend trend history.
- Hand-authored SVG graph instead of a charting runtime.
- No particle system, canvas loop, or continuously changing decorative background.
- Release profile uses size optimization, LTO, one codegen unit, and stripping.
- V3 P0 suppresses recurring telemetry events while the dashboard is hidden, keeping native collection and cache active. Its resource impact has not yet been measured; no memory or CPU reduction is claimed.

## Measurement protocol

1. Record exact build, Windows build, hardware, power mode, display state, and sampling settings.
2. Warm the application for two minutes.
3. Measure dashboard-visible and tray-hidden cases separately for at least ten minutes each.
4. Record process CPU, working set, private bytes, handles, and threads at regular intervals.
5. Repeat after a 30-minute soak; beta requires 8+ hours and sleep/resume.
6. Compare identical builds and workloads. Report distributions or averages with duration, not a single Task Manager observation.

## Current evidence

V4 first-slice overhead is not yet profiled. CPU collection adds GetActiveProcessorGroupCount/GetSystemTimes calls on visible, enabled HUD samples and no calls when hidden/disabled. Foreground executable queries run only when exclusions are configured. No GPU or FPS capture exists. These are implementation facts, not an overhead measurement or a performance improvement claim.

Measured 2026-10-18 on Windows 11 Home Insider Preview build 26340 with 24 logical processors and the dashboard visible:

| Build / scope | Duration | Normalized CPU | Private bytes | Working set | Result |
|---|---:|---:|---:|---:|---|
| Debug native process only | 24 s | 0.011% | 9.45 MiB average | 7.70 MiB private working set average | Diagnostic only |
| Optimized release, complete MemFlux + WebView2 child tree | 20.4 s | 0.131% | 249.95 MiB | 426.37 MiB | CPU target met; memory target failed |

The optimized native executable is 3.50 MiB and its own working set was 33.36 MiB at the end of the release measurement. WebView2 child processes dominate the combined memory figure. These are short exploratory measurements, not long-duration guarantees. Hidden/tray overhead was not measured because native UI attachment was unavailable in this session.

Before beta, investigate whether WebView2 feature/process configuration can materially lower the combined footprint without compromising security or rendering. If not, explicitly revise the memory budget or evaluate a lighter native shell; do not market the current build as a 50–100 MB application.

