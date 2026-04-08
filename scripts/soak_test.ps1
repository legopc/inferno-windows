#Requires -Version 5.1
<#
.SYNOPSIS
    Overnight stability soak test harness for inferno_wasapi.exe

.DESCRIPTION
    Starts inferno_wasapi in the background and monitors its stability by:
    - Polling IPC GetStatus every 30 seconds (configurable)
    - Logging results to CSV with timestamp, uptime, channel counts, clock mode, peak dB
    - Alerting on service crashes, IPC failures, or audio dropouts (rx_active false)
    - Running for a configurable duration (default 8 hours)

.PARAMETER DurationHours
    Total soak test duration in hours (default: 8)

.PARAMETER ExePath
    Path to inferno_wasapi.exe (default: .\target\release\inferno_wasapi.exe)

.PARAMETER PipeName
    Named pipe name for IPC (default: \\.\pipe\inferno)

.PARAMETER PollIntervalSec
    Interval between GetStatus polls in seconds (default: 30)

.EXAMPLE
    .\soak_test.ps1 -DurationHours 8
    Run 8-hour soak test using default executable path and poll interval.

.EXAMPLE
    .\soak_test.ps1 -DurationHours 24 -PollIntervalSec 60 -ExePath "C:\inferno_wasapi.exe"
    Run 24-hour soak test with custom executable and 60-second poll interval.
#>

param(
    [int]$DurationHours = 8,
    [string]$ExePath = ".\target\release\inferno_wasapi.exe",
    [string]$PipeName = "\\.\pipe\inferno",
    [int]$PollIntervalSec = 30
)

$ErrorActionPreference = "Stop"
$WarningPreference = "Continue"

# ============================================================================
# Configuration
# ============================================================================

$TotalDurationSec = $DurationHours * 3600
$MaxConsecutiveIpcFailures = 2
$IpcTimeoutMs = 2000
$ProcessName = "inferno_wasapi"
$LogDir = "./soak_results"
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$CsvPath = "$LogDir/soak_results_$Timestamp.csv"
$AlertLogPath = "$LogDir/alerts_$Timestamp.log"

# ============================================================================
# Initialization
# ============================================================================

# Ensure output directory exists
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
}

Write-Host "=== Inferno Soak Test Harness ===" -ForegroundColor Cyan
Write-Host "Duration: $DurationHours hours"
Write-Host "Poll interval: $PollIntervalSec seconds"
Write-Host "CSV output: $CsvPath"
Write-Host "Alert log: $AlertLogPath"
Write-Host ""

# Initialize CSV header
$CsvHeader = "Timestamp,UptimeSecs,RxActive,TxActive,RxChannels,TxChannels,ClockMode,PeakDbMax,IpcOk"
$CsvHeader | Out-File -FilePath $CsvPath -Encoding UTF8
Write-Host "Created CSV: $CsvPath" -ForegroundColor Green

# ============================================================================
# IPC Communication Helper
# ============================================================================

function Invoke-InfernoIpc {
    param([string]$Message)
    
    try {
        $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", "inferno", 
            [System.IO.Pipes.PipeDirection]::InOut)
        $pipe.Connect($IpcTimeoutMs)
        
        $writer = New-Object System.IO.StreamWriter($pipe)
        $reader = New-Object System.IO.StreamReader($pipe)
        $writer.AutoFlush = $true
        
        $writer.WriteLine($Message)
        $response = $reader.ReadLine()
        
        $pipe.Close()
        $pipe.Dispose()
        
        if ($null -ne $response) {
            return $response | ConvertFrom-Json -ErrorAction SilentlyContinue
        }
        return $null
    }
    catch {
        return $null
    }
}

# ============================================================================
# Process Management
# ============================================================================

function Start-InfernoService {
    $existingProc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
    if ($existingProc) {
        Write-Host "inferno_wasapi already running (PID: $($existingProc.Id))" -ForegroundColor Yellow
        return $existingProc
    }
    
    if (-not (Test-Path $ExePath)) {
        throw "Executable not found: $ExePath"
    }
    
    Write-Host "Starting $ExePath..." -ForegroundColor Cyan
    $proc = Start-Process -FilePath $ExePath -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 2  # Give service time to initialize
    
    Write-Host "Process started (PID: $($proc.Id))" -ForegroundColor Green
    return $proc
}

function Assert-ProcessRunning {
    param([System.Diagnostics.Process]$Process)
    
    if ($Process.HasExited) {
        Write-Alert "CRITICAL: Process exited unexpectedly (exit code: $($Process.ExitCode))"
        return $false
    }
    return $true
}

# ============================================================================
# Logging and Alerts
# ============================================================================

function Write-Alert {
    param([string]$Message)
    
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $alertMsg = "[$timestamp] $Message"
    
    Write-Host $alertMsg -ForegroundColor Red
    Add-Content -Path $AlertLogPath -Value $alertMsg -Encoding UTF8
}

function Write-LogCsv {
    param(
        [datetime]$Timestamp,
        [int]$UptimeSecs,
        [bool]$RxActive,
        [bool]$TxActive,
        [int]$RxChannels,
        [int]$TxChannels,
        [string]$ClockMode,
        [double]$PeakDbMax,
        [bool]$IpcOk
    )
    
    $row = "{0:yyyy-MM-ddTHH:mm:ss},{1},{2},{3},{4},{5},{6},{7},{8}" -f `
        $Timestamp, $UptimeSecs, $RxActive, $TxActive, $RxChannels, `
        $TxChannels, $ClockMode, $PeakDbMax, $IpcOk
    
    Add-Content -Path $CsvPath -Value $row -Encoding UTF8
}

# ============================================================================
# Main Soak Test Loop
# ============================================================================

$process = Start-InfernoService
$startTime = Get-Date
$pollCount = 0
$ipcSuccessCount = 0
$ipcFailureCount = 0
$consecutiveIpcFailures = 0
$audioDropoutCount = 0
$lastRxActive = $null
$testEndTime = $startTime.AddSeconds($TotalDurationSec)

Write-Host "Starting soak test loop (will run until $(Get-Date $testEndTime -Format 'HH:mm:ss'))..." `
    -ForegroundColor Cyan
Write-Host ""

while ((Get-Date) -lt $testEndTime) {
    $pollCount++
    $now = Get-Date
    $elapsedSec = [int]($now - $startTime).TotalSeconds
    
    # Check if process is still alive
    if (-not (Assert-ProcessRunning $process)) {
        Write-Alert "Process died after $elapsedSec seconds ($pollCount polls)"
        break
    }
    
    # Poll IPC GetStatus
    $statusJson = Invoke-InfernoIpc '{"type":"GetStatus"}'
    
    if ($null -eq $statusJson) {
        $consecutiveIpcFailures++
        $ipcFailureCount++
        Write-LogCsv -Timestamp $now -UptimeSecs $elapsedSec -RxActive $false `
            -TxActive $false -RxChannels 0 -TxChannels 0 -ClockMode "Unknown" `
            -PeakDbMax 0.0 -IpcOk $false
        
        Write-Host "Poll $pollCount`: IPC TIMEOUT (consecutive failures: $consecutiveIpcFailures)" `
            -ForegroundColor Yellow
        
        if ($consecutiveIpcFailures -ge $MaxConsecutiveIpcFailures) {
            Write-Alert "IPC unresponsive for $($MaxConsecutiveIpcFailures) consecutive polls (120s)"
            break
        }
    }
    else {
        $consecutiveIpcFailures = 0
        $ipcSuccessCount++
        
        # Extract fields from response
        $rxActive = $statusJson.rx_active
        $txActive = $statusJson.tx_active
        $rxChannels = $statusJson.rx_channels
        $txChannels = $statusJson.tx_channels
        $clockMode = $statusJson.clock_mode
        $peakDbMax = $statusJson.peak_db_max
        
        # Check for audio dropouts
        if ($null -ne $lastRxActive -and $lastRxActive -eq $true -and $rxActive -eq $false) {
            $audioDropoutCount++
            Write-Alert "AUDIO DROPOUT DETECTED at poll $pollCount (rx_active went false)"
        }
        $lastRxActive = $rxActive
        
        # Log to CSV
        Write-LogCsv -Timestamp $now -UptimeSecs $elapsedSec -RxActive $rxActive `
            -TxActive $txActive -RxChannels $rxChannels -TxChannels $txChannels `
            -ClockMode $clockMode -PeakDbMax $peakDbMax -IpcOk $true
        
        # Status display every 10 polls
        if ($pollCount % 10 -eq 0) {
            $rxStr = if ($rxActive) { "ON " } else { "OFF" }
            $txStr = if ($txActive) { "ON " } else { "OFF" }
            Write-Host ("Poll {0,4}: elapsed={1,5}s | RX={2} TX={3} | " + `
                "Channels RX={4} TX={5} | Clock={6} | Peak={7,7}dB | IPC: {8}/{9} OK") -f `
                $pollCount, $elapsedSec, $rxStr, $txStr, $rxChannels, $txChannels, `
                $clockMode, $peakDbMax, $ipcSuccessCount, $pollCount `
                -ForegroundColor Gray
        }
    }
    
    # Wait before next poll
    if ((Get-Date) -lt $testEndTime) {
        Start-Sleep -Seconds $PollIntervalSec
    }
}

# ============================================================================
# Shutdown and Summary
# ============================================================================

$now = Get-Date
$totalElapsedSec = [int]($now - $startTime).TotalSeconds

Write-Host ""
Write-Host "=== Soak Test Complete ===" -ForegroundColor Cyan
Write-Host "Total elapsed time: $totalElapsedSec seconds ($([math]::Round($totalElapsedSec / 3600, 2)) hours)"
Write-Host "Total polls: $pollCount"
Write-Host "IPC successes: $ipcSuccessCount"
Write-Host "IPC failures: $ipcFailureCount"
Write-Host "Audio dropouts: $audioDropoutCount"
Write-Host ""
Write-Host "Results saved to:"
Write-Host "  CSV: $CsvPath"
Write-Host "  Alerts: $AlertLogPath"
Write-Host ""

# Optionally kill the service
$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
if ($proc) {
    Write-Host "Stopping inferno_wasapi (PID: $($proc.Id))..." -ForegroundColor Cyan
    Stop-Process -Id $proc.Id -Force
    Start-Sleep -Seconds 1
    Write-Host "Process stopped" -ForegroundColor Green
}

Write-Host ""
if ($ipcFailureCount -eq 0 -and $audioDropoutCount -eq 0) {
    Write-Host "✓ Soak test PASSED - no errors detected" -ForegroundColor Green
    exit 0
}
else {
    Write-Host "⚠ Soak test completed with warnings" -ForegroundColor Yellow
    Write-Host "  See alerts: $AlertLogPath" -ForegroundColor Yellow
    exit 1
}
