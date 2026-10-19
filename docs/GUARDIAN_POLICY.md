# Adaptive Guardian Policy

## Phase 1 status

Implemented as a deterministic pressure evaluator. It can Observe or Recommend review. Manual standby cleaning is a separate explicit command; Guardian neither blocks it nor silently triggers it.

## Inputs

- Available physical percentage.
- Commit utilization.
- Documented Windows low-memory notification.
- PDH hard page-read activity when available.
- Core telemetry validity.
- Guardian enabled and monitoring paused state.
- Standby share for explanatory messaging only.

## Rules

1. Disabled or paused: reset history and Observe with `guardian_paused`.
2. Missing/invalid core data: reset history, pressure Unknown, and Observe.
3. Critical candidate: available below 5%, commit at least 90%, and either Windows Low state or at least 20 page reads/sec.
4. Critical candidates must occur for three consecutive sampler evaluations before Recommend.
5. Elevated: Windows Low state, available below 10%, or commit at least 85%; Observe.
6. Watch: available below 15% or commit at least 75%; Observe.
7. Otherwise Comfortable and Observe. A large standby share may be explained but cannot create pressure.

These thresholds are conservative starting policy, not universal Windows performance truths. They require field validation before a beta.

## Invariants

- A single sample cannot recommend.
- A healthy sample resets critical hysteresis.
- Pause resets critical hysteresis.
- Missing hard-read data lowers confidence but does not become zero.
- Invalid core telemetry suspends evaluation.
- Every result has a stable reason code, human explanation, confidence label, and evidence list.
- `actionCapability` identifies the separate manual purge boundary. Future automatic cleaner policy must remain independently configurable and opt-in.

## Cleaner policy (V3 P0, partial)

`cleaner.rs` independently evaluates standby >= configured standby threshold AND Free & Zero <= configured free threshold. It requires valid finite counters, enabled/unpaused policy, exclusion clearance, minimum interval, cooldown, rolling-hour quota, and explicit authorization readiness. Accepted manual requests in persistent history count toward cooldown and quota. A missing foreground identity blocks evaluation when exclusions are configured. Filename exclusions match exact path components, not arbitrary suffixes.

The runtime currently provides no unattended authorization, so eligible policy reports `authorization_required` and performs no automatic action. Foreground observation is also pending. Poll evaluation follows monitoring samples, pauses with monitoring, and cannot exceed the native sampling cadence. This is a tested policy foundation; it does not satisfy automatic-cleaning functional parity yet.

