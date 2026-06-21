# 技术债务扫描公共函数
# 被 tech-debt-scan.ps1 和 tech-debt-scope-scan.ps1 dot-source 使用

$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)

$CanonicalRules = @(
    'yagni', 'unwrap', 'expect', 'panic', 'missing-test', 'path-validation',
    'dead-code', 'stdlib', 'unsafe', 'duplicate',
    'p1', 'p2', 'p3', 'p4', 'p5', 'p6', 'p7',
    'theme-token'
)

function Write-JsonLine {
    param([string]$Path, [object]$Data)
    $json = $Data | ConvertTo-Json -Compress -Depth 3
    [System.IO.File]::AppendAllText($Path, $json + "`n", $Utf8NoBom)
}

function Get-OpenDedupKeys {
    param([string]$DebtFile)
    $keys = @{}
    if (-not (Test-Path $DebtFile)) { return $keys }
    foreach ($line in [System.IO.File]::ReadLines($DebtFile)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try {
            $item = $line | ConvertFrom-Json -AsHashtable
            if ($item.status -in @('open', 'ignored') -and $item.dedup_key) {
                $keys[$item.dedup_key] = $true
                # 同时加入规则别名规范化后的 key，避免旧条目规则名不同导致重复录入
                $normRule = Normalize-Rule -Rule $item.rule
                if ($normRule -ne $item.rule) {
                    $normKey = "$($item.file)#$normRule#$($item.content_hash)"
                    $keys[$normKey] = $true
                }
            }
        }
        catch {}
    }
    return $keys
}

function Normalize-Rule {
    param([string]$Rule)
    if (-not $Rule) { return 'unknown' }
    $r = $Rule.Trim().ToLowerInvariant()
    $r = $r -replace '^ponytail:\s*', ''
    $r = $r -replace '\s*-\s*', '-'
    $r = $r -replace '\s+', '-'

    # 规则别名映射（允许 _/- 混用以及中文后缀）
    switch -Regex ($r) {
        '^(yagni|unnecessary[-_]abstraction|unused[-_]abstraction|redundant[-_]abstraction)$' { return 'yagni' }
        '^unwrap' { return 'unwrap' }
        '^expect' { return 'expect' }
        '^panic'  { return 'panic' }
        '^(missing[-_]test|no[-_]test|test[-_]missing|needs[-_]test)$' { return 'missing-test' }
        '^(path[-_]validation|sanitize[-_]path|invalid[-_]path|path[-_]traversal)$' { return 'path-validation' }
        '^(dead[-_]code|unused[-_]code|unused[-_]import|unused[-_]function)$' { return 'dead-code' }
        '^(stdlib|use[-_]stdlib|prefer[-_]stdlib|could[-_]be[-_]stdlib)$' { return 'stdlib' }
        '^unsafe' { return 'unsafe' }
        '^(duplicate|duplicated|duplicate[-_]code)$' { return 'duplicate' }
        '^p1' { return 'p1' }
        '^p2|删除优先|删除优于添加' { return 'p2' }
        '^p3' { return 'p3' }
        '^p4' { return 'p4' }
        '^p5' { return 'p5' }
        '^p6|theme-token|theme-token-required|egui-layout-token' { return 'p6' }
        '^p7' { return 'p7' }
    }
    return $r
}

function Repair-MechanicalLine {
    <#
    模型经常把行号报偏。对 unwrap/expect/panic 这类机械规则，
    在报告行附近搜索真实调用，返回修正后的行号；找不到返回 -1。
    #>
    param(
        [string]$Rule,
        [int]$Line,
        [string]$FileContent,
        [int]$Window = 15
    )
    $patterns = @{
        'unwrap' = '\.unwrap\('
        'expect' = '\.expect\('
        'panic'  = 'panic!\('
    }
    if (-not $patterns.ContainsKey($Rule) -or [string]::IsNullOrWhiteSpace($FileContent)) { return $Line }

    $re = $patterns[$Rule]
    $lines = $FileContent -split "`r?`n"
    $bestIdx = -1
    $bestDist = [int]::MaxValue
    $center = $Line - 1
    for ($i = [Math]::Max(0, $center - $Window); $i -le [Math]::Min($lines.Count - 1, $center + $Window); $i++) {
        if ($lines[$i] -match $re) {
            $dist = [Math]::Abs($i - $center)
            if ($dist -lt $bestDist) {
                $bestIdx = $i
                $bestDist = $dist
            }
        }
    }
    if ($bestIdx -ge 0) { return $bestIdx + 1 }
    return -1
}

function Test-YagniUsedAcrossCrate {
    <#
    对 pub(crate)/pub 的抽象，检查同一 crate 的其他文件是否也有引用。
    如果有，说明它被跨文件使用，不是 YAGNI。
    #>
    param(
        [string]$RepoRoot,
        [string]$File,
        [string]$Name
    )
    if ([string]::IsNullOrWhiteSpace($RepoRoot) -or [string]::IsNullOrWhiteSpace($File) -or [string]::IsNullOrWhiteSpace($Name)) {
        return $false
    }
    if ($File -notmatch '^crates/([^/]+)/') { return $false }
    $cratePath = "crates/$($matches[1])"
    $search = $Name -replace '\(\)$', ''
    try {
        $output = & git -C $RepoRoot grep -I -w -F --name-only $search HEAD -- $cratePath 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { return $false }
        $files = $output -split "`r?`n" | Where-Object { $_ } | Select-Object -Unique
        return $files.Count -gt 1
    }
    catch {
        return $false
    }
}

function Test-FalsePositive {
    param(
        [hashtable]$Finding,
        [string]$FileContent = '',
        [string]$RepoRoot = ''
    )

    $rule = $Finding.rule
    $desc = $Finding.description
    $sugg = $Finding.suggestion

    # 1. 描述或建议为空/过短
    if ([string]::IsNullOrWhiteSpace($desc) -or $desc.Length -lt 15) { return $true }

    # 2. YAGNI：常量/阈值/ imported 类型/已经在本文件被使用的抽象
    if ($rule -eq 'yagni') {
        if ($sugg -match '删除\s+(MAX_|MIN_|DEFAULT_|LIMIT_|THRESHOLD_|BATCH_|TIMEOUT_)') { return $true }
        if ($desc -match '(常量|阈值|配置项|配置值|枚举值)') { return $true }
        if ($FileContent) {
            $lines = $FileContent -split "`r?`n"
            $lineIdx = $Finding.line - 1
            $isPublic = $false
            if ($lineIdx -ge 0 -and $lineIdx -lt $lines.Count) {
                $target = $lines[$lineIdx]
                # 常量定义不是 YAGNI
                if ($target -match '^\s*(pub\s+)?const\s+') { return $true }
                # 只看 use / 类型使用而不是抽象定义
                if ($target -match '^\s*use\s+') { return $true }
                # pub(crate) / pub 的抽象需要跨文件检查
                if ($target -match '^\s*pub(?:\s*\([^)]*\))?\s+') { $isPublic = $true }
            }
            # 如果建议删除的对象在文件里出现不止一次（定义+使用），说明它被使用了
            $candidates = [System.Collections.Generic.List[string]]::new()
            # 1) 反引号里的标识符，支持 A::B::method()
            foreach ($m in [regex]::Matches($desc, '`([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)`')) {
                $candidates.Add($m.Groups[1].Value)
            }
            # 2) 名字后跟结构体/函数/trait/枚举/模块/抽象/方法/返回值
            $pattern = '\b([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*(?:\(\))?)\b\s*(结构体|函数|trait|枚举|模块|抽象|方法|的返回值)'
            foreach ($m in [regex]::Matches($desc, $pattern)) {
                $candidates.Add($m.Groups[1].Value)
            }
            # 3) CamelCase 标识符（结构体/枚举名）
            foreach ($m in [regex]::Matches($desc, '\b([A-Z][A-Za-z0-9_]*)\b')) {
                $candidates.Add($m.Groups[1].Value)
            }

            $seen = @{}
            foreach ($name in $candidates) {
                if ([string]::IsNullOrWhiteSpace($name)) { continue }
                if ($seen.ContainsKey($name)) { continue }
                $seen[$name] = $true
                # SnapshotConfig::new() -> 搜 SnapshotConfig::new 或 SnapshotConfig
                $search = $name -replace '\(\)$', ''
                $escaped = [regex]::Escape($search)
                $occurrences = ([regex]::Matches($FileContent, $escaped)).Count
                if ($occurrences -gt 1) { return $true }
                # pub(crate)/pub 的抽象还要检查同 crate 其他文件
                if ($isPublic -and (Test-YagniUsedAcrossCrate -RepoRoot $RepoRoot -File $Finding.file -Name $search)) {
                    return $true
                }
            }
            # YAGNI 必须指出具体对象；泛泛而谈的不算
            if ($candidates.Count -eq 0) { return $true }
        }
    }

    # 3. P1 但描述里没有具体标识符（泛泛而谈）
    if ($rule -eq 'p1') {
        if ($desc -notmatch '`[^`]+`' -and $desc -notmatch '[A-Za-z_][A-Za-z0-9_]*') { return $true }
    }

    # 4. 通用模板噪声
    $genericPhrases = @(
        '未请求的多余抽象或泛型层',
        '建议删除',
        '可以考虑',
        '可能违反',
        '建议重新评估',
        'unnecessary abstraction',
        'redundant code',
        'could be simplified'
    )
    foreach ($phrase in $genericPhrases) {
        if ($desc -eq $phrase) { return $true }
    }

    # 5. unwrap/expect/panic 但实际代码行没有对应调用（行号应已被 Repair-MechanicalLine 修正）
    if ($FileContent) {
        $lines = $FileContent -split "`r?`n"
        $lineIdx = $Finding.line - 1
        if ($lineIdx -ge 0 -and $lineIdx -lt $lines.Count) {
            $target = $lines[$lineIdx]
            if ($rule -eq 'unwrap' -and $target -notmatch '\.unwrap\(') { return $true }
            if ($rule -eq 'expect' -and $target -notmatch '\.expect\(') { return $true }
            if ($rule -eq 'panic' -and $target -notmatch 'panic!\(') { return $true }
        }
    }

    # 6. 规则不在已知集合，且描述很短
    if ($rule -notin $CanonicalRules -and $desc.Length -lt 30) { return $true }

    return $false
}

function Get-Confidence {
    param(
        [hashtable]$Finding,
        [bool]$ClippyOverlap,
        [int]$RepeatCount = 1
    )

    $score = 0.5
    $rule = $Finding.rule
    $desc = $Finding.description
    $sugg = $Finding.suggestion

    if ($ClippyOverlap) { $score += 0.3 }
    if (-not [string]::IsNullOrWhiteSpace($sugg) -and $sugg.Length -gt 10) { $score += 0.1 }
    if ($desc.Length -gt 30) { $score += 0.1 }
    if ($rule -in @('unwrap', 'expect', 'panic', 'path-validation', 'missing-test', 'unsafe')) { $score += 0.1 }
    if ($rule -eq 'yagni') { $score -= 0.2 }
    if ($desc -match '未请求的多余抽象') { $score -= 0.2 }
    if ($rule -notin $CanonicalRules) { $score -= 0.15 }
    if ($RepeatCount -ge 2) { $score += 0.1 }
    if ($RepeatCount -ge 3) { $score += 0.1 }

    return [Math]::Round([Math]::Max(0.0, [Math]::Min(1.0, $score)), 2)
}
