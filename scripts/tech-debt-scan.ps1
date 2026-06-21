#requires -Version 7.2
<#
.SYNOPSIS
    事件驱动的技术债务扫描器：把 git commit 的 diff 喂给本地 Ollama，结果写入 JSONL。
.DESCRIPTION
    只扫描自上次记录以来新增的 commit，按 file+rule+content_hash 去重，append-only 写入
    .clarity/tech-debt/debt.jsonl。Ollama 不可用时跳过并记录状态。
.PARAMETER Model
    Ollama 模型名，默认 qwen2.5:7b。
.PARAMETER InitialCommits
    首次运行且没有 state.json 时，扫描最近 N 个 commit，默认 5。
.PARAMETER RepoRoot
    仓库根目录。
.PARAMETER OllamaUrl
    Ollama API 地址。
.PARAMETER NumCtx
    传给模型的上下文长度，默认 8192。
.PARAMETER MaxTokens
    传给模型的最大输出 token 数，默认 4096。
.EXAMPLE
    .\scripts\tech-debt-scan.ps1
    .\scripts\tech-debt-scan.ps1 -Model llama3.1:8b -InitialCommits 10
    .\scripts\tech-debt-scan.ps1 -Model llama3.1:8b -MaxTokens 2048
#>
[CmdletBinding()]
param(
    [string]$Model = 'qwen2.5:7b',
    [int]$InitialCommits = 5,
    [string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path,
    [string]$OllamaUrl = 'http://localhost:11434',
    [int]$NumCtx = 8192,
    [int]$MaxTokens = 4096,
    [switch]$ClippyCheck,
    [int]$ClippyTimeout = 300
)

$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/tech-debt-common.ps1"

$DebtDir = Join-Path $RepoRoot '.clarity' 'tech-debt'
$DebtFile = Join-Path $DebtDir 'debt.jsonl'
$RawFile = Join-Path $DebtDir 'raw.jsonl'
$StateFile = Join-Path $DebtDir 'state.json'
$ClippyCacheFile = Join-Path $DebtDir 'clippy-cache.jsonl'
$TmpDir = Join-Path $DebtDir 'tmp'

function Initialize-DebtDir {
    foreach ($d in @($DebtDir, $TmpDir)) {
        if (-not (Test-Path $d)) {
            New-Item -ItemType Directory -Path $d -Force | Out-Null
        }
    }
}

function Get-AffectedPackages {
    param([string]$Diff)
    $pkgs = @{}
    foreach ($line in $Diff -split "`r?`n") {
        if ($line -match '^diff --git a/crates/([^/]+)/.*\.rs') {
            $pkgs[$matches[1]] = $true
        }
    }
    return $pkgs.Keys
}

function Get-ClippyCache {
    param([string]$Commit, [string]$Package)
    if (-not (Test-Path $ClippyCacheFile)) { return $null }
    foreach ($ln in [System.IO.File]::ReadLines($ClippyCacheFile)) {
        if ([string]::IsNullOrWhiteSpace($ln)) { continue }
        try {
            $obj = $ln | ConvertFrom-Json -AsHashtable
            if ($obj.commit -eq $Commit -and $obj.package -eq $Package) {
                return $obj.lints
            }
        }
        catch {}
    }
    return $null
}

function Save-ClippyCache {
    param([string]$Commit, [string]$Package, [array]$Lints)
    $entry = [ordered]@{
        commit = $Commit
        package = $Package
        timestamp = (Get-Date -Format 'o')
        lints = $Lints
    }
    Write-JsonLine -Path $ClippyCacheFile -Data $entry
}

function Get-ClippyLints {
    param([string]$Commit, [string]$Diff)
    $packages = Get-AffectedPackages -Diff $Diff
    $allLints = [System.Collections.Generic.List[hashtable]]::new()
    foreach ($pkg in $packages) {
        $cached = Get-ClippyCache -Commit $Commit -Package $pkg
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
                try {
                    $msg = $ln | ConvertFrom-Json -AsHashtable
                }
                catch { continue }
                if ($msg.reason -ne 'compiler-message') { continue }
                $m = $msg.message
                if ($m.level -notin @('warning', 'error')) { continue }
                foreach ($span in $m.spans) {
                    $normalized = ($span.file_name -replace '\\', '/')
                    if ($normalized -match '(crates/.+)') { $normalized = $matches[1] } else { continue }
                    $lints.Add(@{
                        file = $normalized
                        line = $span.line_start
                        code = $m.code.code
                        message = $m.message
                    })
                }
            }
            Save-ClippyCache -Commit $Commit -Package $pkg -Lints $lints
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
    if (-not $Lints) { return @{ overlap = $false; lint = $null } }
    $matches = $Lints | Where-Object { $_.file -eq $Finding.file -and $_.line -eq $Finding.line }
    if ($matches) {
        return @{ overlap = $true; lint = $matches[0].code }
    }
    return @{ overlap = $false; lint = $null }
}

function Get-State {
    if (Test-Path $StateFile) {
        return Get-Content -Raw -Path $StateFile | ConvertFrom-Json -AsHashtable
    }
    return @{
        last_scan_commit = $null
        last_scan_time = $null
        ollama_available = $false
        model_used = $Model
        total_debt_open = 0
        total_debt_fixed = 0
        skipped_scans = 0
        last_error = $null
        scan_count = 0
    }
}

function Save-State {
    param([hashtable]$State)
    $json = $State | ConvertTo-Json -Depth 3
    [System.IO.File]::WriteAllText($StateFile, $json, $Utf8NoBom)
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

function Get-NewCommits {
    param([string]$LastCommit)

    if ($LastCommit) {
        $log = & git -C $RepoRoot log "$LastCommit..HEAD" --reverse --format="%H`t%s" 2>&1 | Out-String
    }
    else {
        $log = & git -C $RepoRoot log --max-count=$InitialCommits --reverse --format="%H`t%s" HEAD 2>&1 | Out-String
    }

    if ($LASTEXITCODE -ne 0) {
        throw "git log 失败: $log"
    }

    $commits = @()
    foreach ($line in $log -split "`r?`n") {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        $hash, $subject = $line -split "`t", 2
        $commits += [pscustomobject]@{ hash = $hash; subject = $subject }
    }
    return $commits
}

function Get-CommitDiff {
    param([string]$Commit)

    $diff = & git -C $RepoRoot diff "$Commit^..$Commit" -- '*.rs' '*.toml' 'AGENTS.md' 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        # 可能是 root commit 或 merge，fallback 到 show
        $diff = & git -C $RepoRoot show --format='' -- '*.rs' '*.toml' 'AGENTS.md' $Commit 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "无法获取 $Commit 的 diff"
        }
    }
    return $diff
}

function Get-DiffFiles {
    param([string]$Diff)
    $files = @{}
    foreach ($line in $Diff -split "`r?`n") {
        if ($line -match '^diff --git a/(.+) b/(.+)$') {
            $files[$matches[2]] = $true
        }
    }
    return $files
}

function Get-ContentHash {
    param([string]$Commit, [string]$File, [int]$Line, [int]$ContextLines = 3)

    try {
        $content = & git -C $RepoRoot show "$Commit`:$File" 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { return $null }
    }
    catch {
        return $null
    }

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

function Invoke-Review {
    param([string]$Diff, [string]$Commit)

    $tmpFile = Join-Path $TmpDir "$Commit.diff"
    [System.IO.File]::WriteAllText($tmpFile, $Diff, $Utf8NoBom)

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

    # 去掉 markdown 围栏
    $text = [regex]::Replace($text, '(?ms)^\s*```(?:json)?\s*', '')
    $text = [regex]::Replace($text, '(?ms)\s*```\s*$', '')
    $text = $text.Trim()

    try {
        $parsed = $text | ConvertFrom-Json -AsHashtable
    }
    catch {
        return $null
    }

    # 处理 [{"findings": [...]}] 这种嵌套
    if ($parsed -is [hashtable] -and $parsed.ContainsKey('findings')) {
        $parsed = $parsed['findings']
    }
    if ($parsed -isnot [array]) {
        $parsed = @($parsed)
    }
    return $parsed
}

function Write-Raw {
    param([string]$Commit, [string]$RawOutput, [string]$Reason)
    $entry = [ordered]@{
        timestamp = (Get-Date -Format 'o')
        commit = $Commit
        model = $Model
        reason = $Reason
        raw = $RawOutput
    }
    Write-JsonLine -Path $RawFile -Data $entry
}

# ─── main ───
Initialize-DebtDir
$state = Get-State

if (-not (Test-OllamaAvailable)) {
    $state.ollama_available = $false
    $state.skipped_scans++
    $state.last_error = 'ollama_unavailable'
    $state.last_scan_time = (Get-Date -Format 'o')
    Save-State -State $state
    Write-Warning 'Ollama 不可用，跳过本次扫描。'
    exit 0
}

$state.ollama_available = $true
$state.model_used = $Model
$state.last_error = $null

$commits = Get-NewCommits -LastCommit $state.last_scan_commit

$openKeys = Get-OpenDedupKeys -DebtFile $DebtFile
$added = 0
$skipped = 0

# 预计算 open 债务中各 file+rule 的重复次数，用于置信度和 YAGNI 判断
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

foreach ($c in $commits) {
    $diff = Get-CommitDiff -Commit $c.hash
    if ([string]::IsNullOrWhiteSpace($diff)) {
        $state.last_scan_commit = $c.hash
        continue
    }

    $scanId = [Guid]::NewGuid().ToString('N')
    $scanTime = Get-Date -Format 'o'

    $clippyLints = $null
    if ($ClippyCheck) {
        $clippyLints = Get-ClippyLints -Commit $c.hash -Diff $diff
    }

    try {
        $rawOutput = Invoke-Review -Diff $diff -Commit $c.hash
    }
    catch {
        Write-Raw -Commit $c.hash -RawOutput $_.ToString() -Reason 'review_script_failed'
        $state.last_error = "review_failed:$($c.hash)"
        $state.last_scan_commit = $c.hash
        continue
    }

    $findings = Parse-Findings -RawText $rawOutput
    if ($null -eq $findings) {
        Write-Raw -Commit $c.hash -RawOutput $rawOutput -Reason 'json_parse_failed'
        $state.last_scan_commit = $c.hash
        continue
    }

    $changedFiles = Get-DiffFiles -Diff $diff

    foreach ($f in $findings) {
        if ($f -isnot [hashtable]) { continue }
        $file = $f['file']
        $rule = $f['rule']
        $line = $f['line']
        if (-not $file -or -not $rule) { continue }

        $file = $file.Trim()

        if (-not $changedFiles.ContainsKey($file)) {
            Write-Verbose "忽略不在本次 diff 中的 finding: ${file}"
            continue
        }

        $lineNum = 0
        [int]::TryParse($line, [ref]$lineNum) | Out-Null

        $fileContent = & git -C $RepoRoot show "$($c.hash):$file" 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { continue }

        $rawRule = $rule
        $rule = Normalize-Rule -Rule $rule
        $desc = ($f['description'] -as [string]) ?? ''
        $sugg = ($f['suggestion'] -as [string]) ?? ''

        # 修正机械规则行号；找不到真实调用则视为误报
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

        $contentHash = Get-ContentHash -Commit $c.hash -File $file -Line $lineNum
        if (-not $contentHash) { $contentHash = 'unknown' }

        $dedupKey = "$file`#$rule`#$contentHash"
        if ($openKeys.ContainsKey($dedupKey)) {
            $skipped++
            continue
        }

        $id = [BitConverter]::ToString([System.Security.Cryptography.SHA256]::HashData(
            [System.Text.Encoding]::UTF8.GetBytes($dedupKey + $c.hash)
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
            commit = $c.hash
            subject = $c.subject
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

    $state.last_scan_commit = $c.hash
}

$state.scan_count++
$state.last_scan_time = (Get-Date -Format 'o')

# 重新统计 open/fixed
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

Save-State -State $state

# 输出本次摘要 JSON
@{
    scanned_commits = $commits.Count
    new_findings = $added
    skipped_duplicates = $skipped
    state = $state
} | ConvertTo-Json -Depth 3
