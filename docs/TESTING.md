# Testing

## Automated commands

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

## Implemented automated & native coverage (33 Rust tests + 5 Frontend tests)

- **Core Memory & Guardian (`telemetry`, `guardian`):**
  - Live Windows `GlobalMemoryStatusEx`, `GetPerformanceInfo`, memory resource notifications, and PDH counters.
  - Guardian healthy state, 3-sample critical hysteresis, reset on recovery/pause, and missing-core suspension.
- **Cleaner & Session Helper (`action`, `cleaner`, `app`):**
  - Exact helper arguments (`--memflux-elevated-purge-standby` and `--memflux-elevated-session-helper`), parameter validation, and tested-build allowlist gating.
  - Regression test proving `WAIT_TIMEOUT (0x00000102)` can never become native success.
  - Live Windows Named Pipe session helper test verifying restrictive SDDL security descriptor, `GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId`, executable path verification, `Ping`, `PurgeStandbyList`, and `Shutdown`.
  - Dual-threshold evaluation (`standby >= threshold && free_zero <= threshold`), foreground exclusion matching, minimum interval, cooldown, rolling-hour limit, persisted manual purge accounting, and monotonic `Instant` protection against clock shifts.
- **CPU & WDDM GPU Telemetry (`performance`):**
  - `GetSystemTimes` idle-inclusive delta arithmetic, discontinuity rejection, and live Windows warm-up + bounded delta read.
  - WDDM `GPU Engine` instance parser and aggregator (summing per-PID utilization within the same physical engine while taking the maximum across independent engines).
  - Live Windows DXGI adapter enumeration and WDDM PDH `\GPU Engine(*)\Utilization Percentage` two-sample read.
- **Optimization Rewind, PowerShift, GameFocus & Coordinator (`rewind`, `powershift`, `gamefocus`, `coordinator`):**
  - Atomic `RewindJournal` normal restoration, idempotency, compare-before-restore protection when a user externally changes state, and startup crash recovery.
  - Live Windows PowrProf scheme enumeration (`PowerEnumerate`, `PowerReadFriendlyName`), active scheme detection (`PowerGetActiveScheme`), verified scheme switch (`PowerSetActiveScheme`), and automatic Rewind restoration back to the original baseline scheme.
  - GameFocus non-game blocklist (`memflux.exe`, browsers, OBS, Discord, launchers, system processes), streaming workload protection, Alt+Tab focus transition debouncing (`TemporarilyBackgrounded`), and automatic restoration on game process exit.
- **Frontend (`format.test.ts`, `PerformanceHud.test.tsx`):**
  - Byte/percentage/rate formatting without fabricating unavailable values.
  - Live HUD rendering of CPU, GPU, RAM, absent FPS/frame time (`—`), paused/stale detection, and per-metric HUD customization toggles.

## Atlas regression and qualification protocol

Read-only/nonmutating suite used for Atlas:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -- --skip windows_power_schemes_enumerate_and_round_trip_restore_with_rewind --nocapture
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm test
npm run build
cargo build --manifest-path src-tauri/Cargo.toml --release
```

The existing PowerShift round-trip test changes the actual system scheme and is deliberately excluded from this Atlas run. Running the repository's unfiltered suite still includes it. No new elevated purge was exercised; the helper round-trip uses a mock fixed-operation callback returning zero, which is transport evidence only, not native NTSTATUS evidence.

Atlas replay covers joint sustained pressure, healthy cache/high CPU/GPU without a bottleneck claim, stale/missing/nonfinite/future samples, clock reset/interruption, invalid physical headroom, recovery contradictions, event-vs-arrival context, PID creation/session identity, unavailable event mapping, bounded retention and payload size. Native tests cover cancellation completion on pending accept/read/write, shutdown flag and deadline, event/buffer reuse and a 75ms delayed reader. Read-only live integration passes the existing Windows CPU/WDDM/memory sources through the actual Atlas engine without starting capture. Frontend tests cover evidence/incident/Proof linkage, stale/paused surfaces, per-source frame freshness, new IPC frames between clock ticks and field-specific HUD close.

Synthetic browser layout QA is separate from native WebView2 integration and gameplay. Full Tauri window/DPI/keyboard/tray/Mini HUD interaction, real PresentMon QPC capture, OBS/thermal/storage/VRAM providers and seven-mode profiling remain qualification work.

Current Atlas results: **68 Rust tests passed** (one existing power-mutating test excluded), **11 frontend tests passed**, strict Clippy and format passed, frontend production build passed. Optimized filtered replay passed. Edge layout fixture checked at the default and 900px viewports, simple/advanced disclosure verified; temporary fixture removed. Full Windows native UI integration and provider gameplay timing remain unverified.

Final optimized executable build passed after the last source/frontend changes: `src-tauri/target/release/memflux.exe`, 3,860,992 bytes; SHA-256 `4E3A4C5DB55CB71CFFA734A14B3C24767ED828FAD870D3B5E156CB1952AC527E`. Command: `cargo build --manifest-path src-tauri/Cargo.toml --release` (final compilation 1m 56s). The executable was compiled, not launched for a new native purge or power mutation. Installer bundles/signing and remote CI were not rerun.
