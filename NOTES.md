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

## Virtual Audio Device (TX — Send Windows Audio to Dante)

**Current approach**: [Microsoft SYSVAD `TabletAudioSample`](https://github.com/microsoft/Windows-driver-samples/tree/main/audio/sysvad)
— an open-source (MIT) WDM kernel audio driver that registers a virtual speaker endpoint.

- Windows apps route audio to the SYSVAD virtual speaker
- `inferno_wasapi --tx` captures that audio via WASAPI loopback and sends it over Dante
- No VB-Cable or other third-party tools required

Build and install: `.\scripts\install-sysvad.ps1` (requires admin, WDK, VS2022, test signing)

See `SETUP.md` for full setup instructions including test signing and WDK installation.

**Why WASAPI loopback works without modifying SYSVAD**: WASAPI loopback taps the Windows
audio engine mix stream *before* it reaches the WDM driver DMA buffer. SYSVAD can discard
its DMA buffer as-is; the loopback is handled at a higher level by the OS.

**Future — Shared memory bridge (lower latency)**: Modify SYSVAD to write the DMA render
buffer into a named Windows shared memory section (`CreateFileMapping`). Rust reads via
`OpenFileMapping` directly — eliminates the WASAPI overhead and reduces latency.

**Driver signing for distribution**: Test signing is for development only. Production
distribution requires attestation signing via Microsoft Partner Center with an EV code
signing certificate. See:
https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/attestation-signing-a-kernel-driver-for-public-release

**VB-Cable (legacy)**: Previously used as a virtual audio cable for routing Dante RX audio
to other Windows apps. The SYSVAD-based TX approach replaces this for the send direction.
VB-Cable scripts remain in `scripts/install-vbcable.ps1` for RX routing use cases.

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

