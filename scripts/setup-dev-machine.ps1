#Requires -RunAsAdministrator
# Automated setup for the inferno_windows development machine.
# Installs Git, GitHub CLI, Rust (rustup), and VS2022 Build Tools via winget.
#
# Run from an elevated PowerShell window:
#   .\scripts\setup-dev-machine.ps1

$ErrorActionPreference = "Stop"

function Install-WingetPackage {
    param([string]$Id, [string]$Name)
    Write-Host "  Installing $Name ($Id)..." -ForegroundColor Cyan
    winget install --id $Id --silent --accept-source-agreements --accept-package-agreements
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
        # -1978335189 = APPINSTALLER_ERROR_ALREADY_INSTALLED (not an error)
        Write-Warning "  winget returned $LASTEXITCODE for $Name — check manually"
    } else {
        Write-Host "  $Name OK" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "=== inferno_windows dev machine setup ===" -ForegroundColor White
Write-Host ""

# ---- Check winget ----
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Error "winget is not available. Install the App Installer from the Microsoft Store and re-run."
}

# ---- Install packages ----
Write-Host "Installing base tools via winget..." -ForegroundColor White

Install-WingetPackage "Git.Git"            "Git"
Install-WingetPackage "GitHub.cli"         "GitHub CLI (gh)"
Install-WingetPackage "Rustlang.Rustup"    "Rust (rustup)"

# VS2022 Build Tools with Desktop C++ workload (sufficient for Rust; SYSVAD needs full IDE)
Write-Host "  Installing VS2022 Build Tools with Desktop C++ workload..." -ForegroundColor Cyan
winget install --id Microsoft.VisualStudio.2022.BuildTools `
    --silent --accept-source-agreements --accept-package-agreements `
    --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
if ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq -1978335189) {
    Write-Host "  VS2022 Build Tools OK" -ForegroundColor Green
} else {
    Write-Warning "  VS2022 Build Tools returned $LASTEXITCODE — check manually"
}

# ---- Rust toolchain ----
Write-Host ""
Write-Host "Configuring Rust toolchain..." -ForegroundColor White
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    rustup toolchain install stable-x86_64-pc-windows-msvc --no-self-update
    rustup default stable-x86_64-pc-windows-msvc
    Write-Host "  Rust stable (MSVC) OK" -ForegroundColor Green
} else {
    Write-Warning "  rustup not found in PATH — open a new shell and run: rustup toolchain install stable-x86_64-pc-windows-msvc"
}

# ---- Manual steps reminder ----
Write-Host ""
Write-Host "=== Manual steps still required ===" -ForegroundColor Yellow
Write-Host ""
Write-Host "  1. Disable Secure Boot in VM firmware settings (required for test signing)" -ForegroundColor Yellow
Write-Host "  2. Enable test signing (run as admin):" -ForegroundColor Yellow
Write-Host "       .\scripts\enable-testsigning.ps1" -ForegroundColor Cyan
Write-Host "     Then REBOOT." -ForegroundColor Yellow
Write-Host ""
Write-Host "  3. Install Windows Driver Kit (WDK) for VS2022:" -ForegroundColor Yellow
Write-Host "       https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk" -ForegroundColor Cyan
Write-Host "     (Requires VS2022 full IDE — Build Tools is not sufficient for the WDK extension)" -ForegroundColor Yellow
Write-Host ""
Write-Host "  4. Install VS2022 full IDE if not already done (for WDK extension):" -ForegroundColor Yellow
Write-Host "       https://visualstudio.microsoft.com/downloads/" -ForegroundColor Cyan
Write-Host "     Workload: 'Desktop development with C++'" -ForegroundColor Yellow
Write-Host ""
Write-Host "  5. Clone windows-driver-samples alongside this repo:" -ForegroundColor Yellow
Write-Host "       git clone https://github.com/microsoft/Windows-driver-samples.git ..\windows-driver-samples" -ForegroundColor Cyan
Write-Host ""
Write-Host "  6. Register self-hosted GitHub Actions runner:" -ForegroundColor Yellow
Write-Host "       .\scripts\setup-github-runner.ps1" -ForegroundColor Cyan
Write-Host ""
Write-Host "  7. Disable Copilot integrated firewall in repo Settings → Copilot → Agent → Firewall" -ForegroundColor Yellow
Write-Host ""
Write-Host "  8. Build and install SYSVAD driver (run as admin, after WDK install):" -ForegroundColor Yellow
Write-Host "       .\scripts\install-sysvad.ps1" -ForegroundColor Cyan
Write-Host ""
Write-Host "  9. Open Dante firewall ports (run as admin):" -ForegroundColor Yellow
Write-Host "       .\scripts\open-firewall-admin.ps1" -ForegroundColor Cyan
Write-Host ""
Write-Host "See SETUP.md for full details on each step." -ForegroundColor White
Write-Host ""
