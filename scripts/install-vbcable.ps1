#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Downloads and installs VB-Cable virtual audio device.

.DESCRIPTION
    VB-Cable creates two Windows audio devices:
      - CABLE Input  (playback) -- inferno_wasapi writes Dante audio here
      - CABLE Output (recording) -- your apps read Dante audio from here

    After install, run: inferno_wasapi.exe --virtual-device

    VB-Cable is free donationware by VB-Audio: https://vb-audio.com/Cable/
    This script downloads and installs it unattended.

.NOTES
    Requires admin rights. Reboot required after install.
#>

$ErrorActionPreference = 'Stop'

$downloadUrl = "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip"
$tempDir     = Join-Path $env:TEMP "vbcable_install"
$zipPath     = Join-Path $tempDir "VBCABLE_Driver_Pack45.zip"

Write-Host "=== VB-Cable Virtual Audio Device Installer ===" -ForegroundColor Cyan
Write-Host "Downloading VB-Cable from vb-audio.com..."

if (-not (Test-Path $tempDir)) { New-Item -ItemType Directory -Path $tempDir | Out-Null }

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing

Write-Host "Extracting..."
Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

$setup = Get-ChildItem -Path $tempDir -Filter "VBCABLE_Setup_x64.exe" -Recurse | Select-Object -First 1
if (-not $setup) {
    Write-Error "Could not find VBCABLE_Setup_x64.exe in extracted files."
    exit 1
}

Write-Host "Installing VB-Cable driver (this may take a moment)..."
Start-Process -FilePath $setup.FullName -ArgumentList "/S" -Wait -NoNewWindow

Write-Host ""
Write-Host "=== VB-Cable installed successfully! ===" -ForegroundColor Green
Write-Host ""
Write-Host "Two new audio devices are now available:" -ForegroundColor Yellow
Write-Host "  - CABLE Input  (playback)  <- inferno_wasapi writes Dante audio here"
Write-Host "  - CABLE Output (recording) <- your apps read Dante audio from here"
Write-Host ""
Write-Host "IMPORTANT: A reboot may be required for the devices to appear." -ForegroundColor Magenta
Write-Host ""
Write-Host "After reboot, run:"
Write-Host "  .\target\release\inferno_wasapi.exe --virtual-device" -ForegroundColor Cyan
Write-Host ""
Write-Host "Then in OBS, Audacity, or any app, select 'CABLE Output' as input device."

# Cleanup
Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
