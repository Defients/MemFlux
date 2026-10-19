# Performance Validation

## Targets, not claims

- Hidden/tray CPU: below 0.5% average on representative modern hardware.
- Native process footprint: compact (<15 MiB private bytes, <40 MiB working set); WebView2 child processes tracked separately.
- No busy wait; hidden telemetry cadence defaults to 5 seconds, with independent monotonic cleaner scheduling.
- Bounded in-memory history (90 frames), bounded action history (100 records), bounded quota ledger (256 records), and bounded Rewind journal (200 entries).

## Design controls implemented in V4.1

- Two condition-variable-driven background threads (`memflux-telemetry` and `memflux-cleaner`) with zero busy-wait loops.
- Reused PDH queries for memory counters and WDDM `\GPU Engine(*)\Utilization Percentage` wildcard counters.
- CPU and WDDM GPU collection suspend when the dashboard is hidden and GameFocus does not require GPU contention data.
- Hidden dashboard receives no recurring frontend IPC events.
- Non-injecting PresentMon frame capture never starts automatically at launch and runs only when explicitly permitted and requested.
- Release profile uses `opt-level = "s"`, LTO, `codegen-units = 1`, `panic = "abort"`, and symbol stripping.

## Measured V4.1 Release Footprint (Build 26340, 24 Logical Processors)

Measured 2026-10-19 on `src-tauri/target/release/memflux.exe` (`4,063,744` bytes, SHA-256 `CBC1FE5EAC1B072F4E134253E41F29F8B1EE422E7126811F649BA0BC405EA6EC`) with dashboard visible and CPU, WDDM GPU Engine, Memory PDH, GameFocus detector, PowerShift, and independent cleaner scheduler active:

| Scope | Processes | Normalized CPU | Private Bytes | Working Set | Handles | Threads |
|---|---:|---:|---:|---:|---:|---:|
| Native `memflux.exe` only | 1 | 0.0271% | 11.36 MiB | 33.82 MiB | 443 | 39 |
| WebView2 child process tree | 6 | <0.10% | 253.04 MiB | 378.47 MiB | — | — |
| Combined visible process tree | 7 | ~0.12% | 264.40 MiB | 412.29 MiB | — | — |
| Historical V3/V4 baseline (combined) | 7 | 0.131% | 249.95 MiB | 426.37 MiB | — | — |

### Analysis

- The native Rust runtime (`memflux.exe`) uses only **11.36 MiB private bytes**, **33.82 MiB working set**, and **0.0271% normalized CPU** with all V4.1 subsystems running.
- As in previous milestones, Chromium/WebView2 child processes account for ~95% of private committed memory (~253 MiB) while the dashboard window is open.
- Long-duration (8+ hour) soak testing and hidden-to-tray WebView2 memory reclamation measurements remain required before promoting from **Beta Candidate** to **Public Beta / Release Candidate**.

## Atlas alpha measurement scope

The engine's deterministic replay microbenchmark reports evaluation p50/p95/max and serialized report bytes for 1,000 synthetic ticks with one incident per tick, mixed recovery, the 90-tick ring and 64 retained incident records. Run it in isolation:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --release atlas::tests::bounded_replay_and_profile -- --exact --nocapture
```

This measures engine execution and bounded transport size, not real gameplay, allocation/private bytes, WebView2, disk traffic or capture overhead. Debug measurements varied materially under concurrent compilation/native tests; do not extrapolate them to the release application. No new dedicated sampler/capture thread is added. Hidden mode continues the existing cadence; Atlas's supported first slice requires no expensive additional source. Frontend chart history excludes repeated Atlas reports.

Required remaining profiling: matching V4.2 release baseline versus Atlas in passive, visible dashboard, game session, Stutter, full diagnostic, Mini HUD and Ultra-Light modes. Record normalized CPU, private bytes/working set for native and WebView2 separately, disk I/O, handles, capture costs and aligned frame distributions. Existing V4.1 numbers above remain historical and are not Atlas measurements.

Recorded optimized run (2026-10-19, single filtered test on build 26340.9750): p50 `179us`, p95 `255us`, max `548us`; serialized final report `363,496` bytes. Passes the provisional <1ms engine p95 and <1MiB serialized report targets in this synthetic test only. This does not establish whole-app CPU/memory/disk/capture budgets.
