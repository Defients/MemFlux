# MemFlux Implementation Plan

**Status date:** 2026-10-18  
**Product identity:** Midnight CosmoTech™ Windows standby-memory cleaner with measured intelligence. Manual cleaning is core capability; monitoring and Guardian explain and verify it.

## Core functionality recovery — V2.0

### V3 P0 continuation — 2026-10-18

The current repository is an existing Git worktree at baseline `10fd1a7`, synchronized with `origin/main` before these changes. The blank-workspace findings below describe the original Phase 0 audit, not current reality. The detailed current inventory and gates live in `MEMFLUX_PERFORMANCE_EVOLUTION_V3_PLAN.md`.

- Implemented: typed helper lifecycle and optional native status; timeout cannot become native success; retained pending-helper handle blocks duplicate launches after timeout in the current process.
- Implemented: bounded atomic action-history persistence and schema-v2 cleaner settings, with default automatic evaluation disabled.
- Implemented: deterministic dual-threshold policy, cooldown/minimum interval/hourly limit, historical manual-purge accounting, fail-closed exclusion handling, and observable current policy decision.
- Implemented: hidden dashboard receives no recurring telemetry events; native monitoring and cached snapshots continue.
- Partial: automatic policy evaluates on the monitoring cadence. Secure session authorization and unattended native execution remain incomplete. Foreground-process observation is absent, so configured exclusions produce `exclusion_status_unavailable`.
- Untested: native timeout/late completion, UAC rejection, UI interaction, updated performance footprint, and stable Windows builds.
- P1 remains gated on P0 automatic-execution acceptance.

The earlier monitoring-first phase interpretation is superseded. MemFlux is not complete without a real standby-list purge.

### Milestone 1 — Working Cleaner

**Status: implemented and native-engine verified on the single audited host; UI interaction validation partial**

- [x] Isolated `NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)` in `action.rs`.
- [x] Added exact `SeProfileSingleProcessPrivilege` enablement with `ERROR_NOT_ALL_ASSIGNED` handling.
- [x] Kept the main monitor unelevated; added a fixed-argument, one-shot UAC helper in the same executable.
- [x] Added an exact build allowlist. Build 26340 is enabled; other builds return Unsupported while monitoring continues.
- [x] Added serialized manual action execution, pre/post snapshots, 750 ms settling time, raw `NTSTATUS`, measured reduction, and bounded 100-event session history.
- [x] Added a prominent Overview cleaner control with distinct running, unsupported, failure, native-success/no-change, and measured-reduction results.
- [x] Runtime-tested the helper on Windows 11 Home Insider Preview build 26340. Unelevated direct execution returned `0xC0000061`; elevated execution returned `0x00000000`. A retained controlled sample measured standby 233,820,160 → 25,538,560 bytes and Free & Zero 10,254,319,616 → 10,510,528,512 bytes after 750 ms.
- [ ] Direct UI-driven UAC acceptance/rejection, simultaneous UI requests, and native visual interaction remain unverified because this session cannot attach to native app windows.

### Milestone 2 — ISLC Functional Parity

**Status: partially implemented; automatic native execution not accepted**

Settings, threshold evaluation, cooldown/minimum interval, maximum-frequency policy, exclusion matching, cleaner-only pause configuration, and persistent history are implemented. Secure automatic authorization/execution, foreground identity observation, independent cleaner scheduling, and native background-action verification remain required. Available physical memory must not substitute for Free & Zero Page List bytes.

## 1. Repository findings

### Original Phase 0 audit (historical)

- The workspace was effectively blank at audit time. It contained only `AGENTS.md` and the read-only reference `sources/MASTER-DIRECTIVE-V1.0.txt`.
- No package manifest, lockfile, Rust crate, application source, build scripts, assets, tests, CI, release configuration, TODOs, or earlier implementation existed.
- At the original audit the directory was not a Git worktree. Source control was subsequently initialized and pushed to `Defients/MemFlux`; this is no longer a delivery limitation.
- The host is Windows 11 Home Insider Preview, version 10.0.26340, x64. It has WebView2 155.0.4283.62, Windows SDK 10.0.26100.0, Visual Studio Build Tools, Node.js 24.21.0/npm 11.19.0, and Rust 1.99.0 MSVC.
- Local counter probing succeeded for Available Bytes, Committed Bytes, Commit Limit, three standby-cache classes, Modified Page List Bytes, Free & Zero Page List Bytes, and Page Reads/sec.

### What can be reused

- The authoritative product specification and its identity, safety, architecture, and quality constraints.
- The installed Windows/Rust/Node/Tauri-compatible toolchain.
- Nothing else: there is no existing implementation to refactor.

## 2. Architecture decisions

### Selected stack

- **Shell:** Tauri 2, one process, single-instance policy, native tray menu, WebView2 frontend.
- **Frontend:** React, strict TypeScript, Vite, semantic CSS tokens. Tailwind is deliberately deferred for the initial slice because a small hand-authored token system avoids an extra build layer while still satisfying the design-system requirement; it can be added when component volume justifies it.
- **Backend:** Rust with small modules for telemetry, Guardian policy, settings, and IPC. No service and no elevated main process.
- **Persistence:** versioned JSON for settings with validation and atomic replacement. SQLite is deferred until bounded historical sessions require relational querying.
- **Charts:** an in-house SVG trend graph to avoid a heavy chart dependency and retain accessible labels.

### Runtime flow

```text
Documented Windows APIs / PDH
             ↓
       Telemetry sampler
             ↓
  typed snapshot + availability
             ↓
 deterministic Guardian evaluator
             ↓
   Tauri event/command boundary
             ↓
      React view state + SVG
```

- Sampling belongs to the Rust runtime and continues when the dashboard is hidden.
- The UI receives validated snapshots and can request settings changes, pause/resume, or a fresh snapshot through explicit commands.
- Guardian pressure assessment remains diagnostic and separate from direct cleaner control.
- Settings are owned and validated by Rust. The frontend does not write arbitrary files.
- Closing the main window hides it to the tray; selecting Exit performs a real process exit. The tray exposes Open, Pause/Resume, and Exit.

## 3. Windows API compatibility and privilege boundaries

| Capability | Interface | Documentation / support status | Privilege | Phase 1 decision |
|---|---|---|---|---|
| Physical total/available/load and commit totals | `GlobalMemoryStatusEx` / `MEMORYSTATUSEX` | Documented desktop API, Windows XP+ | Standard user | Implement |
| Commit used/limit, physical totals, page size, process/thread counts | `GetPerformanceInfo` | Documented desktop API, Windows XP+ | Standard user | Implement and cross-check core totals |
| OS low/high-memory condition | `CreateMemoryResourceNotification`, `QueryMemoryResourceNotification` | Documented desktop API, Windows XP+; includes a neutral band | Standard user | Implement; close handles deterministically |
| Standby classes, modified list, free/zero list, page reads | PDH English counter paths | Documented performance-counter interface; individual counters can be absent | Standard user normally | Implement with per-counter unavailable state; never invent fallback values |
| Standby-list purge | `NtSetSystemInformation`, class 80, command 4; exported by `ntdll` but class/command are private and compatibility-sensitive | Runtime-verified only on build 26340 | `SeProfileSingleProcessPrivilege`, obtained through one-shot UAC helper | Implemented behind exact build allowlist |
| Process working-set trim | `EmptyWorkingSet` | Documented, but affects a process working set rather than the system standby list | Target-process rights | Do not substitute for standby clearing |
| Timer-period request | `timeBeginPeriod` / `timeEndPeriod` | Documented; process-scoped behavior on modern Windows and occlusion caveats on Windows 11 | Standard user | Research Lab only, disabled and not implemented in Phase 1 |

Key correctness boundary: `ullAvailPhys` is the authoritative available-physical value. Standby breakdown is explanatory composition inside availability; it must never be added to availability. `Page Reads/sec`, not generic page faults, is the initial hard-read pressure signal.

Authoritative references:

- <https://learn.microsoft.com/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex>
- <https://learn.microsoft.com/windows/win32/api/psapi/nf-psapi-getperformanceinfo>
- <https://learn.microsoft.com/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification>
- <https://learn.microsoft.com/windows/win32/api/memoryapi/nf-memoryapi-querymemoryresourcenotification>
- <https://learn.microsoft.com/windows/win32/api/winternl/nf-winternl-ntquerysysteminformation>
- <https://learn.microsoft.com/windows/win32/api/timeapi/nf-timeapi-timebeginperiod>
- <https://v2.tauri.app/security/capabilities/>

## 4. Implementation phases and live status

### Phase 0 — Audit and foundation

**Status: completed on the audited host**

- [x] Read the master directive in full.
- [x] Audit files, toolchain, Git state, OS, SDK, WebView2, and local memory counters.
- [x] Verify documented monitoring interfaces and identify the compatibility-sensitive intervention boundary.
- [x] Record architecture, risks, verification plan, and measurable gates.
- [x] Add persistent project engineering rules without removing mirror rules.
- [x] Scaffold and build Tauri/React/Rust application.
- [x] Add required architecture, memory-model, compatibility, security, testing, performance, and roadmap documentation.

**Gate result:** passed on the audited host. Dependency install/audit, frontend production build, Rust format/Clippy/test, and no-bundle Tauri debug and optimized release builds completed. Native visual QA remains unavailable because this Codex session exposes no attachable Windows-app surface.

### Phase 1 — Functional monitoring MVP

**Status: implemented; partially runtime-verified**

- Real documented Windows collection with source, timestamp, interval, status, and error metadata.
- Adaptive Rust sampling (dashboard cadence versus background cadence) independent of rendering.
- Overview with memory condition, physical availability, active-use estimate, commit pressure, standby/free/modified composition, page-read activity, and sensor availability.
- Explainable diagnostic Guardian with sustained-condition history, confidence gating, Observe/Recommend outcomes, and reason codes.
- Versioned settings persistence for Guardian enabled state, close-to-tray, sampling intervals, reduced motion, and compact visual density.
- Native system tray with open, pause/resume, and explicit exit.
- Diagnostic logging without user secrets or uploaded telemetry.
- Cohesive, responsive, keyboard-accessible Midnight CosmoTech™ shell and lightweight trend graph.

**Gate result:** the standard-user native process launches, creates the tray before logging readiness, emits real host metrics for 60+ consecutive samples, reports counter warm-up honestly, persists settings in automated native tests, and enforces one instance. Direct visual inspection, tray menu clicks, close-to-tray/resume interaction, hidden-cadence timing, DPI/accessibility matrix, and explicit tray Exit remain unverified in this environment.

### Phase 2 — Automatic cleaner policy

**Status: planned**

- Implement opt-in ISLC-style standby/free-zero threshold evaluation.
- Add cooldown, minimum interval, maximum frequency, exclusions, action serialization, cleaner pause, and persistent bounded history.
- Retain the narrow helper and exact native operation; never expose generic working-set trimming as standby clearing.

### Phase 3 — Advanced intelligence

**Status: planned**

- Bounded historical storage, Memory Explorer, sessions and export, process/game awareness, PresentMon investigation, and Timer Lab.

### Phase 4 — Product polish and release

**Status: planned**

- Compact monitor, onboarding, accessibility/DPI matrix, installer, signing/update plan, long-duration reliability, and stable Windows 10/11 validation.

## 5. Risk assessment

| Risk | Impact | Mitigation / current decision |
|---|---|---|
| Insider-only local host is not representative | Runtime success may not generalize to stable Windows 10/11 | Mark local results precisely; require stable-build matrix before beta |
| PDH counter absence or localization | Partial telemetry | Use English counter registration and per-metric availability; core APIs remain functional |
| Counter semantics and double counting | Misleading dashboard | Treat available as inclusive; chart mutually exclusive estimates only; document formulas |
| Compatibility-sensitive standby action | Breakage, elevation, user disruption | Isolated adapter, exact build allowlist, one-shot helper, raw status, serialization, and measured verification |
| Tauri/WebView idle overhead | Missed lightweight budget | Release visible measurement met the CPU target at 0.131% normalized combined CPU over 20.4 seconds, but exceeded the exploratory memory budget at 249.95 MiB combined private bytes / 426.37 MiB combined working set. Treat memory budget as failed and investigate WebView2 process options or shell alternatives before beta. |
| Close-to-tray ambiguity | User trust problem | Explain behavior in settings; tray Exit always exits; window close is configurable |
| Settings corruption/interrupted write | Lost preferences or startup failure | Versioning, validation, defaults, atomic replacement, diagnostic warning |
| Sleep/resume and counter discontinuity | False trends or trigger | Timestamp samples, reject invalid intervals, reset rolling windows after long gaps |
| Accessibility/DPI | Unusable controls or clipped UI | semantic HTML, focus visibility, reduced motion, responsive layout; matrix testing pending |
| No Git metadata | Weak change traceability | Report explicitly; do not fabricate history; recommend initializing source control |

## 6. Verification strategy

### Automated

- TypeScript strict compilation and production frontend build.
- Rust formatting and Clippy with warnings denied where practical.
- Rust unit tests for arithmetic, missing/failed metrics, Guardian thresholds/sustained windows, resource-state interpretation, settings validation/migration, and unsupported actions.
- Frontend tests for formatting, charts, loading/degraded states, navigation, and settings interactions.
- Dependency audit where compatible with the installed toolchain.

### Native integration

- Compare core snapshot totals to PowerShell/CIM and Performance Monitor values within sampling tolerance.
- Verify all Win32/PDH errors are surfaced as unavailable/degraded and include source/error detail.
- Verify the low/high resource-notification handles are closed and no sampling thread busy-waits.
- Run as a standard user and confirm no UAC prompt for monitoring.
- Hide/show through tray, pause/resume, settings restart, explicit exit, and second-instance focus behavior.

### Visual and interaction

- Capture the actual Tauri window and inspect Overview, degraded/loading, settings, narrow width, keyboard focus, and reduced motion.
- Check 100%, 125%, 150%, and 200% Windows scaling when test infrastructure allows.

### Reliability and performance

- Sample for at least 30 minutes for Phase 1 smoke and later 8+ hours for beta.
- Record process working set, private bytes, handles, and average CPU with dashboard visible and hidden.
- Treat the <0.5% background CPU and 50–100 MB combined memory figures as targets until actual measurements are recorded.

## 7. Measurable acceptance criteria

### Phase 1 acceptance

1. `npm run build`, `cargo test`, `cargo clippy`, and `npm run tauri build -- --debug` complete successfully on the audited host.
2. A standard-user native launch produces at least 60 consecutive real snapshots without fabricated values or an unhandled collection error.
3. Physical total, available, commit used, and commit limit match OS tools within the expected one-sample timing variance.
4. Every displayed metric includes a backend source/status; unavailable PDH metrics render as unavailable rather than zero.
5. Guardian emits stable pressure reason codes and stays separate from explicit cleaner control.
6. A single transient low sample cannot produce a recommendation; a recommendation requires the configured sustained window and valid core telemetry.
7. Settings survive a clean restart; malformed settings fall back safely with a logged diagnostic.
8. Closing to tray leaves one sampler running, Open restores the window, Pause stops new sampling work, Resume restarts it, and Exit terminates the process.
9. Monitoring never requests elevation; the cleaner requests UAC only for its fixed one-shot native operation and opens no shell or network path.
10. The dashboard remains usable at a 900×620 logical window and supports keyboard navigation, visible focus, and reduced motion.

### Phase 1 verification snapshot — 2026-10-18

| Check | Result | Evidence / limitation |
|---|---|---|
| Frontend production build | **Passed** | Vite 8.3.0; 22 modules; 249.82 kB JS and 23.22 kB CSS before gzip |
| Frontend unit tests | **Passed** | 2/2 Vitest tests |
| Rust format | **Passed** | `cargo fmt --check` |
| Rust lint | **Passed** | Clippy across all targets with warnings denied |
| Rust tests | **Passed** | 9/9, including live documented Windows collector and atomic settings tests |
| Dependency audit | **Partial** | npm production audit: 0 vulnerabilities; `cargo-audit` is not installed, so Rust advisory audit was not run |
| Debug native build | **Passed** | 13.39 MiB no-bundle executable |
| Optimized native build | **Passed** | 3.50 MiB no-bundle executable |
| Native launch | **Passed** | Standard process, titled main window, no UAC observed |
| Sustained telemetry | **Passed** | Both debug and release reached logged sample 60 with `collection=Monitoring`; Page Reads/sec warmed from unavailable to a real rate |
| OS cross-check | **Passed within sampling drift** | Available, commit limit/use, three standby classes, and page reads compared with local Performance Monitor/CIM |
| Single instance | **Passed** | Second executable launch left exactly one MemFlux process |
| Release visible CPU | **Passed exploratory target** | 0.131% normalized combined process-tree CPU over 20.4 seconds on 24 logical processors |
| Release visible memory | **Failed exploratory target** | 249.95 MiB combined private bytes; 426.37 MiB combined working set; native process working set alone 33.36 MiB |
| Visual and tray interaction QA | **Untested** | Native computer-use attachment is unavailable in this Codex session; source/DOM compilation is not treated as visual proof |
| Stable Windows matrix | **Untested** | Only Windows 11 Home Insider Preview build 26340 was available |

Acceptance criteria 1–6 and 9 have direct automated/runtime evidence. Criterion 7 is covered by native settings tests but not a UI-driven restart. Criterion 8 is implemented, and tray creation/single-instance are runtime-confirmed, but interaction steps remain untested. Criterion 10 is implemented in CSS/semantics but awaits actual visual, keyboard, DPI, and assistive-technology testing.

### Not accepted as Phase 1 evidence

- Mock data, screenshots without native data provenance, successful compilation without launch, or a frontend-only preview.
- A claimed memory improvement based only on increased free memory.
- A standby action implemented through an undocumented magic information class without compatibility and privilege validation.

## 8. Immediate execution order

1. Scaffold the minimal Tauri 2 + React + strict TypeScript project and lock dependencies.
2. Implement and test the Rust metric schema, documented Windows collector, PDH degradation, and Guardian evaluation.
3. Add versioned settings and the adaptive sampling runtime.
4. Add tray/single-instance/window lifecycle.
5. Build the Overview and Settings experience against live IPC/events.
6. Add required project documentation and status labels.
7. Run automated builds/tests, launch the real desktop app, inspect it, fix failures, and record actual outcomes here.
