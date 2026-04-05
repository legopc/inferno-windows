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

UDP ports that must be open for Dante to work:
- 4455 — Dante audio RX flows
- 8700 — Dante audio TX flows  
- 4400 — Dante ARC (Audio Routing & Control)
- 8800 — Dante CMC
- 5353 — mDNS (Multicast DNS for device discovery)

Also allow incoming UDP from all ports (Dante transmitter source ports are OS-assigned).
