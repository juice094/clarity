#requires -Version 7.2
<#
.SYNOPSIS
    消费 .clarity/tech-debt/debt.jsonl，面向 agent 输出批量 JSON。
.DESCRIPTION
    只输出 JSON，不输出人类提示。agent 可直接反序列化后做任务编排。
.PARAMETER Action
    list | next | status | fix | ignore
.PARAMETER Top
    next/list 时返回的最大条数，默认 10。
.PARAMETER Rule
    按规则过滤（大小写不敏感）。
.PARAMETER Severity
    按严重度过滤 high|medium|low。
.PARAMETER Id
    fix/ignore 时指定 debt id。
.EXAMPLE
    .\scripts\tech-debt-dev.ps1 next
    .\scripts\tech-debt-dev.ps1 next -Top 5 -Severity high
    .\scripts\tech-debt-dev.ps1 status
    .\scripts\tech-debt-dev.ps1 fix -Id <id>
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('list', 'next', 'status', 'fix', 'ignore', 'prune', 'hotspots')]
    [string]$Action = 'next',

    [int]$Top = 10,
    [string]$Rule = '',
    [string]$Severity = '',
    [string]$Id = '',
    [string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path
)

$ErrorActionPreference = 'Stop'

$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)

$DebtDir = Join-Path $RepoRoot '.clarity' 'tech-debt'
$DebtFile = Join-Path $DebtDir 'debt.jsonl'
$StateFile = Join-Path $DebtDir 'state.json'

$SeverityOrder = @{ high = 0; medium = 1; low = 2 }

function Read-Debts {
    $items = [System.Collections.Generic.List[hashtable]]::new()
    if (-not (Test-Path $DebtFile)) { return $items }

    foreach ($line in [System.IO.File]::ReadLines($DebtFile)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try {
            $obj = $line | ConvertFrom-Json -AsHashtable
            $items.Add($obj)
        }
        catch {
            # 忽略损坏行
        }
    }
    return $items
}

function Get-OpenDebts {
    $items = Read-Debts | Where-Object { $_.status -eq 'open' }

    if ($Rule) {
        $items = $items | Where-Object { $_.rule -like "*$Rule*" }
    }
    if ($Severity) {
        $items = $items | Where-Object { $_.severity -eq $Severity.ToLowerInvariant() }
    }

    # YAGNI 降级策略：同一 file+rule 出现 <3 次时，视为 low（设计意见，不是 bug）
    $ruleCounts = @{}
    foreach ($d in $items) {
        $key = "$($d.file)#$($d.rule)"
        $ruleCounts[$key] = ($ruleCounts[$key] ?? 0) + 1
    }
    foreach ($d in $items) {
        if ($d.rule -eq 'yagni') {
            $key = "$($d.file)#$($d.rule)"
            if ($ruleCounts[$key] -lt 3) {
                $d.severity = 'low'
                $d.yagni_demoted = $true
            }
        }
    }

    return $items | Sort-Object `
        @{ Expression = { if ($_.clippy_overlap -eq $true) { 0 } else { 1 } }; Descending = $false }, `
        @{ Expression = { 1 - ($_.confidence ?? 0.5) }; Descending = $false }, `
        @{ Expression = {
            $s = $_.severity
            if ($SeverityOrder.ContainsKey($s)) { $SeverityOrder[$s] } else { 1 }
        }; Descending = $false }, `
        @{ Expression = { $_.timestamp }; Descending = $true }
}

function Write-Json {
    param($Data)
    $Data | ConvertTo-Json -Depth 5 -Compress
}

function Update-Status {
    param([string]$TargetId, [string]$NewStatus)

    if (-not (Test-Path $DebtFile)) {
        throw "debt.jsonl 不存在"
    }

    $lines = [System.Collections.Generic.List[string]]::new()
    $found = $false
    foreach ($line in [System.IO.File]::ReadLines($DebtFile)) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            $lines.Add($line)
            continue
        }
        try {
            $obj = $line | ConvertFrom-Json -AsHashtable
            if ($obj.id -eq $TargetId) {
                $obj.status = $NewStatus
                $obj.resolved_at = (Get-Date -Format 'o')
                $found = $true
            }
            $lines.Add(($obj | ConvertTo-Json -Compress -Depth 3))
        }
        catch {
            $lines.Add($line)
        }
    }

    if (-not $found) {
        throw "未找到 id: $TargetId"
    }

    $tmp = "$DebtFile.tmp"
    [System.IO.File]::WriteAllLines($tmp, $lines, $Utf8NoBom)
    Move-Item -Path $tmp -Destination $DebtFile -Force

    Update-StateCounts -Debts ($lines | ForEach-Object { $_ | ConvertFrom-Json -AsHashtable })
}

function Rewrite-Debts {
    param([array]$Debts)
    $lines = $Debts | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 3 }
    $tmp = "$DebtFile.tmp"
    [System.IO.File]::WriteAllLines($tmp, $lines, $Utf8NoBom)
    Move-Item -Path $tmp -Destination $DebtFile -Force
}

function Update-StateCounts {
    param([array]$Debts)
    $state = @{}
    if (Test-Path $StateFile) {
        $state = Get-Content -Raw -Path $StateFile | ConvertFrom-Json -AsHashtable
    }
    $openCount = 0
    $fixedCount = 0
    foreach ($d in $Debts) {
        if ($d.status -eq 'open') { $openCount++ }
        elseif ($d.status -eq 'fixed') { $fixedCount++ }
    }
    $state.total_debt_open = $openCount
    $state.total_debt_fixed = $fixedCount
    [System.IO.File]::WriteAllText($StateFile, ($state | ConvertTo-Json -Depth 3), $Utf8NoBom)
}

function Invoke-Prune {
    $all = Read-Debts
    $staleIds = [System.Collections.Generic.List[string]]::new()
    foreach ($d in $all) {
        if ($d.status -ne 'open') { continue }

        $content = & git -C $RepoRoot show "HEAD`:$($d.file)" 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            $d.status = 'stale'
            $d.resolved_at = (Get-Date -Format 'o')
            $staleIds.Add($d.id)
            continue
        }

        $lines = $content -split "`r?`n"
        $start = [Math]::Max(0, $d.line - 1)
        $end = [Math]::Min($lines.Count - 1, $d.line - 1 + 3)
        $snippet = ($lines[$start..$end] | ForEach-Object { $_.Trim() }) -join "`n"
        if ([string]::IsNullOrWhiteSpace($snippet)) {
            $d.status = 'stale'
            $d.resolved_at = (Get-Date -Format 'o')
            $staleIds.Add($d.id)
            continue
        }
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($snippet)
        $hash = [BitConverter]::ToString([System.Security.Cryptography.SHA256]::HashData($bytes)).Replace('-', '').ToLowerInvariant()
        if ($hash -ne $d.content_hash) {
            $d.status = 'stale'
            $d.resolved_at = (Get-Date -Format 'o')
            $staleIds.Add($d.id)
        }
    }
    Rewrite-Debts -Debts $all
    Update-StateCounts -Debts $all
    return @{ stale_count = $staleIds.Count; stale_ids = $staleIds }
}

function Get-Hotspots {
    param([int]$Limit = 10)
    $open = Read-Debts | Where-Object { $_.status -eq 'open' }
    $byFile = @{}
    $byRule = @{}
    foreach ($d in $open) {
        $byFile[$d.file] = ($byFile[$d.file] ?? 0) + 1
        $byRule[$d.rule] = ($byRule[$d.rule] ?? 0) + 1
    }
    $topFiles = $byFile.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First $Limit | ForEach-Object { @{file = $_.Key; count = $_.Value} }
    $topRules = $byRule.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First $Limit | ForEach-Object { @{rule = $_.Key; count = $_.Value} }
    return @{ files = $topFiles; rules = $topRules }
}

switch ($Action) {
    'list' {
        $open = Get-OpenDebts | Select-Object -First $Top
        Write-Json -Data $open
    }
    'next' {
        $open = Get-OpenDebts | Select-Object -First $Top
        Write-Json -Data $open
    }
    'status' {
        $all = Read-Debts
        $state = @{}
        if (Test-Path $StateFile) {
            $state = Get-Content -Raw -Path $StateFile | ConvertFrom-Json -AsHashtable
        }
        $byRule = @{}
        $bySeverity = @{}
        foreach ($d in $all) {
            $r = $d.rule
            $s = $d.severity
            $byRule[$r] = ($byRule[$r] ?? 0) + 1
            $bySeverity[$s] = ($bySeverity[$s] ?? 0) + 1
        }
        Write-Json -Data ([ordered]@{
            total = $all.Count
            open = @($all | Where-Object { $_.status -eq 'open' }).Count
            fixed = @($all | Where-Object { $_.status -eq 'fixed' }).Count
            ignored = @($all | Where-Object { $_.status -eq 'ignored' }).Count
            stale = @($all | Where-Object { $_.status -eq 'stale' }).Count
            by_rule = $byRule
            by_severity = $bySeverity
            state = $state
        })
    }
    'fix' {
        if (-not $Id) { throw "fix 需要 -Id" }
        Update-Status -TargetId $Id -NewStatus 'fixed'
        Write-Json -Data @{ id = $Id; status = 'fixed' }
    }
    'ignore' {
        if (-not $Id) { throw "ignore 需要 -Id" }
        Update-Status -TargetId $Id -NewStatus 'ignored'
        Write-Json -Data @{ id = $Id; status = 'ignored' }
    }
    'prune' {
        $result = Invoke-Prune
        Write-Json -Data $result
    }
    'hotspots' {
        $result = Get-Hotspots -Limit $Top
        Write-Json -Data $result
    }
}
