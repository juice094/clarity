#!/usr/bin/env pwsh
# Clarity Performance Benchmark Script
# Measures: compile time, binary startup time, runtime memory
# Output: target/benchmark-report.json

param(
    [switch]$SkipCompile,
    [string]$OutputPath = "target/benchmark-report.json"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$ProfileDir = if ($SkipCompile) { "debug" } else { "release" }

function Write-Report($data) {
    $json = $data | ConvertTo-Json -Depth 10
    $json | Out-File -Encoding utf8 $OutputPath
    Write-Host "Report written to: $OutputPath" -ForegroundColor Green
}

function Measure-CompileTime($crate, $features = @()) {
    Write-Host "Measuring compile time for $crate..." -ForegroundColor Cyan
    cargo clean -p $crate 2>$null
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    if ($features.Count -gt 0) {
        $feat = $features -join ","
        cargo build --release -p $crate --features $feat 2>&1 | Out-Null
    } else {
        cargo build --release -p $crate 2>&1 | Out-Null
    }
    $sw.Stop()
    return @{
        crate = $crate
        features = $features
        duration_ms = [int]$sw.ElapsedMilliseconds
        duration_human = $sw.Elapsed.ToString("mm\:ss\.fff")
    }
}

function Measure-StartupTime($binary, $argsList = @("--help"), $timeoutSec = 15) {
    $exe = "$repoRoot/target/$ProfileDir/$binary.exe"
    if (-not (Test-Path $exe)) {
        Write-Warning "Binary not found: $exe"
        return $null
    }
    Write-Host "Measuring startup time for $binary..." -ForegroundColor Cyan
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = $argsList -join " "
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    $exitOk = $proc.WaitForExit($timeoutSec * 1000)
    if (-not $exitOk) {
        try { $proc.Kill() } catch {}
    }
    $sw.Stop()
    return @{
        binary = $binary
        args = $argsList
        duration_ms = [int]$sw.ElapsedMilliseconds
        duration_human = $sw.Elapsed.ToString("mm\:ss\.fff")
        exit_code = $proc.ExitCode
        killed = -not $exitOk
    }
}

function Measure-ServiceStartup($binary, $readyPattern, $timeoutSec = 15) {
    $exe = "$repoRoot/target/$ProfileDir/$binary.exe"
    if (-not (Test-Path $exe)) {
        Write-Warning "Binary not found: $exe"
        return $null
    }
    Write-Host "Measuring service startup for $binary (pattern: '$readyPattern')..." -ForegroundColor Cyan
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $ready = $false
    $maxWait = [timespan]::FromSeconds($timeoutSec)
    while ($sw.Elapsed -lt $maxWait -and -not $proc.HasExited) {
        if ($proc.StandardOutput.EndOfStream -and $proc.StandardError.EndOfStream) {
            Start-Sleep -Milliseconds 50
            continue
        }
        $line = $proc.StandardOutput.ReadLineAsync().Result
        if (-not $line) { $line = $proc.StandardError.ReadLineAsync().Result }
        if ($line -and $line -match $readyPattern) {
            $ready = $true
            break
        }
    }
    $sw.Stop()
    if (-not $proc.HasExited) { try { $proc.Kill() } catch {} }
    return @{
        binary = $binary
        duration_ms = [int]$sw.ElapsedMilliseconds
        duration_human = $sw.Elapsed.ToString("mm\:ss\.fff")
        ready_detected = $ready
        killed = -not $proc.HasExited
    }
}

function Measure-Memory($binary, $argsList = @("--help"), $sampleIntervalMs = 100, $maxSamples = 50) {
    $exe = "$repoRoot/target/$ProfileDir/$binary.exe"
    if (-not (Test-Path $exe)) {
        Write-Warning "Binary not found: $exe"
        return $null
    }
    Write-Host "Measuring memory for $binary..." -ForegroundColor Cyan
    $proc = Start-Process -FilePath $exe -ArgumentList $argsList -PassThru -WindowStyle Hidden
    $samples = @()
    $maxWait = [timespan]::FromMilliseconds($sampleIntervalMs * $maxSamples + 2000)
    $start = [datetime]::Now
    while ($samples.Count -lt $maxSamples -and -not $proc.HasExited -and ([datetime]::Now - $start) -lt $maxWait) {
        Start-Sleep -Milliseconds $sampleIntervalMs
        try {
            $p = Get-Process -Id $proc.Id -ErrorAction Stop
            $samples += @{
                timestamp_ms = [int]([datetime]::Now - $start).TotalMilliseconds
                working_set_mb = [math]::Round($p.WorkingSet64 / 1MB, 2)
                private_memory_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)
            }
        } catch {
            break
        }
    }
    if (-not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    }
    if ($samples.Count -gt 0) {
        $peak_ws = ($samples | ForEach-Object { $_.working_set_mb } | Measure-Object -Maximum).Maximum
        $avg_ws = [math]::Round((($samples | ForEach-Object { $_.working_set_mb } | Measure-Object -Average).Average), 2)
    } else {
        $peak_ws = 0
        $avg_ws = 0
    }
    return @{
        binary = $binary
        samples = $samples
        peak_working_set_mb = $peak_ws
        avg_working_set_mb = $avg_ws
        sample_count = $samples.Count
    }
}

# ─── Main ───

$gitCommit = (git rev-parse --short HEAD 2>$null)
if (-not $gitCommit) { $gitCommit = "unknown" }
$rustVersion = (rustc --version 2>$null)
if (-not $rustVersion) { $rustVersion = "unknown" }

$report = @{
    metadata = @{
        timestamp = [datetime]::UtcNow.ToString("O")
        git_commit = $gitCommit
        rust_version = $rustVersion
        platform = "Windows"
        profile = $ProfileDir
    }
    compile = @()
    startup = @()
    memory = @()
}

# Compile benchmarks
if (-not $SkipCompile) {
    $report.compile += Measure-CompileTime "clarity-core"
    $report.compile += Measure-CompileTime "clarity-gateway"
    # clarity-tauri archived (v0.3.1), removed from workspace
    $report.compile += Measure-CompileTime "clarity-wire"
    $report.compile += Measure-CompileTime "clarity-memory"
} else {
    Write-Host "Skipping compile benchmarks (--SkipCompile); using $ProfileDir binaries" -ForegroundColor Yellow
}

# Startup benchmarks
$report.startup += Measure-StartupTime "clarity-headless" @("--help")
$report.startup += Measure-ServiceStartup "clarity-gateway" "API Server listening"
$report.startup += Measure-ServiceStartup "clarity-claw" "Claw tray icon active"

# Memory benchmarks
$report.memory += Measure-Memory "clarity-headless" @("--help")
$report.memory += Measure-Memory "clarity-gateway"

# Summary
$totalCompile = ($report.compile | Measure-Object -Property duration_ms -Sum).Sum
if (-not $totalCompile) { $totalCompile = 0 }
$report.summary = @{
    total_compile_time_ms = $totalCompile
    total_compile_time_human = [timespan]::FromMilliseconds($totalCompile).ToString("mm\:ss\.fff")
    crates_measured = $report.compile.Count
    binaries_measured = ($report.startup | Where-Object { $_ -ne $null }).Count
}

Write-Report $report

# Console summary
Write-Host "`n=== Benchmark Summary ===" -ForegroundColor Green
Write-Host "Profile: $ProfileDir"
Write-Host "Crates compiled: $($report.compile.Count)"
Write-Host "Binaries tested: $($report.summary.binaries_measured)"
if ($report.compile.Count -gt 0) {
    Write-Host "Total compile time: $($report.summary.total_compile_time_human)"
    foreach ($c in $report.compile) {
        Write-Host "  $($c.crate): $($c.duration_human)"
    }
}
foreach ($s in $report.startup) {
    if ($s) {
        $status = if ($s.ready_detected) { "ready" } elseif ($s.killed) { "killed" } else { "exited" }
        Write-Host "  $($s.binary) startup: $($s.duration_human) [$status]"
    }
}
foreach ($m in $report.memory) {
    if ($m -and $m.sample_count -gt 0) {
        Write-Host "  $($m.binary) memory: peak $($m.peak_working_set_mb) MB / avg $($m.avg_working_set_mb) MB ($($m.sample_count) samples)"
    }
}
