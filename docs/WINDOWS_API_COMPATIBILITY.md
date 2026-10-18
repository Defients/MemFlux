# Windows API Compatibility

## Capability matrix

| Capability | Interface | Support status | Privilege | Product state |
|---|---|---|---|---|
| Physical total, available, load | `GlobalMemoryStatusEx` | Documented, Windows XP+ desktop | Standard user | Implemented |
| Commit, physical pages, process/thread counts | `GetPerformanceInfo` | Documented, Windows XP+ desktop | Standard user | Implemented with commit fallback |
| Low/high system memory condition | `CreateMemoryResourceNotification`, `QueryMemoryResourceNotification` | Documented, Windows XP+ desktop | Standard user | Implemented; neutral band supported |
| Standby/free/modified/page-read counters | PDH with `PdhAddEnglishCounterW` | Documented interface; counters can be absent | Standard user normally | Implemented with independent availability |
| Full standby-list clearing | `NtSetSystemInformation(80, command 4)` | `NtSetSystemInformation` is documented as an ntdll export; class 80 and command 4 are private/compatibility-sensitive | `SeProfileSingleProcessPrivilege`; current filtered token does not hold it | Implemented behind build allowlist with one-shot UAC helper |
| Low-priority standby clearing | Command 5 in public native-header research | Not independently validated here | Same privilege boundary expected | Not exposed |
| Process working-set trimming | `EmptyWorkingSet` | Documented, but not standby clearing | Target process rights | Not used |
| Timer-period request | `timeBeginPeriod` / `timeEndPeriod` | Documented, modern process-scoped semantics | Standard user | Planned research only; disabled |

## Tested configuration

- Windows 11 Home Insider Preview 10.0.26340, x64.
- Standard-user monitoring intent; no application manifest requests elevation.
- Windows SDK 10.0.26100.0 and WebView2 155.0.4283.62 present on the build host.
- Local Performance Counter probing confirmed every Phase 1 PDH path before implementation.
- Full standby purge runtime result: `STATUS_SUCCESS (0x00000000)`. Retained sample: standby 233,820,160 → 25,538,560 bytes; Free & Zero 10,254,319,616 → 10,510,528,512 bytes after 750 ms.
- Unelevated helper result: `STATUS_PRIVILEGE_NOT_HELD (0xC0000061)`.

This host is not representative of stable Windows 10/11 releases. The action allowlist currently contains only build 26340; compile or runtime success here is not generalized.

## Fallbacks

- `GlobalMemoryStatusEx` is core. If it fails, the snapshot is Error and policy evaluation suspends.
- If `GetPerformanceInfo` fails, commit values fall back to `MEMORYSTATUSEX` while process/thread counts remain unavailable.
- Standby total is shown only when all three standby-class counters are valid; partial sums are rejected.
- A rate counter reports Warming Up until two PDH collections exist.
- Resource notification failures produce an Unavailable OS signal without inventing Low, High, or Neutral.

## Authoritative references

- [GlobalMemoryStatusEx](https://learn.microsoft.com/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)
- [GetPerformanceInfo](https://learn.microsoft.com/windows/win32/api/psapi/nf-psapi-getperformanceinfo)
- [CreateMemoryResourceNotification](https://learn.microsoft.com/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification)
- [QueryMemoryResourceNotification](https://learn.microsoft.com/windows/win32/api/memoryapi/nf-memoryapi-querymemoryresourcenotification)
- [NtQuerySystemInformation limitations](https://learn.microsoft.com/windows/win32/api/winternl/nf-winternl-ntquerysysteminformation)
- [NtSetSystemInformation](https://learn.microsoft.com/windows/win32/sysinfo/ntsetsysteminformation)
- [AdjustTokenPrivileges](https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-adjusttokenprivileges)
- [ShellExecuteEx / `runas`](https://learn.microsoft.com/windows/win32/api/shellapi/ns-shellapi-shellexecuteinfow)
- [System Informer native header: class 80 and memory-list commands](https://github.com/winsiderss/phnt/blob/master/ntexapi.h)
- [RAMMap](https://learn.microsoft.com/sysinternals/downloads/rammap)
- [EmptyWorkingSet](https://learn.microsoft.com/windows/win32/api/psapi/nf-psapi-emptyworkingset)
- [timeBeginPeriod](https://learn.microsoft.com/windows/win32/api/timeapi/nf-timeapi-timebeginperiod)

