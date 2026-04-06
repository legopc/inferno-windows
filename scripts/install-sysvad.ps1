#Requires -RunAsAdministrator
# Build and install the SYSVAD TabletAudioSample virtual audio driver.
#
# Prerequisites:
#   - Visual Studio 2022 with "Desktop development with C++" workload
#   - Windows Driver Kit (WDK) for VS2022 installed
#   - Test signing enabled (run enable-testsigning.ps1 and reboot)
#   - Secure Boot disabled
#
# Run from an elevated PowerShell window:
#   .\scripts\install-sysvad.ps1

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=== SYSVAD TabletAudioSample Driver Build + Install ===" -ForegroundColor White
Write-Host ""

# ---- Locate windows-driver-samples ----
$defaultSamplesPath = Join-Path (Split-Path $PSScriptRoot -Parent) "..\windows-driver-samples"
$defaultSamplesPath = [System.IO.Path]::GetFullPath($defaultSamplesPath)

$samplesPath = Read-Host "Path to cloned windows-driver-samples repo [$defaultSamplesPath]"
if ([string]::IsNullOrWhiteSpace($samplesPath)) {
    $samplesPath = $defaultSamplesPath
}

$slnPath = Join-Path $samplesPath "audio\sysvad\TabletAudioSample\TabletAudioSample.sln"
if (-not (Test-Path $slnPath)) {
    Write-Error "Solution not found: $slnPath`nClone the repo: git clone https://github.com/microsoft/Windows-driver-samples.git"
}

Write-Host "Solution: $slnPath" -ForegroundColor Cyan

# ---- Locate MSBuild ----
$msbuildPaths = @(
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional\MSBuild\Current\Bin\MSBuild.exe",
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Enterprise\MSBuild\Current\Bin\MSBuild.exe",
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe"
)
$msbuild = $msbuildPaths | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $msbuild) {
    Write-Error "MSBuild.exe not found. Ensure VS2022 is installed with the 'Desktop development with C++' workload."
}
Write-Host "MSBuild: $msbuild" -ForegroundColor Cyan

# ---- Build ----
Write-Host ""
Write-Host "Building TabletAudioSample (x64 Release)..." -ForegroundColor White
& $msbuild $slnPath /p:Configuration=Release /p:Platform=x64 /m /nologo /clp:Summary
if ($LASTEXITCODE -ne 0) {
    Write-Error "MSBuild failed (exit code $LASTEXITCODE). Check the output above for errors."
}
Write-Host "Build succeeded." -ForegroundColor Green

# ---- Locate build output ----
$outDir = Join-Path $samplesPath "audio\sysvad\TabletAudioSample\x64\Release\TabletAudioSample"
$infFile = Join-Path $outDir "tabletaudiosample.inf"
if (-not (Test-Path $infFile)) {
    # Try alternate output layout
    $outDir = Join-Path $samplesPath "audio\sysvad\TabletAudioSample\x64\Release"
    $infFile = Get-ChildItem -Path $outDir -Filter "*.inf" -Recurse | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $infFile -or -not (Test-Path $infFile)) {
    Write-Error "Could not find .inf file in build output under $samplesPath\audio\sysvad\TabletAudioSample\x64\Release"
}
Write-Host "INF file: $infFile" -ForegroundColor Cyan

# ---- Locate devcon.exe ----
$devconPaths = @(
    "C:\Program Files (x86)\Windows Kits\10\Tools\x64\devcon.exe",
    "C:\Program Files (x86)\Windows Kits\10\Tools\10.0.22621.0\x64\devcon.exe",
    (Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Tools\" -Filter "devcon.exe" -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*x64*" } | Select-Object -First 1 -ExpandProperty FullName)
)
$devcon = $devconPaths | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $devcon) {
    Write-Warning "devcon.exe not found in standard WDK paths."
    Write-Warning "Download it from: https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/devcon"
    $devcon = Read-Host "Enter full path to devcon.exe (or press Enter to skip install)"
}

# ---- Install driver ----
if ($devcon -and (Test-Path $devcon)) {
    Write-Host ""
    Write-Host "Installing driver..." -ForegroundColor White
    & $devcon install $infFile "Root\Sysvad_TabletAudioSample"
    if ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq 1) {
        # devcon returns 1 when a reboot is required (not an error)
        Write-Host "Driver installed." -ForegroundColor Green
        if ($LASTEXITCODE -eq 1) {
            Write-Host "A reboot may be required for the device to activate." -ForegroundColor Yellow
        }
    } else {
        Write-Warning "devcon returned $LASTEXITCODE. Check Device Manager for errors."
    }
} else {
    Write-Host "Skipping driver install (devcon not found)." -ForegroundColor Yellow
    Write-Host "Install manually:" -ForegroundColor Yellow
    Write-Host "  devcon.exe install `"$infFile`" Root\Sysvad_TabletAudioSample" -ForegroundColor Cyan
}

# ---- Verify ----
Write-Host ""
Write-Host "Verifying installed audio devices..." -ForegroundColor White
$devices = Get-PnpDevice -Class "MEDIA" -ErrorAction SilentlyContinue | Where-Object { $_.FriendlyName -match "Tablet|SYSVAD|Virtual" }
if ($devices) {
    Write-Host "Found virtual audio device(s):" -ForegroundColor Green
    $devices | Format-Table FriendlyName, Status, InstanceId -AutoSize
} else {
    Write-Host "No SYSVAD/Tablet device found yet. Check Device Manager or reboot and re-check." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Next: Open Sound settings (Win+I → Sound) and confirm the SYSVAD device" -ForegroundColor White
Write-Host "appears under Playback. Set it as the default playback device." -ForegroundColor White
Write-Host ""
