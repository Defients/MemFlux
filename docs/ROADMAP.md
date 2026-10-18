# Roadmap

## Phase 0 — Foundation

Audit, architecture, build chain, API boundary, documentation, and test scaffolding. Current milestone; see the implementation plan for verified gate status.

## Phase 1 — Monitoring MVP

Real Windows metrics, background collector, Overview, sensor provenance, monitoring-only Guardian, settings, tray, logging, and CosmoTech interface. Implemented in the current vertical slice; native runtime and soak verification determine gate completion.

## Phase 2 — Guardian depth and capability gating

- Decision history, configurable safeguards, exclusions, cooldowns, and rate limits.
- Stable compatibility matrix and telemetry recovery behavior.
- Explicit unsupported capability surface for memory actions.
- Only after independent evidence: investigate a narrowly isolated, per-action elevated adapter. Ship without clearing if it cannot be made defensible.

## Phase 3 — Evidence lab

- Bounded historical storage and migrations.
- Performance sessions, annotations, comparable A/B methodology, and export.
- Foreground app and opt-in game profiles.
- PresentMon feasibility/licensing and non-injection integration.
- Modern process-scoped Timer Lab with balanced request/release.

## Phase 4 — Productization

- Compact desktop monitor (not injected game overlay).
- First-run capability scan and onboarding.
- Full accessibility/DPI matrix.
- Installer, signing, update authenticity, recovery, and release channels.
- Stable Windows 10/11 matrix and long-duration resource validation.

No roadmap item implies that memory clearing improves gaming or that the mechanism will be shipped.

