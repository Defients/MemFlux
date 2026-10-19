# MemFlux V4.2 Execution Plan — Reliability, Recovery & Performance Proof

**Audit & execution date:** 2026-10-19
**Baseline revision:** `9e289c5283e9bba5cc7008f961de54f531712479` (`main`, clean worktree — verified against last-reviewed HEAD)
**Audited host:** Windows 11 Home Insider Preview `10.0.26340.9750`, x64, NVIDIA RTX 4060 Ti (per V4.1 ledger)
**Specification authority:** MEMFLUX V4.2 — CosmoTech™ Master Engineering Directive
**Supersedes priorities, not history:** `MEMFLUX_V4_1_BETA_EXECUTION_PLAN.md`, `MEMFLUX_PERFORMANCE_EVOLUTION_V3_PLAN.md`, and `MEMFLUX_OMNIFLUX_V4_IMPLEMENTATION_PLAN.md` remain the record of prior phases.

---

## 1. Implemented feature inventory (verified state at HEAD)

| Subsystem | Files | State |
|---|---|---|
| Standby purge (native) | `action.rs` | Implemented. `NtSetSystemInformation(80, 4)` + `SeProfileSingleProcessPrivilege`; allowlist `SUPPORTED_BUILDS = [26340]`; raw NTSTATUS propagation; typed lifecycle. |
| One-shot elevated helper | `action.rs` | Implemented. `--memflux-elevated-purge-standby`, UAC `runas`, 60 s wait, pending-handle tracking, typed timeout (never native success). |
| Session helper IPC | `action.rs` | Implemented. `--memflux-elevated-session-helper`, named pipe, SDDL DACL (`SYSTEM`/`BA`/current-user SID), `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, peer PID + image-path checks, fixed op enum, 4 h bound, parent watchdog. **Defects found — see §3.** |
| Automatic cleaner | `cleaner.rs`, `app.rs` | Implemented. Independent `memflux-cleaner` thread, dual thresholds (standby ∧ free&zero), cooldown/min-interval/hourly cap, fail-closed exclusions, durable `cleaner-quota.json`. **Clock-edge defects — §3.** |
| Telemetry | `telemetry/` | Implemented. `GlobalMemoryStatusEx`, `GetPerformanceInfo`, resource notifications, PDH standby/free-zero/modified/page-reads, per-metric availability. |
| CPU/GPU/FPS HUD | `performance.rs` | CPU `GetSystemTimes` delta (single-group only), DXGI adapters + WDDM GPU Engine busiest-engine, PresentMon external capture. **PresentMon defects — §3.** |
| PowerShift | `powershift.rs` | Implemented. PowrProf enumerate/read/set, battery guard, journal-backed verified apply. |
| Optimization Rewind | `rewind.rs` | Implemented v1. Atomic journal, compare-before-restore, startup recovery of `Active` entries. **Uncertain-outcome defect — §3.** |
| GameFocus | `gamefocus.rs`, `coordinator.rs` | Implemented. Blocklist-gated detection, profiles, Alt+Tab debounce, protected workloads, session restore. |
| Settings/persistence | `settings.rs`, `app.rs` | Schema v4, validated, atomic replace; bounded action history (100). |
| Shell | `app.rs` | Tray (Open / Pause / Exit), close-to-tray, single-instance. |
| Frontend | `src/` | React dashboard, 5 nav pages, Rewind table, settings. |
| CI/packaging | `.github/workflows/ci.yml`, `tauri.conf.json` | Windows CI: version gate, npm test/build, fmt, clippy, cargo test, tauri build (NSIS+MSI), SHA-256. |

## 2. Verified vs. unverified functionality

- **Verified on this host (V4.1 evidence):** unelevated helper → `0xC0000061`; elevated purge → `0x0` with measured standby reduction; named-pipe Ping/Purge/Shutdown round-trip; live PowrProf switch + Rewind restore; DXGI + WDDM collection; 33 Rust + 5 frontend tests; NSIS/MSI build.
- **Unverified:** stable Win10 `19045` / Win11 `22631`/`26100` builds; real PresentMon game capture; UAC interactive flows; long soak; Ultra-Light mode; Mini HUD; Stutter Detective (not yet implemented).

## 3. Source-level defects & risk-ranked findings (audit results)

| # | Sev | File | Finding |
|---|---|---|---|
| F1 | **P0** | `app.rs:160-170`, `:1078-1081`, `:1145-1162` | `request_shutdown` never restores Rewind-owned interventions. Tray Exit and close-without-tray both abandon an active PowerShift/GameFocus-applied power scheme until next-startup recovery. |
| F2 | **P0** | `powershift.rs:236-249`, `rewind.rs` | Mutation-successful/verification-failed is journaled `Failed` → excluded from startup recovery (only `Active` restored). `Pending` crash entries likewise dropped. OS change can be silently abandoned. |
| F3 | **P0** | `action.rs:603-607` | Session ID `"{pid:08x}{time_nanos:024x}"` is time-derived, not a cryptographic secret. Directive requires a Windows CSPRNG authenticator. |
| F4 | **P0** | `action.rs:817-894` | `send_session_request`: connect timeout does not bound later synchronous `WriteFile`/`FlushFileBuffers`/`ReadFile` — a stalled helper blocks a controller thread indefinitely. |
| F5 | **P0** | `action.rs:977-1082` | Helper `ReadFile`/`WriteFile`/`FlushFileBuffers` are unbounded; watchdog unblocks `ConnectNamedPipe` only, not an in-progress connected `ReadFile`. No request-id sequence enforcement (replay/unordered requests accepted). |
| F6 | **P0** | `action.rs:799-812` | Session IPC failure during purge reports `late_completion_possible: false`; the request may already have been delivered — late/duplicate purge accounting is wrong. |
| F7 | **P0** | `cleaner.rs:130-143`, `:362-387` | Quota ledger is wall-clock dependent for restored entries: clock rollback → future timestamps block indefinitely (saturating_sub = 0); forward jump → limits bypassed. No suspend-aware monotonic floor. |
| F8 | **P1** | `performance.rs:328-339` | PresentMon launched with `--stop_existing_session` (kills unrelated captures) and legacy `--no_top`; no unique `--session_name`; no version/flag detection. |
| F9 | **P1** | `performance.rs:354-378` | CSV parsing via `line.split(',')` — no quoting/escaping/header-map handling; no ProcessID validation; PID reuse not guarded (no creation-time check); stale frames could attach to a recycled PID. |
| F10 | **P1** | `performance.rs:913-928` | Empty WDDM counter set reports `0.0` utilization as *available*. Should be unavailable until a genuine zero measurement is established. |
| F11 | P2 | `app.rs` | No `accepting_requests` gate: purge/GameFocus/PowerShift commands can still run once shutdown has begun. |
| F12 | P2 | `rewind.rs:129-131` | Journal eviction drops the oldest entry even if unresolved (non-terminal statuses must outlive the retention limit). |
| F13 | P2 | `performance.rs` | PresentMon child `kill()` orphans the ETW session for the process lifetime edge cases; no graceful CTRL event or `--terminate_on_proc_exit`. |

## 4. V4.2 architecture decisions

1. **Rewind V2 state model** (smallest coherent set): `prepared → applied_verified | outcome_uncertain | failed_before_mutation`; restore side `restoration_pending → restored | external_change_preserved`; startup-derived `recovery_required`; `irreversible` unchanged. `restoring` is not persisted (in-flight only). Schema v1 → v2 migration maps `failed → outcome_uncertain` (must be re-inspected), `conflict_skipped → external_change_preserved`, `pending → prepared` (reconciled by live-state read at startup).
2. **Shutdown lifecycle:** `Running → ShutdownRequested (stop accepting) → StoppingCapture → RestoringSystemState → StoppingPrivilegedHelper → FlushingPersistence → ShutdownComplete|ShutdownDegraded`. All waits bounded (action-drain ≤ 3 s, helper ≤ 2 s, coordinator lock bounded retry); journal persistence is the fallback — anything unrestored is resolved by startup recovery. `RunEvent::Exit` catch-all makes shutdown idempotent (`AtomicBool`).
3. **Session-helper IPC:** session id + pipe suffix from `BCryptGenRandom` (32-hex-char secret); client and server both use `FILE_FLAG_OVERLAPPED` + event + bounded `WaitForSingleObject` + `CancelIoEx`; server enforces strictly increasing `request_id`; IPC failure during a purge reports `late_completion_possible: true`.
4. **Quota ledger:** persisted wall timestamps are converted at load into `Instant`-anchored entries (`age = clamp(now − ts, 0, 1 h)`); future timestamps clamp to "just ran" (conservative, self-recovering); in-session entries keep real `Instant`. Rollback cannot bypass, forward jumps cannot bypass in-session, restored entries never outlive one hour of runtime plus recorded wall age.
5. **PresentMon provider:** probe installed binary (`--help` output) for supported flags; unique `--session_name MemFlux_<rand>`; drop `--stop_existing_session`; `--terminate_on_proc_exit` when supported; real CSV parser (quoted fields, header-name indexing, per-PID/swap-chain filtering); PID creation-time identity tracking.
6. **Ultra-Light Tray (Phase D):** measure first. Tier 1 = suspended frontend rendering (already emits nothing when hidden) + heavy-view unload hints. Tier 2 (opt-in) = destroy `main` webview via `window.destroy()` and recreate on demand through `WebviewWindowBuilder`; gate behind a measured win; native runtime unaffected either way.
7. **Mini HUD (Phase E):** second small `WebviewWindow` (`label: "minihud"`, transparent, always-on-top, skip-taskbar) fed by the *same* telemetry sampler via a dedicated `hud://frame` event; capabilities extended to that window label only. Three presets: pill, stack, gaming.
8. **Stutter Detective (Phase F):** consumes ring buffer of `(qpc_time, ms_between_presents, dropped)` from the validated PresentMon path only; rolling median + MAD detector; events correlated to the *coarse* (≥1 s) system telemetry with explicit uncertainty; confidence categories enforced.
9. **GPU empty-set:** report `unavailable`, not `0.0`.

## 5. Dependency graph

```
settings ──► app(AppState) ──► coordinator ──► powershift ──► rewind(journal)
                 │                    │                          ▲
                 ├─► cleaner(policy)──┤ (cleaner_override)       │ live-state fns
                 ├─► action(helper IPC, capability)
                 ├─► performance(CPU/GPU/PresentMon)
                 └─► telemetry(collector)
shutdown: tray/close/RunEvent::Exit ──► AppState::execute_shutdown ──► coordinator::shutdown_and_restore ──► rewind.restore_all_eligible
PresentMon hardening ◄── Stutter Detective ◄── Performance Proof
```

## 6. Execution phases & acceptance gates

| Phase | Scope | Gate | Status |
|---|---|---|---|
| **A** | This ledger + baseline test run | Ledger accurate; baseline green. | ✅ Done (baseline 33/33) |
| **B** | F1, F2, F3-F7, F11, F12, F10 (restoration, shutdown, IPC, quota) | `cargo test`/`clippy`/`fmt` green; deterministic tests cover each defect; a normal exit path demonstrably invokes restoration. | ✅ Implemented + tested (deterministic). Live exit-with-intervention restore remains unverified on hardware. |
| **C** | F8, F9, F13 PresentMon provider + CSV + identity | Fixture parser tests; flag-probe verified against installed binary (or marked blocked if absent). | ✅ Implemented + fixture-tested. Live ETW capture vs. a real game unverified. |
| **D** | Ultra-Light Tray Mode | Measured private/working-set reduction vs hidden baseline, else labeled experimental. | ✅ Implemented (webview destroy/recreate, native keeps running). Resource-delta measurement on this host pending — label experimental until measured. |
| **E** | Mini HUD window | Real metrics, presets, persistence, click-through recovery. | ✅ Implemented (independent `minihud` webview, presets, opacity, click-through, position persistence). Visual/drag QA unverified. |
| **F** | Stutter Detective + Performance Proof | Real frame-time events only; no capture → feature reports unavailable. | ✅ Implemented + unit-tested (baseline gating, severity, dropped, floor; proof measurable/inconclusive/insufficient). Requires live PresentMon session to produce real events. |
| **G** | Beta qualification | Per §15 matrix; unverified stays unverified. | ⏳ Pending live-host verification. |

## 7. Hardware/Windows compatibility matrix

Unchanged from V4.1: **confirmed** = Insider build 26340 x64 (this host); **unverified** = 26100, 22631, 19045 — all gated off in `SUPPORTED_BUILDS` until runtime evidence. Multi-processor-group systems: CPU metric unavailable (unchanged). PresentMon: requires installed binary + ETW capture permission; Mini HUD is read-only, standard user.

## 8. Test & measurement strategy

- Deterministic Rust tests per defect: rewind uncertain-recovery, retention-vs-unresolved, shutdown ordering (state-machine level), session-id entropy/format, request-id sequencing, IPC timeout paths, quota clock edges, GPU empty-set, CSV fixtures.
- Native re-verification on this host: purge round-trip via session helper; power scheme restore on simulated exit; journal recovery after `Pending`-state kill.
- Resource protocol (Phase D/E): private bytes + working set + handles/threads/CPU for `native`, per WebView2 tree; states: visible / hidden / ultra-light / mini-HUD / ultra-light+HUD; ≥60 s windows.
- Performance Proof (Phase F): paired baseline vs optimized captures; outcome classifications only (no significance claims).

## 9. Current blockers

- Stable Windows 10/11 hosts unavailable → builds remain `unverified`/gated.
- PresentMon may not be installed → Phase C live gate may be blocked; fixture tests still land.
- No interactive desktop automation → tray/UAC/visual QA recorded as blocked, not passed.

## 10. Continuation checklist

- [x] Phase B complete → `CHANGELOG.md` updated; `cargo test` 54/54, `clippy` 0 warnings, `tsc -b` + `vite build` green, vitest 5/5.
- [ ] Live-test: exit MemFlux with a PowerShift intervention active → confirm restore without restart.
- [ ] Live-test: kill -9 during a power change → journal recovery correctness.
- [ ] Live-test: session-helper purge round-trip on build 26340 with pre/post counter evidence.
- [ ] Measure Ultra-Light Tray Mode private-bytes/working-set delta vs. hidden-dashboard baseline (§8 resource protocol).
- [ ] Live PresentMon game capture → Stutter Detective event correlation spot-check.
- [ ] Performance Proof run with a real workload (baseline → PowerShift → compare) and honest result recording.
- [ ] `docs/SECURITY.md` / `docs/TESTING.md` updates for the new IPC bounds and shutdown lifecycle.

## Atlas integrity follow-up — 2026-10-19

The Atlas continuation supersedes Phase F interpretation/timing and IPC/HUD integrity claims above: removed unused recent_frames (strict Clippy blocker); drained canceled overlapped I/O; reset events; waited for client release before pipe disconnect; separated QPC event time from CSV arrival; withheld current FPS on unavailable/stale mapping; neutralized CPU/GPU/RAM Proof deltas and removed the unjustified noise heuristic; added warm-up/duration floors and final native-memory report hydration; protected settings saves and Mini HUD freshness. See the Atlas ledger for measured tests and remaining gates. Live PresentMon gameplay, full statistical experiment qualification and whole-app overhead remain unverified.
