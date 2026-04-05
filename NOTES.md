# Inferno Windows — Known Issues & Notes

## PTP Clock Synchronization (Statime)

**Status**: Not implemented on Windows.

Dante protocol requires PTP (Precision Time Protocol) clock synchronization for
audio streaming. On Linux, this is handled by a fork of
[Statime](https://github.com/teodly/statime) which communicates via Unix domain
sockets using the `usrvclock` protocol.

On Windows:
- The `usrvclock` crate in this workspace is a **stub** that never delivers
  clock overlays. The `ClockOverlay::now_ns()` method uses `SystemTime` instead
  of a PTP-adjusted clock.
- `AsyncClient::start()` spawns a no-op task that waits for shutdown without
  connecting to any clock daemon.
- This means audio timestamps will be based on system time, not a PTP clock.

**Consequence**: Audio will receive (Dante device will appear in Dante Controller
and audio connections can be made), but playback may have timing drift or
glitches due to lack of PTP synchronization.

**Future work to enable PTP on Windows**:
1. Port Statime to Windows or find a Windows PTP daemon
2. Implement `AsyncClient` using a named pipe or UDP socket to receive clock overlays
3. Use `windows::Win32::System::Performance::QueryPerformanceCounter` as the
   underlying clock instead of SystemTime for sub-millisecond accuracy

## WASAPI Integration

The `inferno_wasapi` binary uses Windows Audio Session API (WASAPI) in shared
mode. For lower latency, exclusive mode could be used, but shared mode is
safer for general use.

## Firewall

UDP ports that must be open for Dante to work (from inferno_aoip source code):
- **4440** — Dante ARC (Audio Routing & Control) — `proto_arc.rs`
- **4455** — Dante flows control — `flows_control.rs`
- **8700** — Dante info/mcast requests — `mcast.rs`
- **8800** — Dante CMC — `proto_cmc.rs`
- **5353** — mDNS (Multicast DNS for device discovery)

Also allow incoming UDP from all ports (Dante transmitter source ports are OS-assigned).

**Opening the firewall (requires admin)**:
```powershell
# From an elevated PowerShell window:
.\scripts\open-firewall-admin.ps1
```
Or manually:
```
netsh advfirewall firewall add rule name="Inferno-Dante UDP 4455" protocol=UDP dir=in localport=4455 action=allow
netsh advfirewall firewall add rule name="Inferno-Dante UDP 8700" protocol=UDP dir=in localport=8700 action=allow
netsh advfirewall firewall add rule name="Inferno-Dante UDP 4400" protocol=UDP dir=in localport=4400 action=allow
netsh advfirewall firewall add rule name="Inferno-Dante UDP 8800" protocol=UDP dir=in localport=8800 action=allow
netsh advfirewall firewall add rule name="Inferno-Dante UDP 5353" protocol=UDP dir=in localport=5353 action=allow
```

## Runtime Status (confirmed)

Tested on Windows 10 with Realtek Audio:
- `inferno_wasapi --list-devices` — enumerates WASAPI devices correctly
- `inferno_wasapi` — starts, detects IP/hostname, initializes WASAPI, begins streaming loop
- Device name (`F53ZDD3`) and IP (`192.168.1.37`) are detected from system
- State storage uses `AppData\Local\inferno_aoip\` (normal on first run to warn about missing state file)
- Without PTP sync, `clock_synced` stays false → output is silence (see PTP section above)
- Dante device **should appear in Dante Controller** once firewall ports are open
