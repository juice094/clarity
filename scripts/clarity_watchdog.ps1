#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Health watchdog + log rotation for Clarity scheduled tasks.

.DESCRIPTION
    Intended to run every minute from Task Scheduler. It:
    - Probes http://127.0.0.1:18790/health
    - Restarts the ClarityGateway task if the probe fails twice
    - Rotates .clarity/gateway_run.err.log when it exceeds 10 MB

.PARAMETER ProjectRoot
    Absolute path to the clarity repository root.
#>
param(
    [string]$ProjectRoot = (Resolve-Path "$PSScriptRoot\..").Path,
    [int]$MaxLogSizeMB = 10,
    [string]$HealthUrl = "http://127.0.0.1:18790/health",
    [int]$TimeoutSec = 5
)

$ErrorActionPreference = "Stop"
$logDir = "$ProjectRoot\.clarity"
$logFile = "$logDir\gateway_run.err.log"

function Test-GatewayHealth {
    try {
        $resp = Invoke-WebRequest -Uri $HealthUrl -TimeoutSec $TimeoutSec -UseBasicParsing
        return $resp.StatusCode -eq 200
    } catch {
        return $false
    }
}

function Restart-GatewayTask {
    Write-Host "Gateway health probe failed; restarting ClarityGateway task..."
    cmd /c "schtasks /run /tn ClarityGateway" | Out-Null
}

function Rotate-LogIfNeeded {
    param([string]$Path, [int]$MaxMB)
    if (-not (Test-Path $Path)) { return }
    $sizeMB = (Get-Item $Path).Length / 1MB
    if ($sizeMB -gt $MaxMB) {
        $timestamp = Get-Date -Format "yyyyMMddHHmmss"
        $rotated = "$Path.$timestamp"
        Move-Item -Path $Path -Destination $rotated -Force
        Write-Host "Rotated $Path to $rotated ($([math]::Round($sizeMB,2)) MB)"
    }
}

# Health watchdog
$healthy = Test-GatewayHealth
if (-not $healthy) {
    Start-Sleep -Seconds 3
    $healthy = Test-GatewayHealth
    if (-not $healthy) {
        Restart-GatewayTask
    }
}

# Log rotation
Rotate-LogIfNeeded -Path $logFile -MaxMB $MaxLogSizeMB
