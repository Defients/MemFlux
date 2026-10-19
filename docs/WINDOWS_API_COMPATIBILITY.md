# Windows API Compatibility

## Capability matrix

| Capability | Interface | Support status | Privilege | Product state |
|---|---|---|---|---|
| Physical total, available, load | `GlobalMemoryStatusEx` | Documented, Windows XP+ desktop | Standard user | Implemented |
| Commit, physical pages, process/thread counts | `GetPerformanceInfo` | Documented, Windows XP+ desktop | Standard user | Implemented with commit fallback |
| Low/high system memory condition | `CreateMemoryResourceNotification`, `QueryMemoryResourceNotification` | Documented, Windows XP+ desktop | Standard user | Implemented; neutral band supported |
| Standby/free/modified/page-read counters | PDH with `PdhAddEnglishCounterW` | Documented interface; counters can be absent | Standard user normally | Implemented with independent availability |
| Full standby-list clearing | `NtSetSystemInformation(80, command 4)` | `NtSetSystemInformation` is documented as an ntdll export; class 80 and command 4 are private/compatibility-sensitive | `SeProfileSingleProcessPrivilege` | Implemented behind build allowlist with one-shot helper + authenticated Named Pipe session helper |
| Session helper IPC & peer identity | `CreateNamedPipeW`, `GetNamedPipeClientProcessId`, `GetNamedPipeServerProcessId`, SDDL DACL | Documented Win32 Pipes & Security APIs | Standard user client / Elevated server | Implemented and verified on build 26340 |
| CPU utilization | `GetSystemTimes` delta | Documented Win32 API | Standard user | Implemented (`(kernel + user - idle) / (kernel + user)`) |
| GPU adapter discovery | DXGI `CreateDXGIFactory1` / `IDXGIFactory1::EnumAdapters1` | Documented DXGI 1.1+ API | Standard user | Implemented and verified (`NVIDIA GeForce RTX 4060 Ti`) |
| GPU engine utilization | PDH `\GPU Engine(*)\Utilization Percentage` (`PdhGetFormattedCounterArrayW`) | Documented WDDM 2.0+ performance counter | Standard user | Implemented; sums per-PID on same physical engine and reports busiest engine |
| Power scheme enumeration & control | `PowerEnumerate`, `PowerGetActiveScheme`, `PowerSetActiveScheme`, `PowerReadFriendlyName`, `GetSystemPowerStatus` | Documented Win32 PowrProf APIs | Standard user | Implemented with Optimization Rewind journal and compare-before-restore |
| Non-injecting frame capture | External Intel PresentMon ETW process | Requires installed PresentMon binary + ETW privileges | Standard/Performance Log Users | Implemented adapter; reports explicit Unavailable when PresentMon binary is absent |

## Tested configuration

- **Host:** Windows 11 Home Insider Preview `10.0.26340.9750`, x64, NVIDIA GeForce RTX 4060 Ti (`LUID 0x00000000_0x00017a11`).
- **Standby purge:**
  - Elevated helper result: `STATUS_SUCCESS (0x00000000)`. Retained sample: standby `233,820,160 → 25,538,560` bytes; Free & Zero `10,254,319,616 → 10,510,528,512` bytes after 750 ms.
  - Unelevated helper result: `STATUS_PRIVILEGE_NOT_HELD (0xC0000061)`.
- **Authenticated Named Pipe Session Helper:**
  - Verified SDDL security descriptor creation, `PIPE_REJECT_REMOTE_CLIENTS`, `GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId` matching, executable image verification, `Ping`, `PurgeStandbyList`, and `Shutdown` round-trip.
- **DXGI + WDDM GPU Engine:**
  - Verified adapter enumeration (`NVIDIA GeForce RTX 4060 Ti`, 8 GiB dedicated VRAM) and live two-sample PDH `\GPU Engine(*)\Utilization Percentage` collection.
- **PowerShift + Optimization Rewind:**
  - Verified enumeration of `Balanced` (`381b4222-f694-41f0-9685-ff5bb260df2e`), `High performance` (`8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c`), and `Power saver` (`a1841308-3541-4fab-bc81-f71556f20b4a`), live `PowerSetActiveScheme` switch + `PowerGetActiveScheme` verification, and exact restoration to the original scheme via `RewindJournal`.

Stable Windows 10 (`19045`) and Windows 11 (`22631`, `26100`) builds remain marked `unverified` in the compatibility matrix until controlled runtime tests are executed on those hosts.
