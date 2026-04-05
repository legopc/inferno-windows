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

Tested on Windows 10 with Realtek Audio + VB-Cable:
- `inferno_wasapi --list-devices` — enumerates WASAPI devices correctly
- `inferno_wasapi --virtual-device` — routes Dante audio to "CABLE Input"; any app can read from "CABLE Output"
- `inferno_wasapi` — plays directly through default speakers
- Device name (`F53ZDD3`) and IP (`192.168.1.37`) are detected from system
- State storage uses `AppData\Local\inferno_aoip\` (normal on first run to warn about missing state file)
- Dante device appears in **Dante Controller** once firewall ports are open
- Channel subscriptions confirmed working (RX 1, RX 2)
- Audio flows Dante → inferno_wasapi → WASAPI (f32 format, 48kHz stereo)

## Virtual Audio Device

**Current approach**: VB-Cable (free donationware, https://vb-audio.com/Cable/) is used as a
virtual audio cable. inferno_wasapi renders Dante audio to "CABLE Input"; Windows apps see
"CABLE Output" as a standard recording device.

Install: `.\scripts\install-vbcable.ps1` (requires admin)

Run: `.\target\release\inferno_wasapi.exe --virtual-device`

**Open-source alternative**: [VirtualDrivers/Virtual-Audio-Driver](https://github.com/VirtualDrivers/Virtual-Audio-Driver)
is a SYSVAD-based open-source WDM virtual audio driver. It requires:
- Windows Driver Kit (WDK) to build
- Test signing mode (`bcdedit -set TESTSIGNING ON`, Secure Boot disabled)
- More setup complexity than VB-Cable

VB-Cable is recommended for ease of use. The open-source driver is an option for
distributions that cannot include a donationware dependency.

**Kernel driver (full self-contained, no VB-Cable)**: A proper WDM driver built from
[Microsoft SYSVAD](https://github.com/microsoft/windows-driver-samples/tree/main/audio/sysvad)
with a shared-memory IPC to the Rust user-mode service would eliminate the VB-Cable
dependency entirely. This requires kernel C/C++ development, WDK, and an EV code signing
certificate for production distribution. See git history for the detailed plan.

## Audio Sample Format

inferno_aoip delivers audio as `i32` (type alias `Sample`) with 24-bit values stored
**left-justified** (top 24 bits used, bottom 8 bits = 0), range ±2³¹. This is documented
in `inferno_aoip/src/device_server/samples_utils.rs` lines 84-90.

WASAPI Shared mode on modern Windows requires **f32** (IEEE float, range [-1.0, 1.0]).
Conversion: `sample as f32 / 2147483648.0_f32`

## Unknown ARC Opcode 0x2204

Dante Controller sends opcode `0x2204` which is not implemented in inferno_aoip.
These `received unknown opcode1 0x2204` log errors are harmless — they are informational
queries from Dante Controller that don't affect audio flow.

