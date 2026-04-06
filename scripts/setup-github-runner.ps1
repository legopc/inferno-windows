# Set up a GitHub Actions self-hosted runner on this Windows machine.
#
# This allows the GitHub Copilot cloud agent to run directly on this VM,
# building Rust code, installing drivers, and testing audio — all natively.
#
# Prerequisites:
#   - GitHub account with access to the inferno_windows repository
#   - Repository must be on GitHub
#   - GitHub Copilot Business or Enterprise subscription
#
# Run from a regular (non-admin) PowerShell window:
#   .\scripts\setup-github-runner.ps1

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=== GitHub Actions Self-Hosted Runner Setup ===" -ForegroundColor White
Write-Host ""

# ---- Get repo info ----
$repoUrl = Read-Host "GitHub repository URL (e.g. https://github.com/your-org/inferno_windows)"
if ($repoUrl -notmatch "github\.com/(.+)/(.+?)(?:\.git)?$") {
    Write-Error "Invalid GitHub URL. Expected format: https://github.com/owner/repo"
}
$owner = $Matches[1]
$repo  = $Matches[2]

Write-Host ""
Write-Host "Repository: $owner/$repo" -ForegroundColor Cyan
Write-Host ""

# ---- Instructions ----
Write-Host "Step 1: Generate a runner registration token" -ForegroundColor Yellow
Write-Host "  Open this URL in your browser:" -ForegroundColor White
Write-Host "  https://github.com/$owner/$repo/settings/actions/runners/new?runnerOs=windows" -ForegroundColor Cyan
Write-Host ""
Write-Host "  GitHub will show a token and download/config commands. Come back here after" -ForegroundColor White
Write-Host "  copying the token shown on that page." -ForegroundColor White
Write-Host ""

$token = Read-Host "Paste the runner registration token from GitHub"
if ([string]::IsNullOrWhiteSpace($token)) {
    Write-Error "Token is required."
}

# ---- Download runner ----
$runnerDir = Join-Path $env:USERPROFILE "actions-runner"
if (-not (Test-Path $runnerDir)) {
    New-Item -ItemType Directory -Path $runnerDir | Out-Null
}
Push-Location $runnerDir

Write-Host ""
Write-Host "Step 2: Downloading GitHub Actions runner..." -ForegroundColor Yellow

# Get the latest runner version from GitHub API
try {
    $apiUrl = "https://api.github.com/repos/actions/runner/releases/latest"
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "inferno-setup" }
    $asset = $release.assets | Where-Object { $_.name -like "actions-runner-win-x64-*.zip" } | Select-Object -First 1
    $runnerVersion = $release.tag_name -replace "^v", ""
    $downloadUrl = $asset.browser_download_url
} catch {
    $runnerVersion = "2.321.0"
    $downloadUrl = "https://github.com/actions/runner/releases/download/v$runnerVersion/actions-runner-win-x64-$runnerVersion.zip"
    Write-Warning "Could not fetch latest version from GitHub API; using $runnerVersion"
}

$zipPath = Join-Path $runnerDir "actions-runner-win-x64-$runnerVersion.zip"
if (-not (Test-Path $zipPath)) {
    Write-Host "  Downloading from: $downloadUrl" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath
} else {
    Write-Host "  Runner zip already present: $zipPath" -ForegroundColor Green
}

Write-Host "  Extracting..." -ForegroundColor Cyan
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::ExtractToDirectory($zipPath, $runnerDir)
Write-Host "  Extracted to: $runnerDir" -ForegroundColor Green

# ---- Configure runner ----
Write-Host ""
Write-Host "Step 3: Configuring runner..." -ForegroundColor Yellow

$configCmd = Join-Path $runnerDir "config.cmd"
& $configCmd `
    --url "https://github.com/$owner/$repo" `
    --token $token `
    --labels "self-hosted,windows,x64" `
    --name "inferno-windows-vm" `
    --work "_work" `
    --unattended

if ($LASTEXITCODE -ne 0) {
    Write-Error "Runner configuration failed (exit code $LASTEXITCODE)."
}
Write-Host "Runner configured." -ForegroundColor Green

# ---- Install as service ----
Write-Host ""
$installService = Read-Host "Install runner as a Windows Service (auto-starts on boot)? [Y/n]"
if ($installService -ne "n" -and $installService -ne "N") {
    Write-Host "Installing runner as Windows Service..." -ForegroundColor Yellow
    $svcCmd = Join-Path $runnerDir "svc.cmd"
    & $svcCmd install
    & $svcCmd start
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Runner service installed and started." -ForegroundColor Green
    } else {
        Write-Warning "Service install returned $LASTEXITCODE. You can start the runner manually with: $runnerDir\run.cmd"
    }
} else {
    Write-Host ""
    Write-Host "To start the runner manually:" -ForegroundColor White
    Write-Host "  $runnerDir\run.cmd" -ForegroundColor Cyan
}

Pop-Location

# ---- Final instructions ----
Write-Host ""
Write-Host "=== Runner setup complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "IMPORTANT: Before using Copilot cloud agent on this runner," -ForegroundColor Yellow
Write-Host "disable the Copilot integrated firewall in your repository settings:" -ForegroundColor Yellow
Write-Host "  https://github.com/$owner/$repo/settings/copilot/agent" -ForegroundColor Cyan
Write-Host "  Settings → Copilot → Agent → Firewall → Disabled" -ForegroundColor White
Write-Host ""
Write-Host "Verify the runner is online:" -ForegroundColor White
Write-Host "  https://github.com/$owner/$repo/settings/actions/runners" -ForegroundColor Cyan
Write-Host "  (should show 'Idle' status)" -ForegroundColor White
Write-Host ""
