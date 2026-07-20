#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Register Clarity Gateway and Claw as persistent Windows Scheduled Tasks.

.DESCRIPTION
    Creates two tasks that start on user logon:
    - ClarityGateway: runs clarity-gateway.exe (public API + admin UI)
    - ClarityClaw:    runs clarity-claw.exe (system-tray mesh node)

    Logs are written to .clarity\gateway-service.log and .clarity\claw-service.log
    under the project root.

.PARAMETER ProjectRoot
    Absolute path to the clarity repository root. Defaults to the script's parent directory.

.PARAMETER GatewayExe
    Path to clarity-gateway.exe. Defaults to target\release\clarity-gateway.exe under ProjectRoot.

.PARAMETER ClawExe
    Path to clarity-claw.exe. Defaults to target\release\clarity-claw.exe under ProjectRoot.
#>
param(
    [string]$ProjectRoot = (Resolve-Path "$PSScriptRoot\..").Path,
    [string]$GatewayExe = "$ProjectRoot\target\release\clarity-gateway.exe",
    [string]$ClawExe = "$ProjectRoot\target\release\clarity-claw.exe",
    [string]$TlsProxyScript = "$ProjectRoot\scripts\tls_reverse_proxy.py",
    [string]$WatchdogScript = "$ProjectRoot\scripts\clarity_watchdog.ps1"
)

$ErrorActionPreference = "Stop"

# Resolve Python executable path for scheduled tasks. The task scheduler may not
# inherit the user's PATH, so "python" alone fails with 0x80070002.
$pythonExe = (Get-Command python -ErrorAction SilentlyContinue)?.Source
if (-not $pythonExe) {
    $pythonExe = (Get-Command py -ErrorAction SilentlyContinue)?.Source
}
if (-not $pythonExe) {
    throw "Python executable not found in PATH. Please install Python and ensure it is on PATH."
}
Write-Host "Using Python: $pythonExe"

function Register-ClarityTask {
    param(
        [string]$Name,
        [string]$Executable,
        [string]$WorkingDir,
        [string]$Argument = ""
    )

    $task = Get-ScheduledTask -TaskName $Name -ErrorAction SilentlyContinue
    if ($task) {
        Write-Host "Removing existing task '$Name'..."
        Unregister-ScheduledTask -TaskName $Name -Confirm:$false
    }

    $actionArgs = @{
        Execute          = $Executable
        WorkingDirectory = $WorkingDir
    }
    if ($Argument) {
        $actionArgs['Argument'] = $Argument
    }
    $action = New-ScheduledTaskAction @actionArgs

    # Start on user logon; do not start multiple instances.
    $trigger = New-ScheduledTaskTrigger -AtLogon

    $principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -LogonType Interactive

    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -StartWhenAvailable `
        -MultipleInstances IgnoreNew `
        -ExecutionTimeLimit (New-TimeSpan -Days 0)

    Write-Host "Registering scheduled task '$Name'..."
    Register-ScheduledTask `
        -TaskName $Name `
        -Action $action `
        -Trigger $trigger `
        -Principal $principal `
        -Settings $settings `
        -Force | Out-Null

    Write-Host "Task '$Name' registered. Executable: $Executable"
}

if (-not (Test-Path $GatewayExe)) {
    throw "Gateway executable not found: $GatewayExe. Build with: cargo build --release -p clarity-gateway"
}
if (-not (Test-Path $ClawExe)) {
    throw "Claw executable not found: $ClawExe. Build with: cargo build --release -p clarity-claw"
}
if (-not (Test-Path $TlsProxyScript)) {
    throw "TLS proxy script not found: $TlsProxyScript"
}
if (-not (Test-Path $WatchdogScript)) {
    throw "Watchdog script not found: $WatchdogScript"
}

# Ensure a persistent admin token exists and is exposed to the Gateway process.
$tokenFile = "$ProjectRoot\.clarity\admin-token"
if (-not (Test-Path $tokenFile)) {
    $bytes = New-Object byte[] 48
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    $rng.GetBytes($bytes)
    $token = [Convert]::ToBase64String($bytes)
    Set-Content -Path $tokenFile -Value $token -NoNewline
    Write-Host "Generated admin token: $tokenFile"
}
$adminToken = Get-Content -Path $tokenFile -Raw
[Environment]::SetEnvironmentVariable("CLARITY_ADMIN_TOKEN", $adminToken, "User")
Write-Host "CLARITY_ADMIN_TOKEN set in user environment"

# Generate a local CA + server certificate for the TLS proxy.
# The CA is trusted on this machine; phones/clients on the same LAN must install
# .clarity/local-ca.crt to avoid self-signed certificate warnings.
$caGenScript = "$ProjectRoot\scripts\generate_local_ca.py"
if (Test-Path $caGenScript) {
    Write-Host "Generating local CA and server certificate..."
    & $pythonExe "$caGenScript" --force

    $caCert = "$ProjectRoot\.clarity\local-ca.crt"
    if (Test-Path $caCert) {
        Write-Host "Trusting local CA on this Windows machine..."
        cmd /c "certutil -addstore -f ROOT `"$caCert`"" | Out-Null
    }
}

Register-ClarityTask -Name "ClarityGateway"    -Executable $GatewayExe      -WorkingDir $ProjectRoot
Register-ClarityTask -Name "ClarityClaw"       -Executable $ClawExe         -WorkingDir $ProjectRoot
Register-ClarityTask -Name "ClarityTLSProxy"   -Executable $pythonExe       -WorkingDir $ProjectRoot -Argument $TlsProxyScript

# Watchdog runs every minute with a hidden window.
$watchdogTask = Get-ScheduledTask -TaskName "ClarityWatchdog" -ErrorAction SilentlyContinue
if ($watchdogTask) {
    Write-Host "Removing existing task 'ClarityWatchdog'..."
    Unregister-ScheduledTask -TaskName "ClarityWatchdog" -Confirm:$false
}
$wdAction = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-ExecutionPolicy Bypass -File `"$WatchdogScript`"" -WorkingDirectory $ProjectRoot
$wdTrigger = New-ScheduledTaskTrigger -Once -At (Get-Date) -RepetitionInterval (New-TimeSpan -Minutes 1) -RepetitionDuration (New-TimeSpan -Days 3650)
$wdPrincipal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -LogonType Interactive
$wdSettings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -MultipleInstances IgnoreNew
Register-ScheduledTask -TaskName "ClarityWatchdog" -Action $wdAction -Trigger $wdTrigger -Principal $wdPrincipal -Settings $wdSettings -Force | Out-Null
Write-Host "Registered scheduled task 'ClarityWatchdog' (every minute)"

Write-Host "Done. Tasks will start at next logon. To start now, run:"
Write-Host "  schtasks /run /tn ClarityGateway"
Write-Host "  schtasks /run /tn ClarityClaw"
Write-Host "  schtasks /run /tn ClarityTLSProxy"
Write-Host "  schtasks /run /tn ClarityWatchdog"
