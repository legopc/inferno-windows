# Scripts

Utility PowerShell scripts for setup, testing, and maintenance of Inferno-Windows.

## Script Index

### `setup-dev-machine.ps1`
Configures a development machine with prerequisites for building Inferno-Windows.

**Usage:**
```powershell
.\scripts\setup-dev-machine.ps1
```

### `setup-github-runner.ps1`
Configures a Windows GitHub Actions self-hosted runner for CI/CD.

**Usage:**
```powershell
.\scripts\setup-github-runner.ps1
```

### `install-sysvad.ps1`
Installs the SYSVAD (System Virtual Audio Device) driver for TX mode testing without physical Dante hardware.

**Usage:**
```powershell
.\scripts\install-sysvad.ps1
```

**Note:** After installation, SYSVAD will appear as a virtual audio device in Windows sound settings.

### `install-vbcable.ps1`
Installs VB-Cable (virtual audio loopback cable) as an alternative to SYSVAD.

**Usage:**
```powershell
.\scripts\install-vbcable.ps1
```

### `enable-testsigning.ps1`
Enables Windows test signing mode for unsigned driver development. Requires reboot.

**Usage:**
```powershell
.\scripts\enable-testsigning.ps1
```

**Warning:** Requires administrator elevation and system reboot. Only use in development environments.

### `open-firewall-admin.ps1`
Opens Windows Firewall administrative console for network debugging.

**Usage:**
```powershell
.\scripts\open-firewall-admin.ps1
```

### `soak_test.ps1`
Overnight stability soak test harness for `inferno_wasapi.exe`.

**Purpose:** Validates long-term stability by polling IPC GetStatus every 30 seconds, monitoring for:
- Service crashes
- IPC communication failures (>2 consecutive timeouts)
- Audio dropouts (`rx_active` transitions from true to false)

**Usage:**
```powershell
# Run 8-hour soak test (default)
.\scripts\soak_test.ps1

# Run 24-hour soak test with custom poll interval
.\scripts\soak_test.ps1 -DurationHours 24 -PollIntervalSec 60

# Custom executable path
.\scripts\soak_test.ps1 -ExePath "C:\release\inferno_wasapi.exe"
```

**Parameters:**
- `-DurationHours` (int, default: 8) — Total test duration in hours
- `-ExePath` (string, default: `.\target\release\inferno_wasapi.exe`) — Path to `inferno_wasapi.exe`
- `-PipeName` (string, default: `\\.\pipe\inferno`) — IPC named pipe name
- `-PollIntervalSec` (int, default: 30) — Seconds between status polls

**Output:**
- `soak_results_<timestamp>.csv` — Time-series data (timestamp, uptime, channel counts, clock mode, peak dB, IPC status)
- `alerts_<timestamp>.log` — Critical events (crashes, IPC failures, audio dropouts)
- Console output — Real-time progress and summary

**Exit codes:**
- `0` — Test passed (no errors)
- `1` — Test completed with warnings (check alerts log)

**Example CSV output:**
```
Timestamp,UptimeSecs,RxActive,TxActive,RxChannels,TxChannels,ClockMode,PeakDbMax,IpcOk
2026-04-08T10:00:00,3600,True,False,2,0,SafeClock,-18.5,True
2026-04-08T10:00:30,3630,True,False,2,0,SafeClock,-17.2,True
```

---

## Running Scripts

All scripts require PowerShell 5.1+. To run:

```powershell
cd inferno-windows
.\scripts\<script-name>.ps1
```

Most setup scripts require **administrator elevation**. Right-click PowerShell and select "Run as Administrator".

## Development

When adding new scripts:

1. Include a `.SYNOPSIS` and `.DESCRIPTION` comment block at the top
2. Document all parameters with `.PARAMETER` comments
3. Include usage examples with `.EXAMPLE` comments
4. Update this README.md with a new section
5. Use `$ErrorActionPreference = "Stop"` to fail fast on errors

Example template:

```powershell
<#
.SYNOPSIS
    Short one-line description

.DESCRIPTION
    Longer description of what the script does and why.

.PARAMETER ParamName
    Description of parameter

.EXAMPLE
    .\script.ps1 -ParamName value
    Brief description of what this does
#>

param(
    [string]$ParamName = "default"
)

$ErrorActionPreference = "Stop"

# ... implementation
```
