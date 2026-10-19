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
- Settings interval/cleaner constraints, persisted round trip, invalid-file fallback, and preservation of malformed data.
- Exact native constants, fixed helper argument, tested-build capability gating, and a regression proving helper timeout cannot become native success.
- Automatic policy dual-threshold triggering, authorization gating, exclusions, minimum interval, cooldown, and rolling-hour rate limiting.
- Bounded action-history persistence and restart loading.

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
12. Invoke the cleaner, approve UAC, and verify raw status plus before/after standby and free/zero values.
13. Repeat with little standby and verify no fictitious reduction is reported.
14. Reject UAC and verify a cancellation result without crashing or changing the monitoring state.

## Native cleaner evidence — build 26340

- Direct unelevated helper: `STATUS_PRIVILEGE_NOT_HELD (0xC0000061)`.
- Elevated fixed helper: `STATUS_SUCCESS (0x00000000)`.
- Retained sample after 750 ms: standby 233,820,160 → 25,538,560 bytes; Free & Zero 10,254,319,616 → 10,510,528,512 bytes.
- The first successful run caused a PowerShell reporting overflow after the action; it is acknowledged but not used as precise numerical evidence.
- UAC rejection, UI-driven concurrency, background/tray action, and shutdown during action remain unverified.
- Helper timeout and late completion remain deterministic-test-only; no native timeout was intentionally induced.

## Required compatibility work

- Stable Windows 10 and Windows 11, standard user.
- 8 GB, 16 GB, and 32+ GB configurations.
- Missing or disabled performance counters.
- Sleep/resume and clock/sample gaps.
- 100%, 125%, 150%, and 200% display scaling.
- Keyboard-only navigation, screen-reader labels, contrast, and reduced motion.
- 30-minute Phase 1 soak, followed by 8+ hour beta soak with handle/working-set tracking.

Mock fixtures prove policy and UI behavior only. They are never evidence that a Windows action works or improves performance.

## V3 P0 continuation

21 Rust tests and 2 frontend tests pass; strict TypeScript production build, Rust format and all-target Clippy with warnings denied pass. Production result conversion now has a regression test for wait timeout, wait failure, missing exit code, cancellation and native privilege failure. Tests also cover exact exclusion matching, unknown foreground identity, cleaner pause/default disable, and cooldown restored from persisted manual actions.

The current sandbox denies CIM queries; read-only registry inspection confirms build 26340.9750. Successful native purge evidence above belongs to the earlier baseline. This continuation has not repeated elevation or disruptive purge tests. P0 is still partial because session-helper authorization and automatic native execution are incomplete.

