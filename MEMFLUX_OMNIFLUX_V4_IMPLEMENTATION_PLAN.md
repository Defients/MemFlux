# OmniFlux V4 implementation ledger

Audit: 2026-10-18. Baseline `fb6125aae32f7a9c1087328714b73aefb5a7550c`, main, clean and synchronized at audit. `sources/` remains read-only.

## Inventory and reconciliation

Tauri 2 / React 19 / TypeScript / Rust, local JSON settings, atomic bounded action history, tray and single-instance shell. Existing memory sensors use GlobalMemoryStatusEx, GetPerformanceInfo, memory resource notifications and PDH. Guardian pressure assessment is separate from cleaner policy. The native one-shot helper accepts only full standby purge, class 80 command 4, enabling SeProfileSingleProcessPrivilege. Only build 26340 is allowlisted. Timeout is typed separately and pending process handles prevent duplicate launches in the current process. Cross-restart helper reconciliation remains absent.

Policy/settings/history exist; unattended cleaning does not. Foreground identity was passed as None, so configured exclusions always blocked evaluation. Cleaner evaluation shares monitor cadence and pause, contrary to independent-scheduler requirements. Cooldowns use wall time and bounded action history, so clock changes and retention churn require correction before automatic execution. The full UI already has a 250/218/82 px responsive sidebar; it must not widen.

No CPU, GPU, FPS, frame-time, GameFocus, PowerShift, restoration journal, process explorer, storage, thermals, OBS or startup integrations exist at baseline. Historical memory-overhead measurements are not current evidence. The existing 21 native tests / 2 frontend tests are a foundation, not V4 acceptance.

## Phases, dependencies and gates

| Phase | Work | Gate / current state |
|---|---|---|
| 0 | Reconcile implementation, audit and plan | Audited; ledger created |
| 1 | Authenticated session helper, independent scheduler, monotonic cooldown, foreground exclusions | Partial policy/manual cleaner; automatic execution disabled |
| 2 | Shared CPU/GPU/RAM sensors and responsive HUD | First slice: real CPU/RAM and foreground exclusions; GPU/FPS explicitly unavailable |
| 3 | Verified game identity, profiles, lifecycle and optional PresentMon capture | Planned; depends on identity and capture feasibility |
| 4 | Rewind journal, compare-before-restore, power and process interventions | Planned; no mutation before authorization/restoration gates |
| 5 | Memory Hunter, VRAM, disk and optional thermal diagnostics | Planned; sources must preserve process/device attribution |
| 6 | FrameLab, repeatable sessions, Stutter Detective | Planned; requires actual synchronized frame capture |
| 7 | StreamShield/OBS, startup inventory and reversible controls | Planned; authentication and recovery required |
| 8 | Stable OS matrix, packaging, signing, accessibility and soak | Planned; representative environments required |

## Architecture decisions

- Preserve existing collector and native boundary. Add read-only sensors to the shared native sampling pipeline, not individual UI timers or external services.
- CPU uses documented GetSystemTimes deltas: busy = kernel + user - idle. Kernel includes idle; no additional division by processor count. Warm-up, zero delta, reset and API failure have absent values. Systems with multiple processor groups are reported unavailable until group-aware collection exists.
- Foreground exclusions use GetForegroundWindow, GetWindowThreadProcessId, limited-query OpenProcess and QueryFullProcessImageNameW. Recheck foreground ownership after reading; inaccessible or changed identity fails closed. This is read-only exclusion observation, not game detection or authorization to manipulate a PID.
- HUD uses current memory metrics, the CPU metric contract and explicit absent GPU/FPS/frame values. No FPS capture starts automatically. Compact mode uses keyboard-accessible disclosure. Persist show/hide preference; other HUD options are incremental work.
- Continue to prohibit background UAC. A future explicit session helper needs authenticated local IPC, fixed operations, limited lifetime, disconnect shutdown and security tests before enabling automatic native execution.

## OS and privilege matrix

| Operation | Interface / privilege | Validation |
|---|---|---|
| Memory monitoring | Documented Win32 / PDH, standard user | Build 26340 runtime evidence |
| CPU utilization | GetSystemTimes, standard user | New slice; native test required |
| Foreground executable | Documented User32/process-query APIs, standard user | New slice; desktop/access restrictions possible |
| Standby purge | Compatibility-sensitive NtSetSystemInformation(80,4), elevated privilege | Build 26340 only; historical successful purge and rebuilt privilege-denial evidence |
| GPU/FPS | No source implemented | Unavailable, never zero |
| Mutable V4 interventions | No implementation | Disabled / planned |

## Verification and resource budgets

Run strict frontend build/tests, native tests, format, Clippy and a native build. CPU tests cover idle inclusion, warm-up, zero delta, discontinuity, overflow and bounds. Native CPU test obtains two real snapshots without elevation. Compare with trusted OS counters over aligned windows before accepting full HUD accuracy. Exclusions retain fail-closed behavior; real desktop transitions remain required.

One shared visible sampling cadence (default 1 s), hidden default 5 s; CPU should not be collected while hidden. No ETW, GPU polling or capture process in this slice. UI buffers remain bounded at 90 samples; action retention 100. Target normalized background CPU below 0.5% over at least five minutes. Renew private bytes and working-set measurements separately across native/WebView2 processes; do not claim resource savings without measurement. Full HUD gate requires GPU semantics, resource profiling, stale/narrow/UI QA and trusted CPU comparison.

## Blockers and continuation checklist

- Secure unattended helper IPC is unimplemented; automatic native execution stays disabled.
- Stable Windows 10/11 test environments are unavailable locally; retain build allowlist.
- Implement independent monotonic cleaner scheduling and durable quota accounting before enabling auto.
- Test real foreground transitions, access denial, CPU trusted-counter comparison, hidden/show, DPI and keyboard behavior.
- Investigate GPU Engine attribution and PresentMon licensing/capture without exposing fake controls.
- Add restoration journal before new power/process/startup modifications.
- Update this ledger and existing plans/docs with actual test results after each slice.

Current maturity: alpha. No V4 phase is declared fully accepted by scaffolding or compilation.

## First-slice implementation ledger

Implemented: foreground executable lookup connected to exclusions; shared GetSystemTimes CPU sensor; CPU/RAM sidebar HUD; missing GPU/FPS/frame states; warm-up and stale/paused handling; unchanged sidebar widths; compact keyboard disclosure; persisted show/hide preference (schema 3). GPU selection and per-metric preferences remain future work.

Verification: 23 Rust tests passed in the first native run, including a real two-sample CPU delta; strict Clippy passed in the final run. Frontend production build and all 4 tests passed, including HUD measured values and stale/missing states. Final native test/build results will be appended. No trusted-counter CPU comparison or native desktop foreground transition test has occurred. Runtime CPU tests do not establish broad Windows compatibility.

Resource impact remains unmeasured. CPU sensor performs two cheap kernel queries per visible sample, no query when hidden/disabled, and stores one baseline. Foreground process lookup occurs only when exclusions are configured. This describes implementation cost, not measured overhead.

Highest-priority continuation remains Phase 1: design and verify authenticated session-helper IPC, independent monotonic scheduling and durable quota accounting, then controlled automatic purge tests. Read-only HUD continuation: GPU source attribution, PresentMon feasibility, CPU reference-counter comparison and UI/resource profiling.

Final checks: 23/23 native tests, 4/4 frontend tests, format, all-target Clippy with warnings denied, frontend production build, native debug build and optimized no-bundle Tauri release build passed. Release compilation took 3m 06s. Output is `src-tauri/target/release/memflux.exe`. Live CPU tests ran on the existing Windows build-26340 host without elevation; no purge or new privileged action was exercised. Visual HUD QA and resource profiling remain untested. Source changes are local on main over baseline fb6125a; `sources/` was not modified.
