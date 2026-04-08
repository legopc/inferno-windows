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

## Step 13 — Test Signing and Driver Installation

### Overview

To load a kernel driver on Windows with test signing enabled, you need to:
1. **Enable test signing** at the OS level (bcdedit)
2. **Create a test signing certificate**
3. **Build the driver** (inferno_driver.sys)
4. **Sign the driver** with your test certificate
5. **Install the driver** via Device Manager or devcon

This section covers self-signed certificate generation and driver installation on a **test VM only**.

---

### Prerequisites

- **Secure Boot disabled** in BIOS (see Step 1)
- **Test signing enabled** (see Step 2)
- **Visual Studio 2022** with **WDK extension** installed (see Steps 4–5)
- **Windows Driver Kit (WDK)** tools available in PATH or via full path

---

### Step 13a — Create a Self-Signed Test Certificate

Test certificates are stored in the current user's certificate store. We'll use PowerShell's built-in
certificate cmdlets:

```powershell
# Create self-signed certificate for driver signing
$cert = New-SelfSignedCertificate -CertStoreLocation "Cert:\CurrentUser\My" `
    -Subject "CN=InfernoTestCert" `
    -KeyUsage DigitalSignature `
    -Type CodeSigningCert `
    -FriendlyName "Inferno Test Certificate"

Write-Host "Certificate created: $($cert.Thumbprint)"
Write-Host "Subject: $($cert.Subject)"

# Export certificate to PFX (for future re-import if needed)
$pwd = ConvertTo-SecureString -String "TestPassword123" -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath "$Home\Desktop\InfernoTestCert.pfx" -Password $pwd -Force

# Export certificate to CER (for distribution/documentation)
Export-Certificate -Cert $cert -FilePath "$Home\Desktop\InfernoTestCert.cer" -Type CERT -Force
```

The certificate is now in the **CurrentUser → My** store. It will be used by `signtool.exe` to sign the driver.

---

### Step 13b — Build the Driver

#### Known Issue: MSBuild WDK Integration

Direct MSBuild builds fail with:
```
error MSB4062: The "ValidateNTTargetVersion" task could not be loaded from the assembly
C:\Program Files (x86)\Windows Kits\10\build\10.0.26100.0\bin\Microsoft.DriverKit.Build.Tasks.18.0.dll
```

This is a known WDK integration issue on some systems. **Workaround: Use Visual Studio IDE instead.**

#### Build with Visual Studio IDE

1. Open **Visual Studio 2022**
2. **File → Open → Project/Solution**
   - Navigate to: `C:\Users\copilot\source\repos\inferno-windows\inferno_driver\inferno_driver.vcxproj`
3. Select configuration and platform:
   - **Configuration:** Release (or Debug)
   - **Platform:** x64
4. **Build → Build Solution** (Ctrl+Shift+B)
5. Wait for the build to complete
6. Output binary location:
   ```
   inferno_driver\x64\Release\inferno_driver.sys
   ```

---

### Step 13c — Sign the Driver

Once you have `inferno_driver.sys`, sign it with your test certificate:

```powershell
# Path to signtool (from WDK)
$wdk = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64"
$signtool = "$wdk\signtool.exe"

# Sign the driver with the test certificate
& $signtool sign /fd SHA256 /a /s "My" /n "InfernoTestCert" `
    "C:\Users\copilot\source\repos\inferno-windows\inferno_driver\x64\Release\inferno_driver.sys"
```

**Parameters:**
- `/fd SHA256` — Use SHA-256 hash algorithm
- `/a` — Auto-select the best certificate from the store
- `/s "My"` — Look in the CurrentUser\My certificate store
- `/n "InfernoTestCert"` — Certificate name to match

On success, you will see:
```
SignTool verify: Number of files successfully signed: 1
```

---

### Step 13d — Install the Driver via Device Manager

Test-signed drivers can be installed using the **Add Legacy Hardware Wizard**:

1. Open **Device Manager** (devmgmt.msc)
2. **Action → Add legacy hardware**
3. Select **"Install the hardware that I manually select from a list"**
4. Choose a device category (e.g., **Sound, video and game controllers**)
5. Click **"Have Disk…"** and browse to:
   - `C:\Users\copilot\source\repos\inferno-windows\inferno_driver\`
6. Select the `.inf` file (e.g., `ComponentizedAudioSample.inx` → compiled to `.inf`)
7. Follow the prompts to install
8. **Reboot** when prompted

Alternatively, use **devcon.exe** from the command line (requires admin):
```powershell
$devcon = "C:\Program Files (x86)\Windows Kits\10\Tools\x64\devcon.exe"
& $devcon install "inferno_driver.inf" "*"
```

---

### Step 13e — Verify the Driver is Loaded

After installation and reboot, verify the driver appears:

```powershell
# List all PnP devices with "Inferno" in the name
Get-PnpDevice | Where-Object { $_.FriendlyName -like "*Inferno*" } | Select-Object FriendlyName, Status

# Or check all sound/audio devices
Get-PnpDevice -Class "Sound" | Select-Object FriendlyName, Status
```

Expected output (if successful):
```
FriendlyName              Status
----------------          ------
Inferno Virtual Audio D… OK
```

If the driver is not listed, check **Device Manager** for errors (yellow exclamation mark) or check the Event Viewer for kernel errors.

---

### Important: Binary Exclusions

Driver binaries (`.sys`, `.cat`, `.pdb`) are excluded from version control. Check `.gitignore`:

```
# Driver binaries and signing artifacts
*.sys
*.cat
*.pdb
inferno_driver/*.sys
inferno_driver/*.cat
inferno_driver/*.pdb
```

Do **NOT** commit compiled drivers. Rebuild on each target system.

---

### Production Signing

Test signing is **only for development and testing on your own VM**. For production drivers,
you must:
- Obtain an **Extended Validation (EV) Code Signing Certificate** from a Certificate Authority
- Sign drivers with that certificate
- Submit drivers to **Microsoft for WHQL certification** (for kernel drivers)

See the **production signing documentation** (if available) for full details.

---

## Quick Reference — Test Signing Commands

| Task | Command |
|---|---|
| Enable test signing (admin) | `bcdedit /set testsigning on` |
| Reboot to apply | `Shutdown /r /t 0` |
| Create test certificate | `$cert = New-SelfSignedCertificate -CertStoreLocation "Cert:\CurrentUser\My" -Subject "CN=InfernoTestCert" -KeyUsage DigitalSignature -Type CodeSigningCert` |
| Build in VS (IDE) | Open VS → inferno_driver.vcxproj → Build Solution |
| Sign driver (admin) | `signtool sign /fd SHA256 /a /s "My" /n "InfernoTestCert" inferno_driver.sys` |
| Install via Device Manager | Action → Add legacy hardware → Have Disk → select .inf |
| Verify driver loaded | `Get-PnpDevice \| Where-Object {$_.FriendlyName -like "*Inferno*"}` |
| Disable test signing | `bcdedit /set testsigning off` (then reboot) |

---

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
