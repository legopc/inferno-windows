# inferno-windows

Windows port of [Inferno](https://github.com/teodly/inferno) — an unofficial open-source implementation of the Dante AoIP protocol.

[![Release](https://img.shields.io/github/v/release/legopc/inferno-windows)](https://github.com/legopc/inferno-windows/releases)

**Receive Dante AoIP audio and play via Windows (RX mode)** or **capture Windows audio and transmit as Dante AoIP (TX mode)**.

## Quick Start

### Prerequisites

- **SYSVAD** (Windows Virtual Audio Device Driver) — required for TX mode
  - See [SETUP.md](SETUP.md) for installation instructions
- **VB-Cable** (optional) — virtual audio bridge for routing Dante audio to apps
  - Download: https://vb-audio.com/Cable/

### 1. Build

```powershell
cargo build --release -p inferno_wasapi
```

### 2. Open Firewall (Admin)

Run with the `--setup-firewall` flag to auto-configure netsh rules:
```powershell
.\target\release\inferno_wasapi.exe --setup-firewall
```

Alternatively, use the legacy script:
```powershell
.\scripts\open-firewall-admin.ps1
```

### 3. RX Mode (Receive Dante → Play Audio)

```powershell
# Play directly through default speakers
.\target\release\inferno_wasapi.exe

# Route through VB-Cable virtual device (apps see "CABLE Output")
.\target\release\inferno_wasapi.exe --virtual-device

# Use specific device
.\target\release\inferno_wasapi.exe --device "CABLE Input"

# List available WASAPI devices
.\target\release\inferno_wasapi.exe --list-devices
```

Connect in **Dante Controller** and route TX channels from Dante devices to your RX channels.

### 4. TX Mode (Capture Windows Audio → Transmit as Dante)

```powershell
# Capture Windows audio and transmit
.\target\release\inferno_wasapi.exe --tx

# Use specific loopback device
.\target\release\inferno_wasapi.exe --tx --tx-device "Stereo Mix"

# List available TX devices
.\target\release\inferno_wasapi.exe --list-tx-devices
```

### 5. Use in Windows Apps (RX)

Select **"CABLE Output"** in OBS, Audacity, browser, DAW, etc. to receive Dante audio.

## CLI Reference

### Discovery & Listing

| Flag | Purpose |
|------|---------|
| `--list-devices` | List WASAPI output devices |
| `--list-tx-devices` | List WASAPI render devices for TX loopback |
| `--list-dante-devices` | Discover Dante devices via mDNS |
| `--list-interfaces` | List network interfaces with IPs |

### Setup & Admin

| Flag | Purpose |
|------|---------|
| `--setup-firewall` | Configure Windows Firewall rules for Dante ports and exit |

### RX Mode Options

| Flag | Description |
|------|-------------|
| `--device <NAME>` | WASAPI device name (substring match) |
| `--channels <N>` | Number of RX channels (default: 2) |
| `--sample-rate <HZ>` | Sample rate (default: 48000) |
| `--virtual-device` | Route to VB-Cable virtual device |
| `--lock` | Lock device from Dante Controller changes |

### TX Mode Options

| Flag | Description |
|------|-------------|
| `--tx` | Enable TX mode (capture Windows audio) |
| `--tx-device <NAME>` | TX loopback source device (substring match) |
| `--tx-channels <N>` | TX channel count |
| `--virtual-device` | Route output to VB-Cable |

### Dante Controller

| Flag | Description |
|------|-------------|
| `--name <NAME>` | Device name shown in Dante Controller (default: hostname) |
| `--lock / --unlock` | Lock/unlock device from Controller changes |

### GUI & Service

| Flag | Description |
|------|-------------|
| `--tray` | Show system tray icon |
| `--install-service` | Install as Windows Service (requires admin) |
| `--uninstall-service` | Uninstall Windows Service |
| `--service` | Run as service (internal use) |

GUI: `inferno_gui.exe` — Win32 status window, IPC-connected, no GPU required

### Utilities

| Tool | Purpose |
|------|---------|
| `inferno2pipe.exe` | Pipe Dante audio to stdout as raw PCM |

## Configuration & Logging

**Config file:** `%LOCALAPPDATA%\inferno_aoip\config.toml` (auto-created on first run)

**Supported sample rates:**
- 44100 Hz
- 48000 Hz (default)
- 96000 Hz (⚠️ Dante hardware limit: max 32 channels at 96kHz; see NOTES.md for details)

**Logs:** `%LOCALAPPDATA%\inferno_aoip\logs\inferno.log` (daily rolling)

**Metrics:** Prometheus metrics on `http://localhost:9090/metrics`

## Environment Variables

```bash
RUST_LOG=debug                  # Log level (debug, info, warn, error)
DANTE_DEBUG_PACKETS=1           # Hex dump RTP packets
CLOCK_OFFSET_NS=N               # Manual clock trim in nanoseconds
DANTE_BITS_PER_SAMPLE=24        # Bit depth (16/24/32)
DANTE_MAX_FLOWS=8               # Max parallel flows (1-64)
```

## Architecture

```
RX Path:
  Dante network (UDP RTP) → inferno_aoip (protocol stack)
    → inferno_wasapi (WASAPI renderer) → speakers / VB-Cable / app input

TX Path:
  Windows audio (WASAPI loopback) → inferno_wasapi (capture)
    → inferno_aoip (protocol stack) → Dante network (UDP RTP)

GUI:
  inferno_gui.exe ↔ inferno_wasapi (IPC) — Win32 status window

Service:
  Windows Service wrapper → inferno_wasapi (--service flag)

Prometheus:
  :9090/metrics endpoint — metrics, alerts, monitoring
```

## Troubleshooting

**No audio output (RX mode)**
- Verify Dante devices are on the network: `--list-dante-devices`
- Check network connectivity: `--list-interfaces`
- Enable debug logging: `RUST_LOG=debug`
- Verify Dante Controller routing

**TX mode fails or no audio transmitted**
- SYSVAD driver must be installed (see SETUP.md)
- List TX devices: `--list-tx-devices`
- Try explicit device: `--tx --tx-device "Stereo Mix"`
- Check Windows device settings for loopback capture

**VB-Cable not detected**
- Install from https://vb-audio.com/Cable/ and reboot
- Check device name: `--list-devices`
- Use explicit name: `--device "CABLE Input"`

**High latency or clock drift**
- See NOTES.md for PTP synchronization details
- Manual trim: `CLOCK_OFFSET_NS=<value>`
- Verify network stability and Dante device clock sources

**Service won't start**
- Check logs: `%LOCALAPPDATA%\inferno_aoip\logs\inferno.log`
- Re-run with `--install-service` (admin required)
- Verify user account has audio device permissions

**Firewall issues**
- Run firewall script: `.\scripts\open-firewall-admin.ps1`
- Or manually allow UDP 5353 (mDNS), RTP/PTP ranges

## See Also

- [SETUP.md](SETUP.md) — SYSVAD installation and driver setup
- [NOTES.md](NOTES.md) — Known issues, PTP clock sync, performance tuning

