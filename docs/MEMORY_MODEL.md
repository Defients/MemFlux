# Memory Model

## Implemented definitions

- **Total physical:** usable physical memory returned by Windows.
- **Available physical:** `MEMORYSTATUSEX.ullAvailPhys`. This already includes reclaimable standby, free, and zero pages.
- **Non-available physical:** `max(total physical - available physical, 0)`. This is a display estimate, not a claim that every byte is application working-set memory.
- **Physical load:** the Windows `dwMemoryLoad` estimate.
- **Commit used / limit:** `GetPerformanceInfo` page counts multiplied by page size. `MEMORYSTATUSEX` page-file fields are the fallback because they represent commitment values in this API, not the physical pagefile file size alone.
- **Commit percent:** commit used divided by commit limit, bounded to 0–100%.
- **Standby:** the sum of PDH Standby Cache Reserve, Normal Priority, and Core Bytes, shown only when all components are available.
- **Hard page reads:** PDH `Memory\\Page Reads/sec`. Generic page faults are deliberately not labeled hard faults.

## Composition rule

The physical-memory bar uses mutually exclusive segments:

```text
total = non-available + standby + free/zero + other available
other available = max(available - standby - free/zero, 0)
```

If detailed counters are unavailable, it degrades to:

```text
total = non-available + available
```

Modified pages are displayed as a diagnostic metric but not added as another composition segment because that would overlap the non-available side of this simplified model.

## Interpretation boundaries

- A large standby cache is not itself pressure and is usually beneficial caching.
- More free memory is not automatically better performance.
- Page-read activity is rate evidence and needs context; one sample is not a diagnosis.
- Commit pressure and physical pressure are related but distinct.
- Values are volatile. Cross-tool comparisons must allow for sampling time differences.

