#!/usr/bin/env pwsh
# Clarity 项目验收脚本
# 使用: .\scripts\verify.ps1 [crate-name|--all]

param(
    [Parameter(Position=0)]
    [string]$Target = "--all",
    
    [switch]$Report,
    [switch]$Strict
)

$ErrorActionPreference = "Stop"
$StartTime = Get-Date

# 颜色设置
$Red = "Red"
$Green = "Green"
$Yellow = "Yellow"
$Cyan = "Cyan"

# 结果存储
$Results = @()

function Write-Header($text) {
    Write-Host "`n═══════════════════════════════════════════════════════════════" -ForegroundColor $Cyan
    Write-Host "  $text" -ForegroundColor $Cyan
    Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor $Cyan
}

function Write-Result($name, $passed, $details = "") {
    if ($passed) {
        Write-Host "  ✅ $name" -ForegroundColor $Green
    } else {
        Write-Host "  ❌ $name" -ForegroundColor $Red
    }
    if ($details) {
        Write-Host "     $details" -ForegroundColor Gray
    }
}

function Test-Crate($crateName) {
    Write-Header "验证 Crate: $crateName"
    
    $result = @{
        Crate = $crateName
        Timestamp = Get-Date -Format "o"
        Checks = @{}
        Overall = "FAIL"
    }
    
    # 检查 1: 文档存在性
    Write-Host "`n  [1/5] 文档检查..." -ForegroundColor Yellow
    $readmePath = "crates/$crateName/README.md"
    $agentsPath = "crates/$crateName/AGENTS.md"
    $readmePass = Test-Path $readmePath
    $agentsPass = Test-Path $agentsPath
    $docPass = $readmePass -and $agentsPass
    $result.Checks.Docs = @{
        Status = if ($docPass) { "PASS" } else { "FAIL" }
        Readme = $readmePass
        Agents = $agentsPass
    }
    Write-Result "README.md 存在" $readmePass
    Write-Result "AGENTS.md 存在" $agentsPass
    
    # 检查 2: 编译
    Write-Host "`n  [2/5] 编译检查..." -ForegroundColor Yellow
    $compileTime = Measure-Command {
        $compileOutput = cargo check -p $crateName 2>&1
        $compileExit = $LASTEXITCODE
    }
    
    $compilePass = $compileExit -eq 0
    $result.Checks.Compile = @{
        Status = if ($compilePass) { "PASS" } else { "FAIL" }
        DurationMs = [int]$compileTime.TotalMilliseconds
    }
    Write-Result "编译检查" $compilePass
    if (-not $compilePass) {
        Write-Host "     错误: $compileOutput" -ForegroundColor Red
    }
    
    # 检查 3: 单元测试
    Write-Host "`n  [3/5] 单元测试..." -ForegroundColor Yellow
    $testTime = Measure-Command {
        $testOutput = cargo test -p $crateName 2>&1
        $testExit = $LASTEXITCODE
    }
    
    # 解析测试结果
    $testSummary = $testOutput | Select-String "test result:"
    $passed = 0
    $failed = 0
    $ignored = 0
    
    if ($testSummary -match "(\d+) passed.*?(\d+) failed.*?(\d+) ignored") {
        $passed = [int]$matches[1]
        $failed = [int]$matches[2]
        $ignored = [int]$matches[3]
    }
    
    $testPass = $testExit -eq 0 -and $failed -eq 0
    $result.Checks.Test = @{
        Status = if ($testPass) { "PASS" } else { "FAIL" }
        Passed = $passed
        Failed = $failed
        Ignored = $ignored
        DurationMs = [int]$testTime.TotalMilliseconds
    }
    Write-Result "单元测试" $testPass "通过: $passed, 失败: $failed, 忽略: $ignored"
    if ($failed -gt 0) {
        $failedTests = $testOutput | Select-String "FAILED"
        foreach ($ft in $failedTests) {
            Write-Host "     $ft" -ForegroundColor Red
        }
    }
    
    # 检查 4: Clippy
    Write-Host "`n  [4/5] Clippy 检查..." -ForegroundColor Yellow
    $clippyOutput = cargo clippy -p $crateName -- -D warnings 2>&1
    $clippyExit = $LASTEXITCODE
    
    $clippyPass = $clippyExit -eq 0
    $result.Checks.Clippy = @{
        Status = if ($clippyPass) { "PASS" } else { "FAIL" }
        Warnings = if ($clippyPass) { 0 } else { 1 }
    }
    Write-Result "Clippy 检查" $clippyPass
    if (-not $clippyPass) {
        $warnings = $clippyOutput | Select-String "warning:|error:"
        foreach ($w in $warnings | Select-Object -First 5) {
            Write-Host "     $w" -ForegroundColor Red
        }
    }
    
    # 检查 5: 代码格式化
    Write-Host "`n  [5/5] 格式化检查..." -ForegroundColor Yellow
    $fmtOutput = cargo fmt -p $crateName -- --check 2>&1
    $fmtExit = $LASTEXITCODE
    
    $fmtPass = $fmtExit -eq 0
    $result.Checks.Fmt = @{
        Status = if ($fmtPass) { "PASS" } else { "FAIL" }
    }
    Write-Result "格式化检查" $fmtPass
    if (-not $fmtPass) {
        Write-Host "     代码需要格式化: cargo fmt -p $crateName" -ForegroundColor Yellow
    }
    
    # 总体结果
    $overallPass = $docPass -and $compilePass -and $testPass -and $clippyPass
    if ($Strict) {
        $overallPass = $overallPass -and $fmtPass
    }
    
    $result.Overall = if ($overallPass) { "PASS" } else { "FAIL" }
    
    Write-Host "`n  ─────────────────────────────────────────" -ForegroundColor Gray
    if ($overallPass) {
        Write-Host "  ✅ $crateName 验收通过" -ForegroundColor Green
    } else {
        Write-Host "  ❌ $crateName 验收失败" -ForegroundColor Red
    }
    Write-Host "  ─────────────────────────────────────────" -ForegroundColor Gray
    
    return $result
}

# 主程序
Write-Header "Clarity 项目验收脚本"
Write-Host "  目标: $Target" -ForegroundColor Gray
Write-Host "  严格模式: $Strict" -ForegroundColor Gray
Write-Host "  时间: $($StartTime.ToString("yyyy-MM-dd HH:mm:ss"))" -ForegroundColor Gray

# 进入项目目录
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

# 确定要验证的 crates
if ($Target -eq "--all") {
    $Crates = Get-ChildItem -Directory "crates/*" | ForEach-Object { $_.Name }
} else {
    $Crates = @($Target)
}

# 逐个验证
foreach ($crate in $Crates) {
    $result = Test-Crate $crate
    $Results += $result
}

# 生成报告
Write-Header "验收总结"

$totalPass = ($Results | Where-Object { $_.Overall -eq "PASS" }).Count
$totalFail = ($Results | Where-Object { $_.Overall -eq "FAIL" }).Count

Write-Host "`n  总计: $($Results.Count) 个 crate"
Write-Host "  通过: $totalPass" -ForegroundColor $Green
Write-Host "  失败: $totalFail" -ForegroundColor $Red

foreach ($r in $Results) {
    $status = if ($r.Overall -eq "PASS") { "✅" } else { "❌" }
    Write-Host "    $status $($r.Crate)" -ForegroundColor $(if ($r.Overall -eq "PASS") { $Green } else { $Red })
}

# 输出 JSON 报告
if ($Report) {
    $JsonReport = $Results | ConvertTo-Json -Depth 4
    $ReportFile = "verify-report-$(Get-Date -Format 'yyyyMMdd-HHmmss').json"
    $JsonReport | Out-File -FilePath $ReportFile -Encoding UTF8
    Write-Host "`n  报告已保存: $ReportFile" -ForegroundColor $Cyan
}

# 返回退出码
$ExitCode = if ($totalFail -gt 0) { 1 } else { 0 }
exit $ExitCode
