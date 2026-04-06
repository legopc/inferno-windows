#Requires -RunAsAdministrator
# Enable Windows test signing mode for loading self-signed kernel drivers (e.g. SYSVAD).
#
# IMPORTANT: Secure Boot must also be DISABLED in firmware settings for this to take effect.
#
# Run from an elevated PowerShell window:
#   .\scripts\enable-testsigning.ps1

Write-Host ""
Write-Host "=== Enabling Windows Test Signing Mode ===" -ForegroundColor White
Write-Host ""

bcdedit /set testsigning on

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "Test signing enabled successfully." -ForegroundColor Green
    Write-Host ""
    Write-Host "REQUIRED NEXT STEPS:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  1. Disable Secure Boot in your VM firmware / BIOS settings." -ForegroundColor Yellow
    Write-Host "     Test-signed drivers will NOT load if Secure Boot is active," -ForegroundColor Yellow
    Write-Host "     even with test signing enabled." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  2. REBOOT the machine." -ForegroundColor Yellow
    Write-Host "     After reboot, a 'Test Mode' watermark will appear in the bottom-right" -ForegroundColor Yellow
    Write-Host "     corner of the desktop. This confirms test signing is active." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "To verify after reboot:" -ForegroundColor White
    Write-Host "  bcdedit | Select-String testsigning" -ForegroundColor Cyan
    Write-Host "  (should show: testsigning     Yes)" -ForegroundColor White
    Write-Host ""
    Write-Host "To revert (production machines only):" -ForegroundColor White
    Write-Host "  bcdedit /set testsigning off" -ForegroundColor Cyan
    Write-Host ""
} else {
    Write-Error "bcdedit failed (exit code $LASTEXITCODE). Ensure you are running as Administrator."
}
