#Requires -RunAsAdministrator
# Open Windows Firewall ports required for Dante AoIP (inferno_wasapi)
# Run this script once from an elevated PowerShell window.

# Correct Dante protocol ports (from inferno_aoip source):
#   4440 = ARC  (proto_arc.rs)
#   4455 = flows control  (flows_control.rs)
#   8700 = info/mcast requests  (mcast.rs)
#   8800 = CMC  (proto_cmc.rs)
#   5353 = mDNS device discovery

$ports = @(
    @{ port = 4440; desc = "Dante ARC (routing/control)" },
    @{ port = 4455; desc = "Dante flows control" },
    @{ port = 8700; desc = "Dante info/mcast requests" },
    @{ port = 8800; desc = "Dante CMC" },
    @{ port = 5353; desc = "mDNS device discovery" }
)

# Remove any old/incorrect rules first
netsh advfirewall firewall delete rule name="Inferno-Dante UDP 4400" 2>$null | Out-Null

foreach ($entry in $ports) {
    $name = "Inferno-Dante UDP $($entry.port)"
    Write-Host "Adding rule: $name ($($entry.desc))"
    netsh advfirewall firewall delete rule name="$name" 2>$null | Out-Null
    netsh advfirewall firewall add rule `
        name="$name" `
        protocol=UDP `
        dir=in `
        localport=$($entry.port) `
        action=allow | Out-Null
}

# Also allow the executable by name (Windows may block it regardless of port rules)
$exePath = Join-Path $PSScriptRoot "..\target\release\inferno_wasapi.exe"
$resolved = Resolve-Path $exePath -ErrorAction SilentlyContinue
if ($resolved) {
    $exePath = $resolved.Path
    Write-Host "Adding executable rule for: $exePath"
    netsh advfirewall firewall delete rule name="Inferno-WASAPI" 2>$null | Out-Null
    netsh advfirewall firewall add rule `
        name="Inferno-WASAPI" `
        dir=in `
        action=allow `
        program="$exePath" `
        protocol=UDP | Out-Null
} else {
    Write-Warning "Could not find inferno_wasapi.exe -- run 'cargo build --release -p inferno_wasapi' first, then re-run this script."
}

Write-Host ""
Write-Host "Firewall rules added. Verify with:"
Write-Host '  netsh advfirewall firewall show rule name="Inferno-Dante*"'
Write-Host '  netsh advfirewall firewall show rule name="Inferno-WASAPI"'
