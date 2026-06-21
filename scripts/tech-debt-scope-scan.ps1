#requires -Version 7.2
<#
.SYNOPSIS
    按范围扫描项目代码，不依赖 commit diff。
.DESCRIPTION
    用 git ls-files 拿到范围内的 .rs 文件，分批喂给 Ollama 审查，结果写入 debt.jsonl。
    适合对单个 crate 或协议层做一次性/周期性扫描。
.PARAMETER Scope
    范围路径前缀，可多个，例如 @("crates/clarity-egui", "crates/clarity-wire")。
.PARAMETER Model
    Ollama 模型名，默认 qwen2.5:7b。
.PARAMETER MaxBatchChars
    每个 prompt 最大字符数，默认 15000。
.PARAMETER ClippyCheck
    是否运行 cargo clippy 做交叉验证。
.EXAMPLE
    .\scripts\tech-debt-scope-scan.ps1 -Scope crates/clarity-wire
    .\scripts\tech-debt-scope-scan.ps1 -Scope crates/clarity-egui -ClippyCheck
    .\scripts\tech-debt-scope-scan.ps1 -Scope crates/clarity-gateway/src/handlers,crates/clarity-wire
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string[]]$Scope,
    [string]$Model = 'qwen2.5:7b',
    [string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path,
    [string]$OllamaUrl = 'http://localhost:11434',
    [int]$NumCtx = 8192,
    [int]$MaxTokens = 4096,
    [int]$MaxBatchChars = 15000,
    [switch]$ClippyCheck,
    [int]$ClippyTimeout = 300
)

$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/tech-debt-common.ps1"

$DebtDir = Join-Path $RepoRoot '.clarity' 'tech-debt'
$DebtFile = Join-Path $DebtDir 'debt.jsonl'
$ClippyCacheFile = Join-Path $DebtDir 'clippy-cache.jsonl'
$TmpDir = Join-Path $DebtDir 'tmp'

function Initialize-DebtDir {
    foreach ($d in @($DebtDir, $TmpDir)) {
        if (-not (Test-Path $d)) {
            New-Item -ItemType Directory -Path $d -Force | Out-Null
        }
    }
}

function Test-OllamaAvailable {
    try {
        $null = Invoke-RestMethod -Uri "$OllamaUrl/api/tags" -Method Get -TimeoutSec 5
        return $true
    }
    catch {
        return $false
    }
}

function Get-ScopedFiles {
    $all = & git -C $RepoRoot ls-files HEAD -- '*.rs' 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "git ls-files 失败: $all" }
    $lines = $all -split "`r?`n" | Where-Object { $_ }
    $filtered = $lines | Where-Object {
        $p = $_
        foreach ($s in $Scope) {
            if ($p.StartsWith($s.TrimStart('/').Replace('\', '/'), [System.StringComparison]::OrdinalIgnoreCase)) {
                return $true
            }
        }
        return $false
    }
    return $filtered | Sort-Object
}

function Read-FileAtHead {
    param([string]$File)
    $content = & git -C $RepoRoot show "HEAD`:$File" 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { return $null }
    return $content
}

function Get-ContentHash {
    param([string]$File, [int]$Line, [int]$ContextLines = 3)
    $content = Read-FileAtHead -File $File
    if (-not $content) { return $null }
    $lines = $content -split "`r?`n"
    $start = [Math]::Max(0, $Line - 1)
    $end = [Math]::Min($lines.Count - 1, $Line - 1 + $ContextLines)
    if ($start -gt $end) { return $null }
    $snippet = ($lines[$start..$end] | ForEach-Object { $_.Trim() }) -join "`n"
    if ([string]::IsNullOrWhiteSpace($snippet)) { return $null }
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($snippet)
    $hash = [System.Security.Cryptography.SHA256]::HashData($bytes)
    return [BitConverter]::ToString($hash).Replace('-', '').ToLowerInvariant()
}

function Build-Batches {
    param([array]$Files)
    $batches = [System.Collections.Generic.List[array]]::new()
    $current = [System.Collections.Generic.List[string]]::new()
    $currentChars = 0
    foreach ($f in $Files) {
        $content = Read-FileAtHead -File $f
        if (-not $content) { continue }
        $size = $content.Length
        if ($size -gt $MaxBatchChars) {
            # 单文件就超限，单独一批
            if ($current.Count -gt 0) {
                $batches.Add($current.ToArray())
                $current.Clear()
                $currentChars = 0
            }
            $batches.Add(@($f))
            continue
        }
        if ($currentChars + $size -gt $MaxBatchChars -and $current.Count -gt 0) {
            $batches.Add($current.ToArray())
            $current.Clear()
            $currentChars = 0
        }
        $current.Add($f)
        $currentChars += $size
    }
    if ($current.Count -gt 0) {
        $batches.Add($current.ToArray())
    }
    return $batches
}

function Build-PseudoDiff {
    param([array]$Files)
    $sb = [System.Text.StringBuilder]::new()
    foreach ($f in $Files) {
        $content = Read-FileAtHead -File $f
        if (-not $content) { continue }
        [void]$sb.AppendLine("=== FILE: $f ===")
        $lines = $content -split "`r?`n"
        for ($i = 0; $i -lt $lines.Count; $i++) {
            [void]$sb.AppendLine("$($i + 1): $($lines[$i])")
        }
    }
    return $sb.ToString()
}

function Invoke-Review {
    param([string]$DiffText)
    $tmpFile = Join-Path $TmpDir "scope_$(New-Guid).diff"
    [System.IO.File]::WriteAllText($tmpFile, $DiffText, $Utf8NoBom)
    $reviewScript = Join-Path $PSScriptRoot 'ollama-review.ps1'
    $output = & pwsh -NoProfile -ExecutionPolicy Bypass -File $reviewScript `
        -Model $Model `
        -DiffFile $tmpFile `
        -Json -Quiet `
        -OllamaUrl $OllamaUrl `
        -NumCtx $NumCtx `
        -MaxTokens $MaxTokens 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "ollama-review.ps1 失败: $output"
    }
    return $output
}

function Parse-Findings {
    param([string]$RawText)
    $text = $RawText.Trim()
    if ([string]::IsNullOrWhiteSpace($text)) { return @() }
    $text = [regex]::Replace($text, '(?ms)^\s*```(?:json)?\s*', '')
    $text = [regex]::Replace($text, '(?ms)\s*```\s*$', '')
    $text = $text.Trim()
    if ([string]::IsNullOrWhiteSpace($text)) { return @() }
    try {
        $parsed = $text | ConvertFrom-Json -AsHashtable
    }
    catch {
        return $null
    }
    if ($parsed -is [hashtable] -and $parsed.ContainsKey('findings')) {
        $parsed = $parsed['findings']
    }
    if ($parsed -isnot [array]) { $parsed = @($parsed) }
    return $parsed
}

function Get-AffectedPackages {
    param([array]$Files)
    $pkgs = @{}
    foreach ($f in $Files) {
        if ($f -match '^crates/([^/]+)/') {
            $pkgs[$matches[1]] = $true
        }
    }
    return $pkgs.Keys
}

function Get-ClippyLints {
    param([array]$Files)
    $packages = Get-AffectedPackages -Files $Files
    $allLints = [System.Collections.Generic.List[hashtable]]::new()
    $head = git -C $RepoRoot rev-parse HEAD
    foreach ($pkg in $packages) {
        if (-not (Test-Path $ClippyCacheFile)) { $cached = $null } else {
            $cached = $null
            foreach ($ln in [System.IO.File]::ReadLines($ClippyCacheFile)) {
                if ([string]::IsNullOrWhiteSpace($ln)) { continue }
                try {
                    $obj = $ln | ConvertFrom-Json -AsHashtable
                    if ($obj.commit -eq $head -and $obj.package -eq $pkg) { $cached = $obj.lints; break }
                }
                catch {}
            }
        }
        if ($cached) {
            $allLints.AddRange($cached)
            continue
        }
        try {
            $job = Start-Job -ScriptBlock {
                param($pkg)
                & cargo clippy -p $pkg --message-format=json 2>&1 | Out-String
            } -ArgumentList $pkg
            if (-not ($job | Wait-Job -Timeout $ClippyTimeout)) {
                Stop-Job $job
                Remove-Job $job
                throw "cargo clippy -p $pkg 超时"
            }
            $output = ($job | Receive-Job)
            Remove-Job $job
            $lints = [System.Collections.Generic.List[hashtable]]::new()
            foreach ($ln in $output -split "`r?`n") {
                if (-not $ln.StartsWith('{')) { continue }
                try { $msg = $ln | ConvertFrom-Json -AsHashtable } catch { continue }
                if ($msg.reason -ne 'compiler-message') { continue }
                $m = $msg.message
                if ($m.level -notin @('warning', 'error')) { continue }
                foreach ($span in $m.spans) {
                    $normalized = ($span.file_name -replace '\\', '/')
                    if ($normalized -match '(crates/.+)') { $normalized = $matches[1] } else { continue }
                    $lints.Add(@{ file = $normalized; line = $span.line_start; code = $m.code.code; message = $m.message })
                }
            }
            Write-JsonLine -Path $ClippyCacheFile -Data @{ commit = $head; package = $pkg; timestamp = (Get-Date -Format 'o'); lints = $lints }
            $allLints.AddRange($lints)
        }
        catch {
            Write-Warning "clippy -p $pkg 失败: $_"
        }
    }
    return $allLints
}

function Get-ClippyMatch {
    param([hashtable]$Finding, [array]$Lints)
    if (-not $Lints) { return @{ overlap = $null; lint = $null } }
    $matches = $Lints | Where-Object { $_.file -eq $Finding.file -and $_.line -eq $Finding.line }
    if ($matches) { return @{ overlap = $true; lint = $matches[0].code } }
    return @{ overlap = $false; lint = $null }
}

# ─── main ───
Initialize-DebtDir

if (-not (Test-OllamaAvailable)) {
    Write-Warning 'Ollama 不可用，跳过本次范围扫描。'
    exit 0
}

$files = Get-ScopedFiles
if ($files.Count -eq 0) {
    Write-Host "范围内没有 .rs 文件" -ForegroundColor Yellow
    exit 0
}

Write-Host "范围内共 $($files.Count) 个 .rs 文件" -ForegroundColor Cyan

$openKeys = Get-OpenDedupKeys -DebtFile $DebtFile
$batches = Build-Batches -Files $files
$added = 0
$skipped = 0

$ruleRepeatCounts = @{}
if (Test-Path $DebtFile) {
    foreach ($line in [System.IO.File]::ReadLines($DebtFile)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try {
            $item = $line | ConvertFrom-Json -AsHashtable
            if ($item.status -eq 'open') {
                $key = "$($item.file)#$($item.rule)"
                $ruleRepeatCounts[$key] = ($ruleRepeatCounts[$key] ?? 0) + 1
            }
        }
        catch {}
    }
}

$clippyLints = $null
if ($ClippyCheck) {
    $clippyLints = Get-ClippyLints -Files $files
}

$head = git -C $RepoRoot rev-parse HEAD
$scanId = [Guid]::NewGuid().ToString('N')
$scanTime = Get-Date -Format 'o'

$validFilesSet = @{}
foreach ($f in $files) { $validFilesSet[$f] = $true }

foreach ($batch in $batches) {
    $diffText = Build-PseudoDiff -Files $batch
    try {
        $rawOutput = Invoke-Review -DiffText $diffText
    }
    catch {
        Write-Warning "审查失败: $_"
        continue
    }

    $findings = Parse-Findings -RawText $rawOutput
    if ($null -eq $findings) {
        Write-Warning "无法解析 JSON 输出"
        continue
    }

    foreach ($f in $findings) {
        if ($f -isnot [hashtable]) { continue }
        $file = $f['file']
        $rule = $f['rule']
        $line = $f['line']
        if (-not $file -or -not $rule) { continue }

        $file = $file.Trim()
        $lineNum = 0
        [int]::TryParse($line, [ref]$lineNum) | Out-Null

        if (-not $validFilesSet.ContainsKey($file)) {
            Write-Verbose "忽略不在扫描范围内的 finding: ${file}"
            continue
        }

        $fileContent = Read-FileAtHead -File $file
        if (-not $fileContent) { continue }

        $rawRule = $rule
        $rule = Normalize-Rule -Rule $rule
        $desc = ($f['description'] -as [string]) ?? ''
        $sugg = ($f['suggestion'] -as [string]) ?? ''

        # 修正机械规则行号；修正后仍找不到真实调用则视为误报
        $repairedLine = Repair-MechanicalLine -Rule $rule -Line $lineNum -FileContent $fileContent
        if ($repairedLine -eq -1) {
            Write-Verbose "机械规则在附近找不到真实调用，跳过: ${file}:$lineNum rule=$rule"
            continue
        }
        $lineNum = $repairedLine

        if (Test-FalsePositive -Finding @{rule = $rule; description = $desc; suggestion = $sugg; line = $lineNum; file = $file} -FileContent $fileContent -RepoRoot $RepoRoot) {
            Write-Verbose "过滤掉疑似误报: ${file}:$lineNum rule=$rule"
            continue
        }

        $contentHash = Get-ContentHash -File $file -Line $lineNum
        if (-not $contentHash) {
            Write-Verbose "无法计算 content_hash，跳过: ${file}:$lineNum"
            continue
        }

        $dedupKey = "$file`#$rule`#$contentHash"
        if ($openKeys.ContainsKey($dedupKey)) {
            $skipped++
            continue
        }

        $id = [BitConverter]::ToString([System.Security.Cryptography.SHA256]::HashData(
            [System.Text.Encoding]::UTF8.GetBytes($dedupKey + $head)
        )).Replace('-', '').ToLowerInvariant()

        $clippyMatch = if ($ClippyCheck) {
            Get-ClippyMatch -Finding @{ file = $file; line = $lineNum } -Lints $clippyLints
        } else {
            @{ overlap = $null; lint = $null }
        }

        $repeatKey = "$file#$rule"
        $repeatCount = $ruleRepeatCounts[$repeatKey] ?? 0
        $confidence = Get-Confidence -Finding @{rule = $rule; description = $desc; suggestion = $sugg} -ClippyOverlap ($clippyMatch.overlap -eq $true) -RepeatCount $repeatCount

        $entry = [ordered]@{
            id = $id
            scan_id = $scanId
            timestamp = $scanTime
            commit = $head
            subject = "scope-scan: $($Scope -join ', ')"
            scope = $Scope
            model = $Model
            status = 'open'
            severity = ($f['severity'] -as [string]) ?? 'medium'
            rule = $rule
            raw_rule = $rawRule
            file = $file
            line = $lineNum
            dedup_key = $dedupKey
            content_hash = $contentHash
            description = $desc
            suggestion = $sugg
            confidence = $confidence
            clippy_overlap = $clippyMatch.overlap
            clippy_lint = $clippyMatch.lint
        }

        Write-JsonLine -Path $DebtFile -Data $entry
        $openKeys[$dedupKey] = $true
        $added++
    }
}

# 更新 state
$state = @{}
if (Test-Path (Join-Path $DebtDir 'state.json')) {
    $state = Get-Content -Raw -Path (Join-Path $DebtDir 'state.json') | ConvertFrom-Json -AsHashtable
}
$state.last_scan_time = (Get-Date -Format 'o')
$state.last_scan_commit = $head
$state.model_used = $Model
$state.ollama_available = $true
$state.scan_count = ($state.scan_count ?? 0) + 1
$openCount = 0
$fixedCount = 0
if (Test-Path $DebtFile) {
    foreach ($line in [System.IO.File]::ReadLines($DebtFile)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try {
            $item = $line | ConvertFrom-Json -AsHashtable
            if ($item.status -eq 'open') { $openCount++ }
            elseif ($item.status -eq 'fixed') { $fixedCount++ }
        }
        catch {}
    }
}
$state.total_debt_open = $openCount
$state.total_debt_fixed = $fixedCount
[System.IO.File]::WriteAllText((Join-Path $DebtDir 'state.json'), ($state | ConvertTo-Json -Depth 3), $Utf8NoBom)

@{
    scope = $Scope
    files = $files.Count
    batches = $batches.Count
    new_findings = $added
    skipped_duplicates = $skipped
    total_open = $openCount
    state = $state
} | ConvertTo-Json -Depth 3
