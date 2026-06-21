#requires -Version 7.2
<#
.SYNOPSIS
    用本地 Ollama 模型审查 Clarity git diff 是否符合 AGENTS.md 风格/健康规则。
.DESCRIPTION
    取指定 commit 范围的 diff，把 AGENTS.md 的 Ponytail + P1-P7 规则一起喂给 Ollama，
    输出审查意见。只做辅助审查，不自动修改代码。
.PARAMETER Model
    Ollama 模型名，默认 qwen2.5:7b。
.PARAMETER Range
    git diff 范围，默认 HEAD~1..HEAD（仅已提交变更）。
.PARAMETER RepoRoot
    仓库根目录，默认脚本所在目录的父目录。
.PARAMETER RulesFile
    规则文件路径，默认 <RepoRoot>/AGENTS.md。
.PARAMETER Json
    要求模型输出 JSON 并解析为 PowerShell 对象。
.PARAMETER OllamaUrl
    Ollama API 地址，默认 http://localhost:11434。
.PARAMETER DiffFile
    如果提供，直接读取该 diff 文件，而不是调用 git diff。
.PARAMETER Quiet
    静默模式：不输出进度和警告，只输出审查结果。配合 -Json 使用。
.EXAMPLE
    .\scripts\ollama-review.ps1
    .\scripts\ollama-review.ps1 -Model qwen2.5:3b-instruct-q8_0 -Range HEAD~3
    .\scripts\ollama-review.ps1 -Json | ConvertTo-Json -Depth 3
    .\scripts\ollama-review.ps1 -Json -Quiet
    .\scripts\ollama-review.ps1 -NumCtx 4096 -MaxTokens 2048
    .\scripts\ollama-review.ps1 -DiffFile C:\tmp\sample.diff -Model mistral:7b
#>
[CmdletBinding()]
param(
    [string]$Model = 'qwen2.5:7b',
    [string]$Range = 'HEAD~1..HEAD',
    [string]$RepoRoot = (Resolve-Path "$PSScriptRoot/..").Path,
    [string]$RulesFile = "$RepoRoot/AGENTS.md",
    [switch]$Json,
    [switch]$Quiet,
    [string]$OllamaUrl = 'http://localhost:11434',
    [int]$NumCtx = 8192,
    [int]$MaxTokens = 4096,
    [string]$DiffFile = ''
)

$ErrorActionPreference = 'Stop'

function Invoke-OllamaGenerate {
    param([string]$Prompt, [string]$ModelName, [switch]$ForceJson, [int]$NumCtx, [int]$MaxTokens)

    $body = [ordered]@{
        model  = $ModelName
        prompt = $Prompt
        stream = $false
        options = @{
            temperature = 0.1
            num_ctx     = $NumCtx
            num_predict = $MaxTokens
        }
    }
    if ($ForceJson) {
        $body.format = 'json'
    }

    $uri = "$OllamaUrl/api/generate"
    try {
        $resp = Invoke-RestMethod -Uri $uri -Method Post -Body ($body | ConvertTo-Json -Depth 3) -ContentType 'application/json' -TimeoutSec 300
        return $resp.response
    }
    catch {
        throw "调用 Ollama 失败 ($uri, model=$ModelName): $_"
    }
}

function Get-AgentsRules {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        throw "规则文件不存在: $Path"
    }
    $raw = Get-Content -Raw -Path $Path

    # 提取 ## 0. Ponytail 底层认知 和 ## 7. 代码风格与健康规则
    $sections = @()
    foreach ($pattern in @(
        '(?ms)^## 0\.\s+.*?^(?=## 1\.\s)',
        '(?ms)^## 7\.\s+.*?^(?=## 8\.\s)'
    )) {
        $m = [regex]::Match($raw, $pattern)
        if ($m.Success) { $sections += $m.Value.Trim() }
    }

    if ($sections.Count -eq 0) {
        # 兜底：截断前 4000 字符
        return $raw.Substring(0, [Math]::Min(4000, $raw.Length))
    }
    return $sections -join "`n`n---`n`n"
}

# 1. 取 diff
if ($DiffFile) {
    if (-not (Test-Path $DiffFile)) {
        throw "Diff 文件不存在: $DiffFile"
    }
    $diff = Get-Content -Raw -Path $DiffFile
    Write-Verbose "使用 diff 文件: $DiffFile"
}
else {
    $diff = & git -C $RepoRoot diff $Range 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "git diff $Range 失败: $diff"
    }
    if ([string]::IsNullOrWhiteSpace($diff)) {
        if (-not $Quiet) {
            Write-Host "没有可审查的 diff（范围: $Range）" -ForegroundColor Yellow
        }
        exit 0
    }
}

# 2. 读规则
$rules = Get-AgentsRules -Path $RulesFile

# 3. 构造 prompt
$systemPrompt = @"
你是一名严格的 Rust 代码审查员，只根据下面提供的 Clarity AGENTS.md 规则审查 git diff。
禁止 invent 规则；如果 diff 没有明显违规，直接回答"未发现明显违规"。

规则：
$rules

审查要求：
1. 只关注 diff 中新增/修改的代码，未改动代码不要评。
2. 重点检查：
   - 是否有未请求的多余抽象或泛型层（YAGNI）
   - 是否能用 stdlib / 已有依赖一行搞定
   - 是否有冗余代码、死代码、未使用的 feature
   - 临时简化方案是否缺少 // ponytail: 注释说明上限和升级路径
   - 信任边界（路径、MCP 命令、API key、用户输入）是否校验
   - 非平凡纯函数/状态机是否缺少单元测试
   - 是否违反 P1-P7（尤其是 P2 删除优先、P3 单源真相、P6 theme token 强制）
3. 不算违规，不要输出：
   - 常量值、阈值、配置项的合理调整
   - 注释更新、文档补充
   - 测试数据、fixture、mock 值的变更
   - 单纯的格式化、重命名、导入顺序调整
   - 已有文档说明的常量或配置阈值
4. YAGNI 专指「新增但未使用的抽象层」（trait/struct/function/module/feature 开关），不要把常量定义、阈值、枚举值、配置项归入 YAGNI。
5. 每条意见必须具体：指出具体函数/结构体/常量名，不能泛泛而谈。
6. 禁止 invent 不存在的文件、不存在的函数、不存在的规则。
7. 每条意见用中文给出：问题位置（文件:行）、违反规则、最小化修改建议。
8. `line` 必须是问题代码实际出现的源文件行号，不要复制示例中的行号；如果代码中没有对应问题，不要输出。

示例：
- 好 finding：{"file":"crates/foo/src/bar.rs","line":88,"rule":"unwrap_used","severity":"high","description":"read_config 在用户传入路径上直接使用 unwrap()","suggestion":"返回 Result 并用 sanitize_path 校验路径"}
- 不算违规：把 `const MAX_RETRY: u32 = 3;` 改成 5；把 `let model_id = if x { a } else { b };` 写成一行；`.unwrap_or_default()` / `.unwrap_or_else()` / `.unwrap_or(...)`；文档注释更新。
- 差 finding：{"rule":"YAGNI","description":"未请求的多余抽象或泛型层","suggestion":"删除"} 这种没有具体对象的不算。
- 差 finding：{"rule":"YAGNI","description":"McpConfig 结构体未被使用","suggestion":"删除"} 但 McpConfig 只是被导入并作为参数使用，不是新增未使用的抽象，不算。
"@

$fullPrompt = "$systemPrompt`n`n=== GIT DIFF ===`n`n$diff"

if ($Json) {
    $fullPrompt += "`n`n请严格输出 JSON 数组。没有问题时输出 []。`n每条 finding 的字段：file, line, rule, severity(high|medium|low), description, suggestion。`n不要输出 markdown 代码块，不要输出 JSON 以外的任何内容。"
}

# 4. 调用模型
$sourceLabel = if ($DiffFile) { $DiffFile } else { $Range }
if (-not $Quiet) {
    Write-Host "正在用模型 $Model 审查 $sourceLabel 的 diff ..." -ForegroundColor Cyan
}
$response = Invoke-OllamaGenerate -Prompt $fullPrompt -ModelName $Model -ForceJson:$Json -NumCtx $NumCtx -MaxTokens $MaxTokens

# 5. 输出
if ($Json) {
    $text = $response
    # 去掉可能的 markdown 围栏
    $text = [regex]::Replace($text, '(?ms)^\s*```(?:json)?\s*', '')
    $text = [regex]::Replace($text, '(?ms)\s*```\s*$', '')
    $text = $text.Trim()
    if ([string]::IsNullOrWhiteSpace($text)) { $text = '[]' }
    try {
        $parsed = $text | ConvertFrom-Json
        if ($parsed -isnot [array]) { $parsed = @($parsed) }
        $parsed | ConvertTo-Json -Depth 5 -Compress:$Quiet -AsArray
    }
    catch {
        if (-not $Quiet) {
            Write-Warning "模型输出不是合法 JSON，原样输出。"
        }
        $response
    }
}
else {
    $response
}
