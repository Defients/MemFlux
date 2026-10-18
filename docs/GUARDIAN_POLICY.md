# Adaptive Guardian Policy

## Phase 1 status

Implemented as a deterministic, monitoring-only evaluator. It can Observe or Recommend review. It has no Act outcome or native memory-action command.

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
- `actionCapability` remains `unsupported_monitoring_only`.

