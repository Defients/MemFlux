# Changelog

## 2026-10-19 — Interface refinement pass

Visual polish only — no behavior, copy, layout structure, or information-architecture changes. `tsc -b` + `vite build` clean; all 15 frontend tests pass.

- **Readability:** raised the smallest text sizes across the dashboard (≈7.5–10.5 px → ≈8–11.5 px) and tightened line-height rhythm so dense telemetry stays legible; brightened `--text-muted`/`--text-secondary` for small-text contrast; added a global monospace `code` face and `::selection` styling.
- **Depth & affordance:** inset top highlight on panels, cards, and elevated surfaces; metric-card and table-row hover states; full-bleed hover on settings rows; custom select chevrons (replacing native arrows); input focus rings; success/failure tinting on the cleaner result panel; WebKit scrollbar theming.
- **Motion:** animated memory-dial sweep via registered `@property --dial-value`, animated composition-bar/HUD track widths, page fade-in on navigation, disclosure chevrons on Atlas domains and the sidebar HUD — all suppressed under reduced-motion settings.
- **Harmonization:** Bottleneck Atlas panels now use the shared design tokens instead of hard-coded colors; consistent 22 px panel padding; brand/nav divider; session-strip accent edge; HUD track colors aligned to the accent palette.
- **Fixes:** `.cleaner-panel` referenced the undefined `--surface-panel` variable (its background declaration was invalid and the gradient never rendered) — replaced with the intended tinted surface; the fixed-width sensor-table columns no longer crush the fifth column of the Stutter Detective and Performance Proof tables (width rules now scope to four-column tables).
- **Dev tooling:** added `preview.html` + `src/preview.tsx`, a dev-only mocked-IPC surface for browser-based visual QA (`npm run dev` → `/preview.html`); it is not referenced by `index.html` and is excluded from production bundles.

## 2026-10-19 — Dashboard layout & settings save polish

Local validation: 15 frontend tests passed (4 new Settings save-floater regressions); strict TypeScript build passed. Rust suite unchanged (69 tests); cargo fmt and strict all-target Clippy pass.

- Overview is now a telemetry-only command center: the standby cleaner panel, session authorization, and Adaptive Guardian panel are consolidated under Memory Control.
- Settings moved the save action into a floating bar with Discard, shown only while a draft diverges or a save is in flight. Fixed an unhandled rejection on save failure, an untracked confirmation timer, and a stale "Saved" label surviving into a new edit.
- Keyboard focus on settings toggles now draws the ring on the visible switch instead of the 1px hidden checkbox input.
- Fire-and-forget action buttons (purge, session authorize/revoke, PowerShift, Rewind restores, GameFocus, FPS capture, onboarding dismiss) now consume their rejections; hook-recorded failures keep surfacing through the shared error banner. GameFocus profile edits keep form state when a save fails.
- Onboarding eyebrow updated to the 1.0 Release Candidate product name.

## 1.0.0 — Release Candidate · 2026-10-19

- Version synchronized to `1.0.0` across `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, and the sidebar footer label (`Beta Candidate` → `Release Candidate`).
- Scope unchanged from the 2026-10-19 Bottleneck Atlas diagnostic alpha; stable Windows 10/11 qualification and live PresentMon validation remain pending.

## 2026-10-19 — Bottleneck Atlas foundation / diagnostic alpha

Local validation: 68 Rust tests passed (one actual power-change test excluded), 11 frontend tests passed; format, strict all-target Clippy, TypeScript/Vite and final optimized executable build passed. Remote CI/installer qualification remains pending.

- Added a shared Rust sensor/evidence model, bounded 90-tick observation buffer and 64 incident investigations. First rules identify sustained joint memory/commit/page-read pressure, healthy reclaimable cache and insufficient evidence; they do not identify a causal root bottleneck.
- Added Atlas findings, six-domain coverage disclosures, evidence/attribution inspector, incident selection and linked Proof baselines to the existing Midnight interface. CPU/GPU remain context; disk/VRAM/thermal/streaming collectors are not implemented.
- Removed the unused test-only PresentMon method that failed strict all-target Clippy. Preserved provider QPC/relative timestamps and CSV arrival separately; unreliable mapping suppresses time correlation and current frame metrics. Selected the first observed swapchain explicitly; isolated reader state across captures. Stutter uses incremental sorted baseline maintenance and resets on mapped gaps.
- Fixed pipe cancellation lifetime by draining completion before releasing OVERLAPPED/events/buffers, reset reused events, and bounded the wait for client release before disconnect so unread helper replies are not discarded. Native cancellation and delayed-reader regressions use no elevated operation.
- Removed Proof's unsupported sampling-noise/significance heuristic and favorable utilization labels. Reports are descriptive only, require 30-second phases after 5-second warm-up, reject duplicate/nonfinite samples, retain the final linked report in native memory and refuse replacement of an active baseline. Full comparable-workload/repeated-trial outcome scoring remains deferred.
- Added per-source HUD freshness/pause handling, live Mini HUD preference updates, field-specific HUD disable, optimistic settings conflict checks and serialized native settings writes. Chart history omits repeated Atlas incident payloads.
- New functionality is an engineering alpha. Gameplay capture, native action requalification, stable Windows matrix, durable incident/experiment history, full interactive timeline and seven-mode overhead profiling remain unverified or unimplemented; historical cleaner qualification is unchanged.


## Unreleased — MemFlux V4.2 Reliability, Recovery & Performance Proof

### Added

- **Coordinated shutdown lifecycle (`app.rs`):** Ordered teardown — refuse new actions → drain in-flight action (≤3 s) → stop PresentMon/ETW capture → restore system state (GameFocus + Rewind journal obligations via `coordinator.shutdown_and_restore`) → revoke the privileged session helper → flush persistence (action history, pending Mini HUD position). Tray Exit, window close, and `RunEvent::Exit` all converge on the same idempotent path; commands gate on the shutdown flag.
- **Rewind V2 state machine (`rewind.rs`):** Explicit lifecycle `Prepared → AppliedVerified | OutcomeUncertain | FailedBeforeMutation`, restoration side `RestorationPending → Restored | ExternalChangePreserved`, `RecoveryRequired`, `Irreversible`. Journal eviction now protects unresolved obligations; schema v1 journals migrate (`failed → outcome_uncertain`, `conflict_skipped → external_change_preserved`, `pending → prepared`).
- **Shutdown-time restoration (`coordinator.rs`):** `shutdown_and_restore` closes the GameFocus session, then reconciles every unresolved journal entry against live Windows state — `AppliedVerified`/`OutcomeUncertain`/`RestorationPending`/`RecoveryRequired` are all restored with compare-before-restore external-change preservation. Per-tick transitions gate on the shutdown flag.
- **Session-helper IPC hardening (`action.rs`):** Session ID + pipe suffix now derive from `BCryptGenRandom` (32-hex-char CSPRNG material) instead of PID+timestamp; all client and server pipe I/O uses `FILE_FLAG_OVERLAPPED` with bounded `WaitForSingleObject` + `CancelIoEx` (connect/read/write/shutdown all bounded); server enforces strictly increasing `request_id` (replay/reorder rejected); IPC failure mid-purge sets `late_completion_possible = true`.
- **Monotonic quota restore (`cleaner.rs`):** Persisted wall-clock quota timestamps are anchored to `Instant` at load with clamped age; clock rollback can no longer bypass cooldown or hourly limits and future timestamps never over-block.
- **PresentMon provider repair (`performance.rs`):** Unique `--session_name MemFlux_<random>` replaces `--stop_existing_session`; `--terminate_on_proc_exit` when supported; capability probing parses `--help`; quoted-field CSV parser with header-name indexing; target PID + creation-time identity tracking rejects stale frames on PID reuse; empty WDDM counter sets now report unavailable instead of a fabricated 0%.
- **Stutter Detective (`stutter.rs`):** Online detector over verified `MsBetweenPresents` frames — rolling ≥64-frame p95 baseline, flag threshold `max(p95 × 1.8, p95 + 7 ms)`, dropped presents flagged separately, anomalous frames excluded from baseline so sustained degradation still reports. Events surface in `FpsCaptureStatus.recent_stutters` and the Intelligence page correlates each event with timestamp-matched CPU/GPU/memory telemetry (correlation only, never causation).
- **Performance Proof (`proof.rs`):** Opt-in measurement workflow — begin baseline → apply change → mark boundary → finish & compare. Mean ± stddev comparison per metric (CPU, GPU, RAM, FPS, frame time, p99) with `measurable`/`inconclusive` confidence and explicit same-workload caveats; verdicts never claim proof of causation.
- **Ultra-Light Tray Mode (`app.rs`, settings v5):** Opt-in `ultra_light_tray_mode` — closing the dashboard with close-to-tray destroys the `main` webview entirely; native sampler, cleaner policy, and restoration keep running; tray "Open dashboard" recreates the window on demand.
- **Independent Mini HUD (`app.rs`, `MiniHud.tsx`):** Frameless transparent always-on-top `minihud` webview sharing the same bundle (`?hud=mini`) and the same native telemetry event stream — no dedicated sensor pipeline. Presets (minimal/compact/detailed), opacity, always-on-top, click-through, position persistence with throttled writes, tray toggle, and close-to-disable persistence.

### Verification

- 54 Rust tests (rewind recovery/migration/retention, IPC auth/replay/timeout/session-material, quota clocks, PresentMon parse/attribution, GPU empty-set, CPU deltas, stutter detection, proof comparison, shutdown gating) — all green. 5 frontend tests green; `tsc -b` + `vite build` clean; `cargo clippy` 0 warnings.
- **Untested on this pass:** real PresentMon capture against a game on this host, interactive UAC/tray flows, multi-hour soak, stable Windows 10/11 builds (still gated in `SUPPORTED_BUILDS`).

## Unreleased — MemFlux V4.1 Core Completion & Beta Candidate

### Added

- **Authenticated Elevated Session Helper (`action.rs`):** Named-pipe session helper (`--memflux-elevated-session-helper`) with restrictive SDDL DACL, `PIPE_REJECT_REMOTE_CLIENTS`, peer PID (`GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId`) and executable path verification, parent-process `SYNCHRONIZE` watchdog, and fixed operation enum (`Ping`, `PurgeStandbyList`, `Shutdown`).
- **Independent Monotonic Cleaner Scheduler & Quota Ledger (`cleaner.rs`, `app.rs`):** Dedicated `memflux-cleaner` thread using `Instant` monotonic timing and persistent `cleaner-quota.json` so restarts, clock shifts, and history churn cannot bypass cooldown or hourly rate limits.
- **Live GPU & PresentMon HUD (`performance.rs`, `PerformanceHud.tsx`):** DXGI adapter enumeration (`CreateDXGIFactory1`) and WDDM PDH `\GPU Engine(*)\Utilization Percentage` aggregation reporting the busiest engine (`3D`, `Compute`, `Copy`, `VideoDecode`, `VideoEncode`) without summing independent engines, plus non-injecting PresentMon ETW capture controller with explicit unavailable states.
- **Optimization Rewind (`rewind.rs`):** Versioned atomic restoration journal (`rewind-journal.json`) with compare-before-restore external-override protection and startup crash recovery.
- **PowerShift (`powershift.rs`):** Documented Win32 `PowerEnumerate`, `PowerGetActiveScheme`, `PowerSetActiveScheme`, `PowerReadFriendlyName`, and `GetSystemPowerStatus` integration with post-mutation verification and battery safeguards.
- **GameFocus & Central Coordinator (`gamefocus.rs`, `coordinator.rs`):** Conservative game detection, user profiles (`Balanced Gaming`, `Performance Gaming`, `Gaming + Streaming`, `Custom`), Alt+Tab focus debounce, streaming protection, background workload contention intelligence, and temporary session overrides.
- **Packaging & CI (`.github/workflows/ci.yml`, `tauri.conf.json`):** GitHub Actions CI/packaging pipeline, version consistency gate, SHA-256 artifact checksums, and Windows NSIS/MSI bundler configuration.

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

