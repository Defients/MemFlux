# MemFlux Performance Evolution V3 Plan

Updated: 2026-10-18  
Audited revision: `10fd1a7` (`main`, synchronized with `origin/main`)

## 1. Current implementation inventory

The repository is a compact Tauri 2 application rather than a broad scaffold. The working baseline consists of a React 19/TypeScript dashboard, Rust telemetry and policy runtime, a native tray, versioned JSON settings, and a Windows-only standby-list purge. The worktree was clean at audit time. Files under `sources/` remain read-only.

Implemented and retained:

- Documented standard-user memory telemetry from `GlobalMemoryStatusEx`, `GetPerformanceInfo`, memory resource notifications, and English PDH counters.
- Separate Guardian pressure assessment with sustained-condition hysteresis.
- Adaptive foreground/background sampling, tray lifecycle, close-to-tray, and single-instance behavior.
- Manual full standby purge through the fixed operation `NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)` after enabling `SeProfileSingleProcessPrivilege`.
- One-shot `runas` helper with no arbitrary command, path, or file protocol.
- Pre/post standby and Free & Zero measurements, native status display, serialization, and bounded session history.
- Midnight CosmoTech interface with a prominent manual cleaner control.

Missing or incomplete:

- The helper transport incorrectly stores `WAIT_TIMEOUT` (`0x00000102`) as though it were a native `NTSTATUS`, which can report a timeout as accepted.
- Helper launch, UAC cancellation, wait failure/timeout, exit-code retrieval, and native completion are not typed separately.
- Action history is not persistent.
- Automatic threshold cleaning, cooldown/frequency policy, exclusions, cleaner-only pause, and secure unattended privilege lifecycle are absent.
- Only Windows 11 Home Insider Preview build 26340 is allowlisted and runtime-qualified.
- No GameFocus, PowerShift, ProcessPilot, VRAM Guard, FrameLab, Memory Hunter, or Startup Manager implementation exists.
- The prior visible WebView2 process-tree measurement exceeded the exploratory memory target and needs renewal.

## 2. Capability matrix

| Capability | Implementation | Automated tests | Windows runtime evidence | Current classification |
|---|---|---|---|---|
| Core memory telemetry | Implemented | Yes | Build 26340 | Implemented / locally tested |
| Manual standby purge | Implemented | Partial | Build 26340, elevated status `0x00000000` with measured reduction | Confirmed supported on build 26340 only |
| Unsupported-build gating | Implemented | Yes | Not tested on another host | Implemented / runtime untested |
| Helper lifecycle classification | Implemented: typed transport/native outcomes, pending-helper duplicate gate | Regression tests | Native timeout not induced | P0-A implemented / runtime partial |
| Persistent action history | Implemented: bounded atomic JSON journal | Round trip and retention | Restart loading tested with local files | Implemented / UI restart untested |
| Automatic cleaner policy | Implemented evaluation and schema-v2 settings | Threshold, authorization, cooldown, rate, exclusion tests | Native automatic execution absent | P0-B partial |
| Secure unattended authorization | Absent | No | No | P0 architecture required |
| GameFocus / PowerShift | Absent | No | No | P1 planned; must not precede P0 gate |
| Other V3 feature families | Absent | No | No | P2–P4 planned |

## 3. Architecture decisions

1. Keep telemetry/UI unelevated and preserve the fixed native operation adapter.
2. Replace integer-overloaded action results with typed transport/lifecycle outcomes. A native status exists only after the helper process is signaled and `GetExitCodeProcess` succeeds, or after an in-process elevated call returns.
3. A timeout reports `helper_wait_timed_out`, carries no native status, and states that late completion remains possible. The parent does not forcibly terminate the helper.
4. Retain the exact tested-build allowlist, but expose compatibility independently from runtime privilege and mechanism state: confirmed supported, confirmed unsupported, unverified, privilege unavailable, and mechanism unavailable.
5. Implement cleaner policy as a deterministic module separate from Guardian. Inputs are standby bytes, Free & Zero bytes, user settings, exclusions, cooldown/frequency history, authorization readiness, and action serialization.
6. Automatic cleaning remains disabled by default. It must never treat available physical memory as Free & Zero memory.
7. Do not allow automatic triggers to create surprise UAC prompts. Automatic execution will require an explicitly authorized, narrowly scoped session helper with authenticated IPC and controlled termination. Until that boundary is implemented and reviewed, policy may report authorization required but must not pretend an automatic action occurred.
8. Store bounded action records locally with atomic replacement. Persist facts and native outcomes; do not persist invented performance conclusions.
9. Do not start P1 system interventions until P0’s cleaner gate passes. Read-only discovery may be designed earlier, but no placeholder controls will be exposed.

## 4. Windows API compatibility and privilege boundaries

- Native operation: `NtSetSystemInformation`, information class 80, command 4. Microsoft documents the export and `NTSTATUS` return contract but not this private memory-list class/command in the public SDK. It remains compatibility-sensitive.
- Privilege: `SeProfileSingleProcessPrivilege`, enabled only in the elevated action process/session.
- Elevation: `ShellExecuteExW` with `runas`. `ERROR_CANCELLED` is a UAC cancellation, not an `NTSTATUS`.
- Wait: `WaitForSingleObject` returns a Win32 wait result. `WAIT_TIMEOUT` means the helper was still nonsignaled; it neither proves termination nor provides a native result.
- Exit status: `GetExitCodeProcess` is read only after a signaled wait. Its value is the fixed helper’s returned native status.
- Monitoring remains standard-user and uses documented APIs.

Compatibility states:

- **Confirmed supported:** exact edition/build/mechanism has controlled runtime evidence.
- **Confirmed unsupported:** controlled evidence shows the mechanism is unavailable or rejected for that configuration.
- **Unverified:** mechanism may exist but the build is not qualified; action disabled.
- **Privilege unavailable:** compatible mechanism exists but the required privilege could not be enabled.
- **Mechanism unavailable:** required export or platform facility is absent.

Current matrix: Windows 11 Home Insider Preview build 26340 is the sole confirmed-supported build. No stable Windows 10 or 11 build is yet qualified.

## 5. Prioritized implementation phases

### P0-A — Native correctness recovery

- Typed helper/native outcomes.
- Accurate UAC, launch, wait, timeout, exit-code, privilege, native, and measurement classifications.
- Regression tests proving `0x102` cannot become native success.
- Explicit late-completion reporting and duplicate-request serialization.

### P0-B — Durable cleaner foundation

- Persistent bounded action history.
- Versioned automatic-cleaner settings and validation.
- Deterministic threshold/cooldown/rate/exclusion policy with unit tests.
- Cleaner-only pause and clear last-decision state.
- Tray-compatible evaluation independent of dashboard visibility.

### P0-C — Secure automatic execution

- Explicit user-started elevated session helper.
- Authenticated local IPC accepting only enumerated memory-list operations.
- Session state, controlled shutdown, failure recovery, and no repeated UAC prompts.
- Native tests for trigger, non-trigger, cooldown, rate limit, exclusion, simultaneous requests, cancellation, failure, late completion, shutdown, and background operation.

### P1 — Small GameFocus + PowerShift slice

Only after all P0 gates pass: verified foreground process identity and manual game profiles; read-only contention inventory; documented power-state discovery; one user-approved reversible power change with exact restoration and durable journal.

### P2–P4

ProcessPilot + Memory Hunter; then VRAM Guard + FrameLab; then Startup Manager and release maturity. Each follows evidence, consent, restoration, and performance-validation gates in the V3 directive.

## 6. Risks

| Risk | Consequence | Control |
|---|---|---|
| Private memory-list class changes | Unsafe or ineffective purge | Exact tested-build qualification; disabled when unverified |
| Helper times out but later completes | Duplicate actions or false result | Typed timeout, late-completion warning, no claimed native status, serialization, session design |
| Automatic UAC prompts during games | Disruption and loss of trust | Never prompt from a background trigger; require explicit authorized session |
| Corrupt/stale history | Misleading audit trail | Versioned bounded schema, atomic writes, safe fallback |
| Counter absence | Incorrect policy trigger | Require valid standby and Free & Zero metrics; skip otherwise |
| Cache refills quickly | Purge loop | Cooldown, minimum interval, hourly limit, and action serialization |
| WebView2 footprint | Optimizer consumes excessive memory | Re-measure visible/hidden process tree; suspend hidden rendering; avoid high-frequency events |
| Overbroad performance controls | Regressions/security impact | P1+ uses documented APIs, explicit consent, exact restoration, and no prohibited tweaks |

## 7. Verification strategy

- Rust unit tests for typed outcome mapping, capability gating, settings migration/validation, policy triggers/skips, cooldown, frequency, exclusions, serialization, and durable history.
- Frontend strict build and tests for settings/result rendering and unsupported/degraded states.
- Format and Clippy with warnings denied.
- Debug and optimized Tauri builds.
- Controlled Windows native tests record edition/build, elevation state, mechanism, raw native status, before/after counters, timing, and comparison method.
- Renew visible and hidden process-tree CPU/private-bytes/working-set/handle measurements; separate native and WebView2 processes.
- Do not generalize build 26340 evidence to stable Windows 10/11.

## 8. Performance budgets

Initial budgets pending representative measurement:

- Background normalized CPU: target below 0.5% over a five-minute steady-state sample.
- Sampling: no tighter than configured policy; default hidden cadence at least five seconds.
- History: bounded at 100 action records; writes coalesced where practical.
- No unbounded telemetry/event queues.
- Hidden dashboard: decorative animation suspended through visibility/reduced-motion behavior.
- Memory: renew baseline before setting a release gate; the historical 249.95 MiB private / 426.37 MiB working-set process-tree sample is a failed exploratory target, not an accepted budget.

## 9. Acceptance gates and live status

| Gate | Status |
|---|---|
| Timeout can never be reported as native success | Implemented; production conversion regression tested |
| Helper transport and native result are typed separately | Implemented; optional raw status and explicit lifecycle |
| Manual purge remains functional with honest pre/post reporting | Retest required after correction |
| Build 26340 native mechanism remains qualified | Previously verified; regression retest required |
| Automatic cleaner is opt-in, threshold-correct, bounded, serialized, exclusion-aware, observable, and tray-capable | Not implemented |
| Automatic execution has explicit secure authorization without surprise UAC | Not implemented |
| Persistent bounded history survives restart | Implemented; file round trip/retention tested, UI restart untested |
| Stable Windows compatibility matrix | Blocked on representative test environments |
| P1 authorization | Blocked until P0 gates pass |

This plan is a live engineering record. Completed, partial, untested, and blocked states must be updated as evidence changes.

## 10. Continuation evidence and remaining boundary

- Pending helpers retain an owned process handle after timeout/wait failure. The current process blocks further launches until the handle is signaled, then logs any late exit status without inventing attributable measurements. A missing returned handle blocks launches for the current process lifetime. Parent exit and restart reconciliation remain unverified/unimplemented; no forced termination was added.
- History writes are serialized under the history lock. Settings save and in-memory application are serialized under the control lock. Same-millisecond action requests receive distinct IDs.
- A successful request with absent post-action measurement now reports `request_succeeded_measurement_unavailable` rather than asserting zero observed change.
- Exclusions use exact executable filename/path matching and fail closed if foreground identity cannot be observed. Runtime foreground observation is pending.
- Accepted manual history participates in policy cooldown/rate evaluation. Automatic execution remains disabled; no unattended UAC flow exists.
- Policy status is emitted with visible telemetry frames. Hidden telemetry still updates native cache/tray but does not cause recurring frontend rendering. Resource savings are unmeasured.
- The P0 acceptance gate remains **partial**. P1 system changes have not started. Next implementation is authenticated session-helper IPC, independent cleaner scheduling, foreground observation, and controlled native automatic tests.

Verification of the continuation: 21/21 Rust tests passed on the local Windows host, including live standard-user telemetry; Clippy across all targets with warnings denied and Rust format checks passed after correcting lint findings. The final TypeScript/Vite production build and 2/2 frontend tests passed. Native UAC/timeout/late-completion scenarios are not claimed as tested. Registry inspection reports build 26340, revision 9750; CIM edition queries are denied in the restricted environment. The earlier edition/native-purge evidence remains historical and has not been repeated by this continuation.

The optimized no-bundle Tauri release build passed (3m 47s). Executable: `src-tauri/target/release/memflux.exe`, 3,751,936 bytes, SHA-256 `31276F65D3F37171061F0BAFF92490A393A374B7A77CBD0F3879867EA1893B25`. A direct standard-user invocation of its exact fixed helper argument returned `0xC0000061` (`STATUS_PRIVILEGE_NOT_HELD`), confirming the privilege-denial path on the rebuilt binary. No UAC elevation or successful purge was performed in this continuation.
