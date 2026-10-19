# MemFlux

**Standby Memory Control · GameFocus · Reversible PowerShift · Powered by CosmoTech™**

MemFlux is a local-first Windows standby-memory cleaning and gaming session control utility. It performs a real, privilege-gated Windows standby-list purge, measures the system before and after, provides a live CPU/GPU/RAM/FPS sidebar HUD, detects gaming sessions with GameFocus, and applies verified, reversible Windows power schemes through PowerShift and Optimization Rewind.

## Core V4.1 capabilities

- **Manual & Authorized Automatic Standby Cleaner:**
  - Prominent **PURGE STANDBY MEMORY** control calling `NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)` after enabling `SeProfileSingleProcessPrivilege`.
  - Narrow one-shot UAC helper (`--memflux-elevated-purge-standby`) and explicitly authorized session helper (`--memflux-elevated-session-helper`) over an authenticated Windows Named Pipe (`PIPE_REJECT_REMOTE_CLIENTS`, restrictive SDDL DACL, `GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId` peer verification, executable image path verification, and parent-process liveness watchdog).
  - Independent monotonic scheduler (`memflux-cleaner`) evaluating `standby_bytes >= standby_threshold` AND `free_zero_bytes <= free_zero_threshold` without coupling to dashboard refresh cadence.
  - Durable quota ledger (`cleaner-quota.json`) and monotonic elapsed tracking enforcing minimum interval, cooldown, and rolling-hour limits across restarts and clock adjustments.
- **Live Performance HUD:**
  - Compact sidebar HUD (preserving 250 / 218 / 82 px responsive widths) displaying real `GetSystemTimes` CPU %, DXGI + WDDM `\GPU Engine(*)\Utilization Percentage` GPU % (attributed to the busiest engine without summing independent engines), physical RAM %, optional non-injecting Intel PresentMon FPS & frame-time ms, and truthful cleaner state.
  - Full HUD customization persisted in schema-v4 settings (per-metric toggles, GPU adapter selection, summary/detailed attribution, compact mode, and FPS capture permission).
- **GameFocus & Workload Intelligence:**
  - Conservative Windows foreground + process-metadata game session detector with a strict non-game blocklist (never misclassifying MemFlux, browsers, OBS, Discord, or launchers as games).
  - User-managed game profiles (`Balanced Gaming`, `Performance Gaming`, `Gaming + Streaming`, `Custom`) with debounced focus transitions (`TemporarilyBackgrounded` survives Alt+Tab without thrashing power schemes) and automatic restoration on game exit.
  - Streaming workload protection (`obs64.exe`, `discord.exe`, `audiodg.exe`, `voicemeeter.exe`) and read-only background workload contention reporting.
- **PowerShift & Optimization Rewind:**
  - Documented Win32 PowrProf scheme enumeration (`PowerEnumerate`, `PowerReadFriendlyName`), active detection (`PowerGetActiveScheme`), AC/battery detection (`GetSystemPowerStatus`), and verified scheme activation (`PowerSetActiveScheme` + post-apply verification).
  - Durable atomic `rewind-journal.json` with **compare-before-restore** (never overwriting external user power-scheme changes) and automatic startup crash recovery.

## Development & Verification

Requirements: Windows 10/11 x64, WebView2, Node.js, Rust MSVC, Visual Studio C++ Build Tools, and a Windows SDK.

```powershell
npm ci
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

See [MEMFLUX_V4_1_BETA_EXECUTION_PLAN.md](MEMFLUX_V4_1_BETA_EXECUTION_PLAN.md) for the complete V4.1 implementation ledger, verification evidence, resource measurements, and beta gate evaluation.

## Safety and privacy

Monitoring runs as a standard user. Manual cleaning requests UAC only for a fixed helper in the same executable, and automatic cleaning requires an explicitly authorized session helper accepting only fixed operations (`Ping`, `PurgeStandbyList`, `Shutdown`). MemFlux installs no kernel driver, performs no game-process injection, and uploads no analytics or telemetry. See [docs/SECURITY.md](docs/SECURITY.md).

## Bottleneck Atlas diagnostic alpha (2026-10-19)

The Bottleneck Atlas view now links the shared native sampler to a conservative memory/paging investigation, timestamped Stutter context, evidence inspection and a linked Performance Proof baseline. High CPU/GPU usage or a large standby cache alone never establishes a bottleneck. Storage latency, attributed game VRAM, thermal/throttle and OBS statistics remain unavailable.

Integrity repairs cover IPC cancellation lifetime and reply-disconnect ordering, QPC event/arrival separation, neutral Proof resource deltas, HUD freshness and stale-settings conflicts. Live PresentMon gameplay and full application overhead qualification are pending. See [the Atlas ledger](MEMFLUX_BOTTLENECK_ATLAS_V1_IMPLEMENTATION_PLAN.md) for implemented, tested and deferred scope.
