# Bottleneck Atlas V1 implementation ledger

Audit date: 2026-10-19. Baseline `de3d9240d9cf74abb398caa35efd1d6877185b7a`, branch `main`, initially clean. `sources/` is read-only and excluded from edits.

## Audit and current capabilities

Reviewed repository instructions, README/changelog, V3/V4/V4.1/V4.2 ledgers, architecture/security/testing/performance/compatibility docs, native sampler/action/settings/Guardian/coordinator/GameFocus/Rewind/Proof/Stutter/PresentMon code, React surfaces/hooks/types/styles, dependencies and Windows CI. Existing Tauri/React architecture is retained. No dependencies are added for the first slice.

Confirmed findings at baseline:

- `recent_frames` is test-only but unused, so all-target Clippy with denied warnings fails.
- Pipe read/write and accept cancellation release stack OVERLAPPED/buffers before completion. CancelIoEx requests cancellation; it does not complete it. Event reuse also requires reset.
- Stutter time is CSV receipt time, further advanced by `max(now)`. Provider timestamps are discarded. Different swapchains can contaminate one frame baseline.
- Proof labels lower CPU/GPU/RAM usage improved and calls one-standard-deviation separation "beyond sampling noise" without a justified inference.
- Mini HUD retains values indefinitely during pause/interruption, hydrates settings once, and closes by writing an old whole-settings snapshot.
- Shared native sampler already collects memory/CPU/WDDM GPU and optional PresentMon. No disk-latency, attributed VRAM, thermal or OBS collector exists. GameFocus process inventory does not establish per-thread contention or storage causality.
- Earlier ledgers contain historical completion claims; their recorded native evidence is not new Atlas validation.

## Architecture and first vertical slice

1. Integrity recovery: drain canceled IPC, regression-test native cancellation; preserve raw QPC event time with separately recorded arrival time, suppress correlation when mapping is unavailable; neutral Proof resource deltas; honest HUD freshness and concurrency-safe settings updates.
2. `atlas.rs`: normalized sensor provenance, nullable values, explicit status/target/device/time/window/limitations; bounded observations; deterministic conservative memory/paging evidence rules; bounded incident investigations from Stutter with confidence and missing/contradictory evidence. No mutation authority.
3. Shared telemetry tick feeds Atlas, cached/emitted in the existing frame. No new collector thread, automatic tracing, UAC, or privileged action. Native state survives dashboard destruction.
4. Dedicated Atlas view: findings, six-domain coverage, evidence inspector, incident selection, focused next experiment linking to existing Proof. CPU/GPU are context only until corroborating sources exist. Existing controls and sidebar widths are preserved.

## Sensor matrix

| Domain | Source / attribution | First-slice status / limitation |
|---|---|---|
| RAM/commit | GlobalMemoryStatusEx / GetPerformanceInfo, system | Integrated + read-only native smoke tested; hard paging remains system-wide |
| Standby/free/modified/page reads | PDH Memory counters, system | Existing real collector; independent missing/warm-up states |
| CPU | GetSystemTimes delta, system | Existing; aggregate cannot identify a saturated game thread; multi-group unavailable |
| GPU | WDDM GPU Engine + DXGI LUID | Existing selected-adapter busiest engine; no utilization-only bottleneck conclusion |
| Frames | Explicit PresentMon CSV capture, PID + creation stamp | QPC/event/arrival recovery implemented + fixture tested; live provider qualification pending |
| Disk | No latency/device source | Unavailable; page reads do not establish physical-device contention |
| VRAM | Capacity from DXGI only | Game usage/budget/residency unavailable; no MemFlux-local budget substitution |
| Thermal | No temperature/throttle provider | Unavailable; power scheme is context only |
| Streaming | Protected process inventory only | OBS statistics unavailable; no stream-health diagnosis |

## Milestones and acceptance gates

| Milestone | Current status | Acceptance |
|---|---|---|
| ATLAS-0 integrity | Implemented + regression tested; live frame gate partial | Clippy/build/tests pass; IPC cancellation completion covered; timestamp/freshness/Proof defects corrected |
| ATLAS-1 engine | First rule set + replay implemented/tested; broader ranking partial | Deterministic replay covers gaps, invalid/stale data, pressure, healthy cache, interruptions and no reliable cause |
| ATLAS-2 first memory slice | Implemented + replay/native smoke tested; gameplay unverified | Shared native samples -> evidence -> finding -> UI -> focused Proof experiment; attribution and limits visible |
| ATLAS-2 CPU/GPU detail | Partial existing sensors | Per-process/core evidence and representative gameplay qualification still required |
| ATLAS-3 storage/VRAM | Not implemented | Real attributed counters and device-aware tests |
| ATLAS-4 thermal/OBS | Not implemented | Optional supported sources, authenticated local OBS, secret-safe storage and reset-safe deltas |
| ATLAS-5 full experience | First slice implemented/tested; full experience partial | Accessible findings/evidence/incidents; full multi-lane history and durable incident retention later |
| ATLAS-6 experiments | Neutral descriptive reporting + linkage implemented/tested; outcome model partial | Full comparable-workload, repeated-trial outcomes and durable per-game linkage later |
| ATLAS-7 release | Unverified | Live integrations, seven operating-mode overhead comparisons, packaging, stable-host matrix |

Production claims remain gated: deterministic replay is not Windows qualification. Initial Atlas is an engineering alpha with a memory investigation vertical slice, not the completed six-domain product.

## Tests and budgets

Run frontend tests/build, Rust format, all-target strict Clippy and Rust tests. Native tests must distinguish read-only counters and IPC cancellation from mutating tests; no incidental power or memory mutation for Atlas qualification. Test QPC delay/missing/future/drift mapping; stale/missing/NaN sensors; bounded buffer and discontinuities; high utilization with stable frames; healthy cache; sustained pressure with paging; contradictory recovery; incident correlation windows and target identity; neutral Proof deltas; paused/stale Mini HUD; stale settings conflict.

Budget targets (not measurements): no extra threads/capture processes; <=90 observation ticks and <=64 incidents; reuse sampler cadence (visible 1s / hidden 5s defaults); Atlas evaluation p95 <1ms, additional retained serialized data <1MiB. Measure replay execution separately from native CPU/private bytes/working set/disk and WebView2 overhead. Full seven-mode V4.2 comparison requires a matching release baseline and actual gameplay/capture sources; do not substitute a microbenchmark.

## Blockers and continuation

- No stable Windows matrix or live PresentMon gameplay evidence supplied. Probe local host/provider availability; record exact method and limits.
- No OBS, thermal, storage-latency or game-VRAM sources; honest unavailable domains can ship with the first slice.
- IPC cancellation drain may exceed the request deadline: storage lifetime safety takes priority over returning while OS still owns buffers. Native pending accept/read/write, shutdown/deadline and delayed-reader tests passed; documentation updated.
- Full statistically defensible Proof, local redacted recorder, persisted incident history and intervention outcome feedback are later milestones.

## 2026-10-19 execution results and continuation ledger

Implemented: `atlas.rs` shared source metadata/normalization, 90 native ticks, 64 bounded investigations and 128 dedup IDs; sustained joint memory/paging conditions, healthy cache, no reliable cause, explicit gaps and recovery contradictions; Stutter -> surrounding window -> traceable finding -> linked descriptive Proof baseline. No intervention is executed by the engine or experiment button. Final Proof results hydrate after dashboard recreation within the same process; per-game durable outcomes and direct Rewind outcome attachment are deferred.

Integrity fixes: removed unused test method; canceled IPC drains before storage release; resets manual events; bounded client-release wait prevents unread reply discard (a native integration rerun reproduced the old reply race, then the new 75ms delayed-reader regression passed). QPC/relative event timestamps are retained independently of arrival, reader state is isolated per capture, first swapchain is selected explicitly and stale/unmapped provider events cannot produce current FPS. Stutter avoids per-frame baseline sorting and resets on mapped gaps. Proof removes favorable resource-usage classification and unsupported noise claims, excludes initial 5s and requires 30s per phase/five valid samples, refuses active-session replacement and retains final linked reports. HUDs validate source freshness, pause and new frames between UI clock ticks. Whole-settings writes reject stale snapshots; Mini HUD writes current fields under serialized persistence.

Validation: 11 frontend tests and strict TypeScript/Vite build pass. Rust format and strict all-target Clippy pass. Native suite status: 68 passing tests with the one existing actual power-scheme mutation test explicitly excluded; final result recorded in docs/TESTING.md. New native tests are read-only or mock the fixed purge callback; no UAC, standby purge, raw native NTSTATUS or system power mutation is claimed. Local CI steps were exercised; remote GitHub CI, installer rebuild and signing were not run.

Host: x64 build 26340.9750, registry edition Core, medium integrity. RTX 4060 Ti/LUID 0x00000000_0x00017a11. Three 1-second actual memory/CPU/WDDM samples passed through Atlas; details are in docs/WINDOWS_API_COMPATIBILITY.md. PresentMon discovery returned absent. Stable Windows qualification and real frame-event/UTC alignment remain unverified.

Measured isolated optimized replay: 1,000 synthetic ticks, evaluation p50 179us / p95 255us / max 548us; final serialized report 363,496 bytes at 90 ticks / 64 incidents. This is engine/transport-size evidence only. Native/WebView2 CPU, private bytes, working set, disk, capture overhead and full seven-mode baseline comparison remain unmeasured. Frontend trend history omits repeated Atlas incident payloads. No performance or FPS improvement is claimed.

UI: synthetic fixture rendered in Edge at the default and 900px viewport; simple/advanced evidence disclosure verified. A real freshness flicker and unstyled incident selector found during QA were corrected. Temporary preview file/server/tab were removed; no synthetic preview is shipped. Native WebView2/tray/DPI/keyboard/Mini HUD interaction remains unverified.

Repository: baseline HEAD remains de3d9240d9cf74abb398caa35efd1d6877185b7a on main; initially clean, all current modifications belong to this session. No commit/push/PR created; no dependencies changed; sources/ unchanged. Final optimized executable build passed after the last source/frontend changes: `src-tauri/target/release/memflux.exe`, 3,860,992 bytes; SHA-256 `4E3A4C5DB55CB71CFFA734A14B3C24767ED828FAD870D3B5E156CB1952AC527E`. Command: `cargo build --manifest-path src-tauri/Cargo.toml --release` (final compilation 1m 56s). The executable was compiled, not launched for a new native purge or power mutation. Installer bundles/signing and remote CI were not rerun.

Next steps, in order:
1. Qualify PresentMon v1-compatible raw QPC output against actual gameplay, buffering, focus changes, PID reuse and measured drift. Production root-cause claims remain disabled by conservative rules.
2. Add attributed CPU/process/core and physical-storage latency evidence; keep one sampler/query infrastructure. Add real graphics-memory sources before VRAM claims.
3. Add an explicit redacted local recorder/replay schema and bounded persisted incident summaries (current incident history is in-memory only).
4. Add comparable-workload/target/capture quality checks, repeated trials, uncertainty-aware primary outcomes and durable hypothesis/intervention/Rewind linkage.
5. Optional authenticated OBS and supported thermal providers, then full unified timeline and compact HUD summary. No duplicate capture pipeline.
6. Profile the seven operating modes against a matching V4.2 baseline; stable OS matrix, packaging and native UI qualification before beta promotion.

Product maturity: **Bottleneck Atlas engineering alpha**, with a working first memory investigation slice. Gates A/B/C are locally regression-tested foundations with live-source attribution limitations; D is partial; E unavailable/partial; F/G partial; H/I remain unqualified. This does not supersede historical cleaner qualification or authorize new mutable GameFocus/PowerShift behavior.
