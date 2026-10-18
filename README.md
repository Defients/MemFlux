# MemFlux

**Adaptive Memory Intelligence · Powered by CosmoTech™**

MemFlux is a local-first Windows memory monitor built around one rule: measure first, understand second, intervene only when justified, and verify afterward.

The current Phase 1 alpha collects real system memory data from documented Windows APIs, explains pressure through a deterministic monitoring-only Guardian, persists local settings, and continues efficient sampling from the system tray. It does **not** clear standby memory or claim to optimize performance.

## Implemented now

- Native `GlobalMemoryStatusEx` and `GetPerformanceInfo` collection.
- Documented Windows low/high memory resource notifications.
- PDH English counters for standby classes, free/zero pages, modified pages, and hard page reads.
- Per-metric source, timestamp, interval, status, and failure detail.
- Explainable Guardian with three-sample hysteresis and no action path.
- Adaptive foreground/background sampling owned by Rust.
- Versioned, validated, atomically replaced local settings.
- System tray open, pause/resume, close-to-tray, explicit exit, and single instance.
- Responsive Midnight CosmoTech™ Overview, Memory Explorer, Guardian, and Settings views.

## Explicitly unsupported

- Standby-list clearing and automatic memory intervention.
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

Monitoring runs as a standard user. MemFlux installs no service or driver, does not ask for elevation, opens no arbitrary shell path, and uploads no analytics or system telemetry. See [docs/SECURITY.md](docs/SECURITY.md).

## Project maturity

Phase 1 alpha. The implementation is a real native vertical slice, not a performance product claim. Stable Windows 10/11 compatibility, long-duration performance budgets, packaging, signing, and accessibility/DPI matrices remain release work.

