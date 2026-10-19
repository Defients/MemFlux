# Security and Privacy

## Threat boundary

MemFlux monitors local Windows memory, CPU, GPU, and power state, performs privilege-gated standby-list purging, and applies reversible Windows PowrProf power-scheme changes. It does not install kernel drivers, inject into game processes, weaken Windows Defender/HVCI/firewall settings, or upload analytics.

## Implemented controls

- **Standard-user main process:** The UI and telemetry runtime run unelevated.
- **One-shot manual helper:** Launches the current executable with `--memflux-elevated-purge-standby`, enables only `SeProfileSingleProcessPrivilege`, executes `NtSetSystemInformation(80, 4)`, and exits with raw `NTSTATUS`.
- **Authenticated elevated session helper:**
  - Requires explicit user UAC approval (`authorize_cleaner_session`). Background threshold triggers never initiate UAC prompts.
  - Creates a local Windows Named Pipe (`\\.\pipe\MemFluxSession_<pid>_<timestamp>`) with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, `nMaxInstances = 1`, and an SDDL DACL restricted to `SYSTEM`, `Builtin Administrators`, and the launching user's SID.
  - Verifies peer identity on both ends using `GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId` and confirms via `QueryFullProcessImageNameW` that the peer process image matches the current executable.
  - Accepts only versioned, length-bounded (`<= 2048` bytes) JSON requests (`#[serde(deny_unknown_fields)]`) for three enumerated operations: `Ping`, `PurgeStandbyList`, and `Shutdown`.
  - Runs a dedicated watchdog thread holding a `SYNCHRONIZE` handle to the controller process (`WaitForSingleObject`), ensuring the helper exits immediately if the main application crashes or terminates, and enforces a 4-hour maximum session lifetime.
- **Optimization Rewind safety:**
  - Every PowerShift mutation is recorded in `rewind-journal.json` before execution and verified via `PowerGetActiveScheme` after execution.
  - Compare-before-restore checks live Windows state before restoring; if the user externally changed the power scheme while a session was active, MemFlux preserves the user's external choice and logs `conflict_skipped`.
  - Startup crash recovery inspects `rewind-journal.json` and safely restores any leftover active session state.
- **Local-first storage:** Settings, action history, cleaner quota, and rewind journal use fixed application config paths and atomic `MoveFileExW` replacement. No telemetry or analytics leave the machine.

## Remaining security & release considerations

- **Code signing:** Build artifacts produced without a signing certificate are explicitly labeled unsigned development/beta artifacts and will trigger Windows SmartScreen prompts until signed with a trusted publisher certificate.
- **Windows build allowlist:** Private memory-list purging (`NtSetSystemInformation` class 80, command 4) remains gated behind `SUPPORTED_BUILDS` (`[26340]`) until controlled runtime qualification is recorded on additional target Windows 10/11 builds.

## Atlas integrity boundary (2026-10-19)

Atlas is read-only. It cannot request UAC, change power/process state or bypass cleaner eligibility. The existing build allowlist and fixed-operation authenticated helper protocol remain unchanged. Synthetic replay exists only in tests; no replay values feed production telemetry. No new network connection, vendor driver or OBS credential storage was added.

Overlapped pipe timeout/cancellation requests CancelIoEx then drains GetOverlappedResult completion before buffer/event/OVERLAPPED storage can be released. Drain completion may exceed the nominal request deadline; cancellation alone is never treated as native action completion. Events are reset before reuse. After writing a reply, the helper waits for the one-request client connection to close (bounded cancellable read), because DisconnectNamedPipe discards unread data. No second operation is dispatched on that connection and no unbounded FlushFileBuffers was added.

Whole-settings UI writes compare an expected snapshot while holding the control lock through atomic persistence and memory application; stale writes fail. Native Mini HUD toggles/position/close update the current snapshot under the same lock. HUD position is independently owned and preserved during UI saves.

Native contract references: [CancelIoEx completion/lifetime](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex), [DisconnectNamedPipe unread-data behavior](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-disconnectnamedpipe).
