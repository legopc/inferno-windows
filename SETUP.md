# Inferno Windows — Developer Machine Setup

This guide sets up a Windows 11 VM for building and testing the full stack:
SYSVAD virtual audio driver (C/C++, WDK) + Rust Dante TX pipeline + GitHub Copilot cloud agent
via a self-hosted Actions runner.

---

## VM Specification

| | Minimum | Recommended |
|---|---|---|
| **OS** | Windows 11 Pro x64 | Windows 11 Pro x64 |
| **RAM** | 8 GB | 16 GB |
| **Disk** | 80 GB | 120 GB |
| **CPU** | 4-core x64 | 8-core x64 |
| **Secure Boot** | **DISABLED** (required for test signing) | Disabled |
| **Network** | Same subnet as Dante devices | Same subnet |

> **Do not use Windows Server.** The WDM audio driver stack and WASAPI are primarily
> supported on Windows Desktop. Server editions require the "Desktop Experience" optional
> feature and are not tested for audio driver development.

---

## Step 1 — Disable Secure Boot

Test-signed kernel drivers cannot load while Secure Boot is active.

1. Shut down the VM
2. Enter firmware settings (VM hypervisor setting, or F2/DEL at POST)
3. Disable Secure Boot
4. Boot Windows

---

## Step 2 — Enable Test Signing

Run from an **elevated PowerShell** (Run as Administrator):

```powershell
.\scripts\enable-testsigning.ps1
```

Or manually:
```powershell
bcdedit /set testsigning on
```

Reboot after running this command. A "Test Mode" watermark will appear in the bottom-right
corner of the desktop — this is expected and confirms test signing is active.

---

## Step 3 — Install Base Tools (Automated)

Run from an **elevated PowerShell**:

```powershell
.\scripts\setup-dev-machine.ps1
```

This installs via winget: Git, GitHub CLI (`gh`), Rust (`rustup`), and VS2022 Build Tools
with the Desktop C++ workload.

---

## Step 4 — Install Visual Studio 2022 (Manual)

VS2022 Build Tools installed by the script above is sufficient for Rust. However, building
the SYSVAD kernel driver requires the **full VS2022 IDE** with the WDK extension.

If you want the full IDE:
1. Download VS2022 Community (free) or Professional from https://visualstudio.microsoft.com/
2. In the installer, select **"Desktop development with C++"** workload
3. Complete installation (~5-10 GB)

---

## Step 5 — Install Windows Driver Kit (WDK) (Manual)

The WDK is required to build kernel audio drivers.

1. Open this URL: https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk
2. Follow the instructions to install **WDK for Windows 11, version 22H2** (or latest)
   - This installs as a Visual Studio 2022 extension
   - Requires VS2022 to be installed first
3. After installation, open VS2022 and verify the WDK templates appear under:
   **File → New Project → Windows Driver (WDK)**

---

## Step 6 — Clone Repositories

```powershell
# Clone this repo (if not already present)
git clone https://github.com/<your-org>/inferno_windows.git
cd inferno_windows

# Clone Microsoft Windows Driver Samples (for SYSVAD)
# Place alongside this repo or anywhere — the install script will prompt for the path
git clone https://github.com/microsoft/Windows-driver-samples.git ..\windows-driver-samples
```

---

## Step 7 — Building the Kernel Driver

### WDK Build Environment Status

The SYSVAD TabletAudioSample source has been forked into `inferno_driver/` directory. 

**Build Tool Findings (WDK 10.0.26100.0):**
- ✅ WDK installation verified at: `C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\`
- ✅ MSBuild path confirmed: `C:\Program Files\Microsoft Visual Studio\18\Community\MSBuild\Current\Bin\MSBuild.exe`
- ❌ **Build blocker:** Missing WDK build task assembly
  - Error: `Microsoft.DriverKit.Build.Tasks.18.0.dll` not found at `C:\Program Files (x86)\Windows Kits\10\build\10.0.26100.0\bin\`
  - This is a known WDK installation issue on some systems
  - **Workaround:** Build from within Visual Studio IDE with WDK extension, or repair/reinstall WDK

### Building with Visual Studio IDE

1. Open Visual Studio 2022
2. Open `inferno_driver\inferno_driver.vcxproj`
3. Set configuration to **Release** and platform to **x64**
4. **Build → Build Solution**
5. Output binary: `inferno_driver\x64\Release\inferno_driver.sys`

### Source Details

The forked driver is based on Microsoft's SYSVAD TabletAudioSample:
- **Source:** `https://github.com/microsoft/Windows-driver-samples` (audio/sysvad/TabletAudioSample)
- **Files:** 48 source files (C++, headers, INF, project files)
- **Driver name:** `inferno_driver.sys` (renamed from `tabletaudiosample.sys`)
- **Device name:** "Inferno Virtual Audio Device" (updated in INF files)

---

## Step 8 — Install SYSVAD Virtual Audio Driver

Run from an **elevated PowerShell**:

```powershell
.\scripts\install-sysvad.ps1
```

The script will:
1. Prompt for the path to the cloned `windows-driver-samples` repo
2. Build `audio\sysvad\TabletAudioSample` (x64 Release) using MSBuild
3. Install the driver using `devcon.exe`
4. Verify the virtual audio device appears

After installation, open **Sound settings → Playback devices** and confirm a SYSVAD device
is listed (name varies; look for "Tablet" or "SYSVAD"). **Set it as the default playback
device** so Windows routes system audio through it.

---

## Step 9 — Register Self-Hosted GitHub Actions Runner

This allows the **GitHub Copilot cloud agent** to run directly on this VM, with access to
the WDK, SYSVAD driver, Rust toolchain, and local audio devices.

1. On GitHub, open the repository → **Settings → Actions → Runners → New self-hosted runner**
2. Select **Windows**, **x64**
3. Follow the shown commands to download and configure the runner
4. Register with labels `self-hosted,windows,x64`

Or use the guided script:
```powershell
.\scripts\setup-github-runner.ps1
```

**Recommended:** Install the runner as a Windows Service so it starts automatically:
```powershell
.\run.cmd --service
```

---

## Step 10 — Disable Copilot Integrated Firewall

The Copilot cloud agent's built-in firewall is **incompatible with self-hosted runners** and
must be disabled for the runner to work.

1. Open the repository on GitHub
2. **Settings → Copilot → Agent**
3. Under **Firewall**, select **Disabled**
4. Save

---

## Step 11 — Open Dante Firewall Ports

Run from an **elevated PowerShell**:

```powershell
.\scripts\open-firewall-admin.ps1
```

Required UDP ports: 4440, 4455, 5353, 8700, 8800

---

## Step 12 — Build the Rust Code

```powershell
cargo build --release
```

Test the RX binary:
```powershell
.\target\release\inferno_wasapi.exe --list-devices
```

---

## Quick Reference — Common Commands

| Task | Command |
|---|---|
| List Dante RX audio devices | `.\target\release\inferno_wasapi.exe --list-devices` |
| Run as Dante receiver (RX) | `.\target\release\inferno_wasapi.exe` |
| List TX loopback sources | `.\target\release\inferno_wasapi.exe --list-tx-devices` |
| Run as Dante transmitter (TX) | `.\target\release\inferno_wasapi.exe --tx` |
| TX from named device | `.\target\release\inferno_wasapi.exe --tx --tx-device "SYSVAD"` |
| Open firewall ports (admin) | `.\scripts\open-firewall-admin.ps1` |
| Build release | `cargo build --release` |

---

## Troubleshooting

**"Test Mode" watermark not showing after bcdedit**
→ Secure Boot may still be active. Confirm it is disabled in firmware settings.

**SYSVAD device not appearing in Sound settings**
→ Check Device Manager for driver errors. Re-run `.\scripts\install-sysvad.ps1` as admin.

**Copilot cloud agent not running on self-hosted runner**
→ Confirm the runner is online (shows green in Settings → Actions → Runners).
→ Confirm the Copilot integrated firewall is disabled (Settings → Copilot → Agent → Firewall).

**`devcon.exe` not found**
→ devcon.exe is in the WDK samples. Path: `C:\Program Files (x86)\Windows Kits\10\Tools\x64\devcon.exe`
→ Or download it separately from the WDK tools.

**Dante device not appearing in Dante Controller**
→ Check firewall ports are open (`.\scripts\open-firewall-admin.ps1`).
→ Confirm the machine is on the same subnet as the Dante network.
→ See NOTES.md for full port list.
