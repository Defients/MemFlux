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
