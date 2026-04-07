# PTP Clock Synchronization on Windows

## Clock Strategy

InfernoAoIP uses a tiered clock strategy:

### Tier 1: SafeClock (default, always available)
- QPC-based free-running clock with `GetSystemTimePreciseAsFileTime` anchor
- ~100ns resolution, no network dependency
- Activated automatically when no PTP grandmaster is detected
- Set `CLOCK_OFFSET_NS` env var to trim offset (default: -500000ns)

### Tier 2: W32TM PTP (Windows 11 built-in, PTPv2 only)
Windows 11 includes a PTPv2 client via `ptpprov.dll`. Configure via:
```
w32tm /config /manualpeerlist:"<grandmaster-ip>" /syncfromflags:manual /update
net stop w32tm && net start w32tm
```
**Limitation**: Dante uses PTPv1 by default. W32TM PTPv2 is incompatible with
standard Dante PTPv1 grandmasters.

### Tier 3: PTPSync (PTPv1, open source)
For networks with PTPv1 grandmasters (standard Dante networks):
1. Download PTPSync from: https://github.com/GridProtectionAlliance/PTPSync
2. Install as Windows service
3. Configure with your grandmaster IP
4. PTPSync disciplines the Windows clock to PTPv1 — InfernoAoIP then reads
   the disciplined time via `GetSystemTimePreciseAsFileTime`

### Tier 4: teodly/statime port (future)
The Linux inferno uses a `teodly/statime` fork with PTPv1 fixes and MONOTONIC
clock support. Porting to Windows is a 2-3 month effort but would give
full in-process PTP without external tools.

## Testing Clock Accuracy
Run with `DANTE_DEBUG_PACKETS=1` and check RTP timestamp consistency in logs.
SafeClock drift is typically <1ppm over short periods (seconds), adequate for
Dante latency targets of 1-10ms.
