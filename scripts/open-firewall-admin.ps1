#Requires -RunAsAdministrator
# Open Windows Firewall ports required for Dante AoIP (inferno_wasapi)
# Run this script once from an elevated PowerShell window.

$ports = @(
    @{ port = 4455; desc = "Dante audio RX flows" },
    @{ port = 8700; desc = "Dante audio TX flows" },
    @{ port = 4400; desc = "Dante ARC (routing/control)" },
    @{ port = 8800; desc = "Dante CMC" },
    @{ port = 5353; desc = "mDNS device discovery" }
)

foreach ($entry in $ports) {
    $name = "Inferno-Dante UDP $($entry.port)"
    Write-Host "Adding rule: $name ($($entry.desc))"
    netsh advfirewall firewall add rule `
        name="$name" `
        protocol=UDP `
        dir=in `
        localport=$($entry.port) `
        action=allow | Out-Null
}

Write-Host ""
Write-Host "Firewall rules added. Verify with:"
Write-Host '  netsh advfirewall firewall show rule name="Inferno-Dante*"'
