# ChatGPT project context

This directory is a local mirror of the ChatGPT project “MemFlux”.

- Treat every file under `sources/` as read-only reference material.
- Do not edit, rename, move, or delete synced project files.
- These files may be replaced the next time a task is created from this ChatGPT project.


## Project instructions

### Product identity and safety

- Preserve the MemFlux principle: **measure first, understand second, intervene only when justified, verify afterward**.
- Build a native-feeling Windows 10/11 x64 utility with the Midnight CosmoTech™ identity. Prefer cohesive working vertical slices over broad placeholder scaffolding.
- Never fabricate telemetry, benchmarks, action success, or performance improvements. Represent missing measurements as unavailable or degraded.
- Do not expose standby-list clearing, timer manipulation, process trimming, or other system interventions as functional until the exact mechanism, compatibility, privileges, cleanup, and observed result have been verified.
- Ordinary monitoring must run as a standard user. Do not run the main UI elevated, add a kernel driver, install a permanent privileged service, weaken Windows security, or add unrestricted shell execution.

### Architecture

- Use Tauri 2, React, strict TypeScript, and Rust unless repository evidence justifies a documented exception.
- Keep Windows collection, metric derivation, Guardian decisions, persistence, IPC, and presentation as explicit boundaries. The frontend is never the authority for unrestricted native actions.
- Prefer documented Win32 APIs. Isolate any future unstable or undocumented API behind capability detection and an explicit unsupported state.
- Keep collection independent of rendering. Use timed or event-driven sampling; never busy-wait.
- Treat `MEMORYSTATUSEX.ullAvailPhys` as already including reclaimable standby, free, and zero pages. Never add standby to it.
- Do not label page faults as hard faults. Use a documented hard-page-read signal such as `Memory\\Page Reads/sec` and keep source/availability metadata.
- Persist small settings in a versioned, validated local format. Use atomic replacement and safe fallback to defaults on corruption.

### Engineering discipline

- Keep Rust and TypeScript contracts synchronized and typed.
- Add deterministic tests for calculations, Guardian reason codes and thresholds, settings migration/validation, capability gating, and native failure propagation.
- Keep Tauri capabilities least-privilege. Do not add shell, network, filesystem, or process permissions without a concrete reviewed need.
- Use semantic design tokens, accessible keyboard controls, clear loading/degraded states, reduced-motion support, and lightweight charts.
- Update `MEMFLUX_IMPLEMENTATION_PLAN.md`, relevant docs, and `CHANGELOG.md` when implementation status changes. Distinguish implemented, tested, partial, planned, experimental, and unsupported work.
- Before claiming a Windows feature verified, record the Windows edition/build, privilege level, observed result, and test method. A compile-only result is not runtime validation.
- Preserve user changes and avoid destructive Git or filesystem operations. If Git metadata is absent, report that limitation rather than inventing history.
