# ChatGPT project context

This directory is a local mirror of the ChatGPT project “MemFlux”.

- Treat every file under `sources/` as read-only reference material.
- Do not edit, rename, move, or delete synced project files.
- These files may be replaced the next time a task is created from this ChatGPT project.


## Project instructions

### Product direction

- MemFlux is a functional Windows standby-memory cleaner with monitoring and Guardian intelligence around the action. A monitoring-only build is not a completed product.
- Preserve the working Tauri 2, React, Rust telemetry, settings, tray, and Midnight CosmoTech™ interface. Extend cohesive vertical slices; do not rebuild from scratch.
- Full standby-list purging is the primary native operation. Never substitute `EmptyWorkingSet`, process trimming, or a fabricated counter change.
- Report native request acceptance separately from measured before/after reduction. Never invent freed bytes or performance improvement.

### Privilege and compatibility boundary

- Ordinary telemetry and the main UI stay unelevated. Privileged code must accept only explicit memory-list operations; no arbitrary commands, paths, driver, persistent unrestricted elevation, or security bypass.
- The current operation is `NtSetSystemInformation(SystemMemoryListInformation=80, MemoryPurgeStandbyList=4)` after enabling `SeProfileSingleProcessPrivilege`.
- Treat the memory-list information class and command as compatibility-sensitive native interfaces. Keep them isolated, return raw `NTSTATUS`, maintain an explicit tested-build allowlist, and disable actions outside it until validated.
- Preserve direct user control. Automatic cleaning must remain opt-in and must use standby plus Free & Zero Page List thresholds—not available physical memory as a substitute.

### Engineering discipline

- Serialize native actions. Capture pre/post telemetry, use a settling interval, retain bounded action history, and distinguish success, no observed reduction, skip, denial, unsupported, and failure states.
- Keep Guardian pressure assessment separate from cleaner policy. Guardian may explain or recommend but must not silently override configured thresholds.
- Add deterministic tests for action classification, settings, cooldown/frequency policy, exclusions, serialization, native status propagation, and unsupported capability gating.
- Before claiming Windows functionality verified, record edition/build, elevation state, exact mechanism, raw native result, counter evidence, and method. Do not generalize one Insider-build result to Windows 10/11 broadly.
- Update `MEMFLUX_IMPLEMENTATION_PLAN.md`, relevant docs, and `CHANGELOG.md` as milestones change. Mark implemented, tested, partial, untested, and blocked status explicitly.
- Preserve user changes and the read-only `sources/` boundary. Avoid destructive Git and filesystem operations.

### Performance Evolution V3 continuity

- Treat helper launch, UAC cancellation, Win32 wait outcome, timeout, exit-code retrieval, native `NTSTATUS`, request acceptance, and measured effect as separate typed facts. `WAIT_TIMEOUT` (`0x102`) is never a native success and does not mean the helper was terminated.
- Keep automatic cleaner evaluation off by default and independent from Guardian. Never initiate UAC from a background threshold trigger; unattended execution requires an explicitly authorized, fixed-operation session helper with authenticated local IPC.
- Do not begin mutable GameFocus or PowerShift controls until the P0 cleaner acceptance gate in `MEMFLUX_PERFORMANCE_EVOLUTION_V3_PLAN.md` passes.
- New performance features must distinguish action execution, observed state change, workload performance change, and confidence. Preserve exact restoration state for every reversible change.

### OmniFlux V4 continuity

- Follow the phase ledger in `MEMFLUX_OMNIFLUX_V4_IMPLEMENTATION_PLAN.md`. Read-only sensors may progress alongside cleaner completion; privileged automatic execution still requires authenticated session-helper IPC.
- CPU utilization is a delta of GetSystemTimes counters; kernel includes idle. Preserve warm-up/reset/zero-delta handling and the multi-processor-group unavailable state until group-aware implementation exists.
- Foreground executable inspection grants no process-modification authority. Access failure or a focus transition during lookup must fail closed for exclusions.
- GPU utilization, FPS and frame time remain unavailable until real attributed sources exist. Never use refresh rate as FPS, reciprocal average FPS as frame-time distribution, or MemFlux-local DXGI budgets as game VRAM usage.
- Preserve 250/218/82 px sidebar widths. Sensor collection shares the native sampler; hidden/disabled HUD must not cause a dedicated capture pipeline.
