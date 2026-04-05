# inferno-windows

Windows port of [Inferno](https://github.com/teodly/inferno) — an unofficial open-source implementation of the Dante AoIP protocol.

`inferno_wasapi.exe` receives audio from a Dante network and makes it available as a Windows audio device using [VB-Cable](https://vb-audio.com/Cable/) as a virtual audio bridge.

## Quick Start

### 1. Build

```powershell
cargo build --release -p inferno_wasapi
```

### 2. Open firewall (run as Administrator)

```powershell
.\scripts\open-firewall-admin.ps1
```

### 3. Install VB-Cable (free virtual audio device)

Download from **https://vb-audio.com/Cable/** → run `VBCABLE_Setup_x64.exe` as Administrator → reboot.

After install, Windows will show two new devices:
- **CABLE Input** — a playback device (inferno_wasapi writes Dante audio here)
- **CABLE Output** — a recording device (your apps read Dante audio here)

### 4. Run

```powershell
# Route Dante audio through VB-Cable (appears as Windows sound device):
.\target\release\inferno_wasapi.exe --virtual-device

# Or play directly through your default speakers:
.\target\release\inferno_wasapi.exe

# Use a specific device by name:
.\target\release\inferno_wasapi.exe --device "CABLE Input"

# List available WASAPI devices:
.\target\release\inferno_wasapi.exe --list-devices
```

### 5. Connect in Dante Controller

Open **Dante Controller**, find your device (named after your hostname), and route TX channels to its RX channels.

### 6. Use in any Windows app

In OBS, Audacity, browser, or any app — select **"CABLE Output"** as the audio input device to receive Dante audio.

## Options

| Flag | Description |
|------|-------------|
| `--virtual-device` | Auto-detect VB-Cable (CABLE Input) and route audio there |
| `--device <name>` | Use a specific WASAPI device (substring match) |
| `--name <name>` | Device name shown in Dante Controller (default: hostname) |
| `--channels <n>` | Number of receive channels (default: 2) |
| `--sample-rate <hz>` | Sample rate in Hz (default: 48000) |
| `--list-devices` | List available WASAPI output devices and exit |

## Architecture

```
Dante network (UDP multicast + unicast RTP)
        ↓
inferno_aoip (Rust) — Dante protocol stack
        ↓ receive_with_callback
inferno_wasapi (Rust) — converts 24-bit i32 → f32, renders via WASAPI
        ↓ WASAPI Shared mode
"CABLE Input" (VB-Cable virtual device)
        ↓
"CABLE Output" → any Windows app (OBS, browser, Audacity, DAW...)
```

Without `--virtual-device`, audio plays directly through your default speakers instead of VB-Cable.

## Notes

See `NOTES.md` for known issues and limitations, especially regarding PTP clock synchronization.

