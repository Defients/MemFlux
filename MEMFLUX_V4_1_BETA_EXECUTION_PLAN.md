# MemFlux V4.1 Beta Execution Plan

**Audit & Execution Date:** 2026-10-19  
**Baseline Revision:** `c0f7c087f62ec5fd2ef1f64deeaaae0b3b0ff7bb` (`main`)  
**Audited Host:** Windows 11 Home Insider Preview (`10.0.26340.9750`, x64, 24 logical processors), NVIDIA GeForce RTX 4060 Ti (`LUID 0x00000000_0x00017a11`)  
**Specification Authority:** MEMFLUX V4.1 — Core Completion & Beta Readiness (CosmoTech™)

---

## 1. Current Implementation Inventory (V4.1 Completed State)

All six V4.1 core deliverables plus the Optimization Rewind foundation and Central Optimization Coordinator have been implemented and verified:

1. **Core Standby Memory Cleaner (`src-tauri/src/action.rs`, `src-tauri/src/cleaner.rs`, `src-tauri/src/app.rs`):**
   - Retains the validated `NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)` operation with `SeProfileSingleProcessPrivilege` and build allowlist gating (`SUPPORTED_BUILDS = &[26340]`).
   - Adds an explicitly authorized elevated session helper (`--memflux-elevated-session-helper <controller_pid> <pipe_name> <session_id>`) communicating over a local Windows Named Pipe with:
     - Restrictive SDDL DACL (`SYSTEM`, `Builtin Administrators`, and current user SID)
     - `FILE_FLAG_FIRST_PIPE_INSTANCE` and `PIPE_REJECT_REMOTE_CLIENTS`
     - Mutual peer PID verification (`GetNamedPipeClientProcessId` and `GetNamedPipeServerProcessId`) and exact executable image path verification (`QueryFullProcessImageNameW`)
     - Parent-process `SYNCHRONIZE` liveness watchdog (`WaitForSingleObject` on controller process handle) that immediately shuts down the helper if the controller crashes or exits, plus a 4-hour maximum session bound
     - Versioned, length-bounded (`<= 2048` bytes) JSON protocol (`#[serde(deny_unknown_fields)]`) accepting only fixed operations: `Ping`, `PurgeStandbyList`, and `Shutdown`.
   - Adds an independent monotonic scheduler thread (`memflux-cleaner`) driven by `std::time::Instant` and `Condvar`, decoupled from dashboard telemetry refresh intervals.
   - Adds a durable quota ledger (`cleaner-quota.json`) combined with in-session monotonic `Instant` tracking so manual and automatic purges count toward minimum interval, cooldown, and rolling-hour limits across restarts, clock adjustments, and action-history churn.
   - Exposes truthful user-facing cleaner states (`disabled`, `configured_unauthorized`, `thresholds_not_met`, `action_pending`, `cleaning_in_progress`, `cooldown_active`, `rate_limited`, `excluded_application`, `paused`, `unsupported`, `error`).

2. **Structured Windows Compatibility Matrix (`src-tauri/src/action.rs`, `docs/WINDOWS_API_COMPATIBILITY.md`):**
   - Queries OS build (`RtlGetVersion`), edition (`ProductName`), and revision (`UBR`) from the Windows registry.
   - Exposes a structured compatibility matrix recording build `26340` (`confirmed_supported`) alongside representative stable targets (`26100` Windows 11 24H2, `22631` Windows 11 23H2, `19045` Windows 10 22H2) explicitly marked `unverified` and gated off until runtime evidence on those hosts is recorded.

3. **Live Performance HUD (`src-tauri/src/performance.rs`, `src/components/PerformanceHud.tsx`):**
   - **CPU %:** `GetSystemTimes` delta with idle-in-kernel accounting, warm-up, discontinuity rejection, and single-processor-group validation.
   - **GPU %:** DXGI adapter enumeration (`CreateDXGIFactory1` -> `EnumAdapters1` -> `GetDesc1`) paired with WDDM PDH `\GPU Engine(*)\Utilization Percentage` (`PdhGetFormattedCounterArrayW`). Sums per-PID utilization on the same physical engine and reports the **Busiest GPU engine** (`3D`, `Compute`, `Copy`, `VideoDecode`, `VideoEncode`) without summing independent engines.
   - **RAM %:** Physical memory load from `GlobalMemoryStatusEx`.
   - **FPS & Frame Time:** Non-injecting Intel PresentMon ETW controller supporting manual/detected game selection, explicit start/stop, mean FPS + P99 frame time calculation, staleness detection, and explicit `Unavailable` reporting (`—`) when PresentMon is not installed or no capture is active.
   - **HUD Customization:** Schema-v4 settings persist HUD visibility, per-metric toggles, GPU adapter selection, summary/detailed attribution, compact mode, refresh cadence, and FPS capture permission while preserving 250 / 218 / 82 px sidebar widths.

4. **Optimization Rewind (`src-tauri/src/rewind.rs`):**
   - Versioned, atomic local journal (`rewind-journal.json`) recording every system mutation before execution and confirming observed state after execution.
   - **Compare-before-restore:** Queries live Windows state before restoring; if the user externally changed the power scheme (`live != applied`), preserves the external selection and marks the entry `conflict_skipped`.
   - **Startup crash recovery:** Automatically inspects and restores leftover `Active` entries on startup.

5. **PowerShift (`src-tauri/src/powershift.rs`):**
   - Uses documented Win32 PowrProf APIs (`PowerEnumerate`, `PowerReadFriendlyName`, `PowerGetActiveScheme`, `PowerSetActiveScheme`, `GetSystemPowerStatus`).
   - Supports `CurrentSystemMode`, `Balanced`, `Performance`, and `Custom` modes using only real schemes present on the host, with AC/battery detection, conservative battery blocking, Rewind journaling, and post-apply `PowerGetActiveScheme` verification.

6. **GameFocus & Central Optimization Coordinator (`src-tauri/src/gamefocus.rs`, `src-tauri/src/coordinator.rs`):**
   - Conservative foreground + process-metadata game detector with a strict non-game blocklist (`memflux.exe`, browsers, OBS, Discord, launchers, system processes).
   - User-managed game profiles (`Balanced Gaming`, `Performance Gaming`, `Gaming + Streaming`, `Custom`).
   - Debounced session lifecycle (`Idle`, `CandidateDetected`, `ConfirmedGame`, `ProfilePending`, `Active`, `TemporarilyBackgrounded`, `Restoring`, `Completed`, `Degraded`) that survives Alt+Tab without thrashing power schemes and automatically restores via Optimization Rewind when the game process exits.
   - Streaming workload protection (`obs64.exe`, `discord.exe`, `audiodg.exe`, `voicemeeter.exe`) and read-only background process contention intelligence (CPU %, working set bytes, and WDDM per-process GPU %).

7. **CI & Windows Installer Packaging (`.github/workflows/ci.yml`, `src-tauri/tauri.conf.json`):**
   - GitHub Actions workflow verifying version consistency, `npm ci`, `npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `npm run tauri build`, and SHA-256 artifact checksums.
   - Produced installable Windows packages:
     - `src-tauri/target/release/bundle/nsis/MemFlux_0.2.0_x64-setup.exe` (`1,675,621` bytes, SHA-256 `CEF471D6F21515274C236C58CD6AD484D2466EEBDFE6B46533745B90EB2FF4CA`)
     - `src-tauri/target/release/bundle/msi/MemFlux_0.2.0_x64_en-US.msi` (`2,248,704` bytes, SHA-256 `4D78B9FDFB3D87DEE61BDA1C1E25FDCB52864B33B19535C02B01646F65544357`)
     - `src-tauri/target/release/memflux.exe` (`4,063,744` bytes, SHA-256 `CBC1FE5EAC1B072F4E134253E41F29F8B1EE422E7126811F649BA0BC405EA6EC`)

---

## 2. Verification Results

| Check | Command / Method | Result |
|---|---|---|
| Frontend unit tests | `npm test` | **Passed** (5/5 tests across `format.test.ts` and `PerformanceHud.test.tsx`) |
| Frontend production build | `npm run build` (`tsc -b && vite build`) | **Passed** (strict TypeScript + Vite bundle: 292.42 kB JS, 29.60 kB CSS) |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | **Passed** |
| Rust lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | **Passed** (0 warnings, 0 errors) |
| Rust unit & native tests | `cargo test --manifest-path src-tauri/Cargo.toml` | **Passed** (33/33 tests, including live Named Pipe session helper IPC, live DXGI + WDDM GPU Engine collection, live PowrProf PowerShift switch + Rewind restore, GameFocus Alt+Tab lifecycle, and Rewind crash recovery) |
| Windows installer build | `npm run tauri build` | **Passed** (NSIS `1.60 MiB` per-user setup + MSI `2.14 MiB` installer) |
| Native helper privilege boundary | `memflux.exe --memflux-elevated-purge-standby` (standard user) | **Passed** (returned `0xC0000061` `STATUS_PRIVILEGE_NOT_HELD`) |
| Resource overhead profile | 15s release run on build 26340 (24 logical CPUs) | **Measured**: Native `memflux.exe` = `0.0271%` CPU, `11.36 MiB` private, `33.82 MiB` working set, 443 handles, 39 threads; WebView2 children (6) = `253.04 MiB` private, `378.47 MiB` working set |

---

## 3. Beta Gate Evaluation

| Gate | Status | Evidence & Remaining Limitations |
|---|---|---|
| **Cleaner** | **Pass (on Build 26340)** | Manual and session-authorized automatic purging implemented with independent monotonic scheduler and durable quota ledger. |
| **Privileges** | **Pass** | Named Pipe session helper uses restrictive SDDL DACL, `PIPE_REJECT_REMOTE_CLIENTS`, peer PID + executable path verification, fixed operation enum, and parent `SYNCHRONIZE` watchdog. |
| **Compatibility** | **Blocked on External VMs** | Build 26340 verified; structured matrix implemented; stable Windows 10 (`19045`) and Windows 11 (`22631`, `26100`) VMs are not attached to this machine and remain gated as `unverified`. |
| **HUD** | **Pass** | Real CPU (`GetSystemTimes`), attributed GPU (DXGI + WDDM `GPU Engine` busiest engine), RAM, and PresentMon adapter with truthful `—` when capture is absent. |
| **GameFocus** | **Pass** | Conservative detection, presets, Alt+Tab debounce, streaming protection, and background contention analyzer implemented and tested. |
| **PowerShift** | **Pass** | Documented Win32 PowrProf enumeration, active detection, battery awareness, and verified scheme switching tested live on host. |
| **Rewind** | **Pass** | Atomic journal, compare-before-restore external-change preservation, idempotency, and startup crash recovery verified. |
| **Resource Budget** | **Partial** | Native process is lightweight (`0.0271%` CPU, `11.36 MiB` private); visible WebView2 child tree uses `253.04 MiB` private. Multi-hour soak pending. |
| **UI** | **Pass (Automated) / Interactive QA Pending** | Midnight CosmoTech™ UI updated with truthful states and onboarding; interactive desktop window/DPI/UAC click-through requires human desktop session. |
| **CI** | **Pass** | `.github/workflows/ci.yml` configured with least-privilege permissions, version check, lint/test gates, bundler, and SHA-256 checksums. |
| **Packaging** | **Pass (Unsigned Beta Artifact)** | Both NSIS (`MemFlux_0.2.0_x64-setup.exe`) and MSI (`MemFlux_0.2.0_x64_en-US.msi`) installers built and hashed. Code signing requires external certificate credentials. |
| **Documentation** | **Pass** | `README.md`, `CHANGELOG.md`, `docs/*`, and implementation plans synchronized with V4.1 reality. |

### Overall Release Classification: **Beta Candidate**

MemFlux V4.1 has completed all core functional engineering slices (Cleaner + Session Helper IPC, WDDM GPU HUD, GameFocus, PowerShift, Optimization Rewind, Central Coordinator, and NSIS/MSI Packaging). It is classified as a **Beta Candidate** rather than Public Beta / Release Candidate because:
1. Hands-on standby-purge qualification on stable Windows 10 (`19045`) and Windows 11 (`22631`, `26100`) VMs has not yet been executed outside this build `26340` host.
2. Interactive UAC prompt click-through, PresentMon binary capture against a live 3D game, and multi-hour soak testing require interactive human desktop testing.
3. Installer artifacts are unsigned development/beta packages pending publisher code-signing credentials.

---

## 4. Highest-Value Continuation Step

Run the built `MemFlux_0.2.0_x64-setup.exe` installer on representative stable Windows 10 (`19045`) and Windows 11 (`22631` / `26100`) test machines with Intel PresentMon installed, execute the interactive UAC session-helper and standby-purge qualification checklist in `docs/TESTING.md`, and record the resulting build numbers and pre/post counter evidence into `SUPPORTED_BUILDS` in `src-tauri/src/action.rs`.
