# Testing

## Automated commands

```powershell
npm run build
npm test
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build -- --debug --no-bundle
```

## Implemented automated coverage

- Byte, percentage, and rate formatting preserves unavailable states.
- Percentage arithmetic bounds results and rejects a zero denominator.
- Guardian healthy outcome, three-sample critical hysteresis, hysteresis reset, and missing-core suspension.
- Settings interval constraints, persisted round trip, invalid-file fallback, and preservation of malformed data.

## Native smoke checklist

1. Launch without an administrator prompt.
2. Confirm physical total/available and commit values update from native sources.
3. Compare values to PowerShell/CIM or Performance Monitor within one sampling interval.
4. Confirm Page Reads/sec warms up rather than showing an invented zero.
5. Open Memory Explorer and confirm every sensor has source and availability.
6. Pause, wait longer than foreground cadence, and verify the sample sequence does not advance.
7. Resume and verify the sequence advances.
8. Change settings, restart, and verify persistence.
9. Close to tray, reopen from tray, and verify one continuing sequence.
10. Start a second instance and verify the original window focuses.
11. Choose tray Exit and verify the process terminates.

## Required compatibility work

- Stable Windows 10 and Windows 11, standard user.
- 8 GB, 16 GB, and 32+ GB configurations.
- Missing or disabled performance counters.
- Sleep/resume and clock/sample gaps.
- 100%, 125%, 150%, and 200% display scaling.
- Keyboard-only navigation, screen-reader labels, contrast, and reduced motion.
- 30-minute Phase 1 soak, followed by 8+ hour beta soak with handle/working-set tracking.

Mock fixtures prove policy and UI behavior only. They are never evidence that a Windows action works or improves performance.

