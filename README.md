# MemFlux

**Standby Memory Control + Adaptive Intelligence · Powered by CosmoTech™**

MemFlux is a local-first Windows standby-memory cleaning utility. It performs a real, privilege-gated Windows standby-list purge, measures the system before and after, and keeps monitoring and Guardian diagnostics around that core action.

The current cleaner milestone is enabled only on the explicitly validated Windows build. Monitoring remains available without elevation; a manual purge launches the same binary in a narrow one-shot helper mode through Windows UAC.

## Working cleaner milestone

- Prominent **PURGE STANDBY MEMORY** control.
- Native `NtSetSystemInformation` call using information class 80 and `MemoryPurgeStandbyList` command 4.
- One-shot UAC helper that accepts only the fixed standby-purge operation and enables only `SeProfileSingleProcessPrivilege`.
- Explicit capability detection and a tested-build allowlist; unsupported builds keep monitoring but disable the action.
- Serialized actions with real standby, free/zero, and available-memory snapshots before and after a 750 ms settling interval.
- Raw `NTSTATUS`, request success, measured reduction, duration, elevation path, and bounded in-session history.
- Honest success-with-no-measurable-reduction reporting.

## Monitoring foundation

- Native `GlobalMemoryStatusEx` and `GetPerformanceInfo` collection.
- Documented Windows low/high memory resource notifications.
- PDH English counters for standby classes, free/zero pages, modified pages, and hard page reads.
- Per-metric source, timestamp, interval, status, and failure detail.
- Explainable Guardian pressure assessment kept separate from cleaner control.
- Adaptive foreground/background sampling owned by Rust.
- Versioned, validated, atomically replaced local settings.
- System tray open, pause/resume, close-to-tray, explicit exit, and single instance.
- Responsive Midnight CosmoTech™ Overview, Memory Explorer, Guardian, and Settings views.

## Not implemented yet

- Opt-in automatic threshold cleaning, cooldown/frequency limits, and exclusions (Milestone 2).
- Low-priority-only standby purge, pending independent native validation.
- Timer-resolution changes.
- Game overlays, PresentMon, performance sessions, and performance-improvement claims.
- Windows versions other than those actually tested and recorded in the compatibility document.

## Development

Requirements: Windows 10/11 x64, WebView2, Node.js, Rust MSVC, Visual Studio C++ Build Tools, and a Windows SDK.

```powershell
npm install
npm run build
npm test
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri dev
```

Create a native debug executable without an installer:

```powershell
npm run tauri build -- --debug --no-bundle
```

See [MEMFLUX_IMPLEMENTATION_PLAN.md](MEMFLUX_IMPLEMENTATION_PLAN.md) for live milestone status, measurable gates, current risks, and actual validation results.

## Safety and privacy

Monitoring runs as a standard user. Manual cleaning requests UAC only for a one-shot helper in the same executable. The helper accepts no arbitrary shell command or path, installs no service or driver, and returns only its native status through the process exit code. MemFlux uploads no analytics or system telemetry. See [docs/SECURITY.md](docs/SECURITY.md).

## Project maturity

Milestone 1 cleaner alpha. Native purging is runtime-verified only on Windows 11 Home Insider Preview build 26340; other builds remain action-disabled until tested. Clearing cache is not a memory-leak fix and is not evidence of higher FPS or lower latency.

