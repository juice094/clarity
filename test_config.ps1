# Clarity 配置验证脚本
# 测试环境变量配置是否正确

Write-Host "🧪 Project Clarity 配置验证" -ForegroundColor Cyan
Write-Host "==============================`n"

function Mask-Key($key) {
    if (-not $key) { return "未设置" }
    if ($key.Length -le 15) { return "***" }
    return $key.Substring(0, 15) + "..."
}

function Get-OrDefault($value, $default) {
    if ($value) { return $value }
    return $default
}

# 检查环境变量
$config = @{}

# 检测 Kimi Code (Claude Code 风格)
if ($env:ANTHROPIC_BASE_URL -or $env:ANTHROPIC_AUTH_TOKEN) {
    Write-Host "✅ 检测到 Claude Code 风格配置 (Kimi Code)" -ForegroundColor Green
    $config['type'] = 'Kimi Code'
    $config['base_url'] = Get-OrDefault $env:ANTHROPIC_BASE_URL "未设置"
    $config['auth_token'] = Mask-Key $env:ANTHROPIC_AUTH_TOKEN
    $config['model'] = Get-OrDefault $env:ANTHROPIC_MODEL "kimi-for-coding (默认)"
}
# 检测 Kimi API (Clarity 风格)
elseif ($env:KIMI_API_KEY -or $env:KIMI_BASE_URL) {
    Write-Host "✅ 检测到 Clarity 风格配置 (Kimi API)" -ForegroundColor Green
    $config['type'] = 'Kimi API'
    $config['base_url'] = Get-OrDefault $env:KIMI_BASE_URL "https://api.moonshot.cn/v1 (默认)"
    $config['api_key'] = Mask-Key $env:KIMI_API_KEY
    $config['model'] = Get-OrDefault $env:KIMI_MODEL "moonshot-v1-8k (默认)"
}
# 检测 OpenAI
elseif ($env:OPENAI_API_KEY) {
    Write-Host "✅ 检测到 OpenAI 配置" -ForegroundColor Green
    $config['type'] = 'OpenAI'
    $config['base_url'] = Get-OrDefault $env:OPENAI_BASE_URL "https://api.openai.com/v1 (默认)"
    $config['api_key'] = Mask-Key $env:OPENAI_API_KEY
    $config['model'] = Get-OrDefault $env:OPENAI_MODEL "gpt-3.5-turbo (默认)"
}
else {
    Write-Host "❌ 未检测到任何 LLM 配置" -ForegroundColor Red
    Write-Host "`n可用配置选项：" -ForegroundColor Yellow
    Write-Host "1. Kimi Code (推荐):"
    Write-Host "   `$env:ANTHROPIC_BASE_URL='https://api.kimi.com/coding/'"
    Write-Host "   `$env:ANTHROPIC_AUTH_TOKEN='sk-kimi-your-key'"
    Write-Host "`n2. Kimi API:"
    Write-Host "   `$env:KIMI_API_KEY='sk-your-key'"
    Write-Host "   `$env:KIMI_BASE_URL='https://api.moonshot.cn/v1'"
    Write-Host "`n3. OpenAI:"
    Write-Host "   `$env:OPENAI_API_KEY='sk-...'"
    exit 1
}

# 显示配置详情
Write-Host "`n📋 配置详情:" -ForegroundColor Cyan
$config.GetEnumerator() | ForEach-Object {
    Write-Host "   $($_.Key): $($_.Value)"
}

# 协议检测
$baseUrl = $config['base_url']
if ($baseUrl -match "kimi.com/coding" -or $baseUrl -match "anthropic") {
    Write-Host "`n🔌 协议: Anthropic (/v1/messages)" -ForegroundColor Blue
} else {
    Write-Host "`n🔌 协议: OpenAI (/v1/chat/completions)" -ForegroundColor Blue
}

# 检查 Ollama
Write-Host "`n📡 Ollama 检测:" -ForegroundColor Cyan
try {
    $response = Invoke-WebRequest -Uri "http://localhost:11434" -Method GET -TimeoutSec 2 -ErrorAction SilentlyContinue
    Write-Host "   ✅ Ollama 运行中" -ForegroundColor Green
} catch {
    Write-Host "   ℹ️  Ollama 未运行（如需要使用本地模型，请先启动 ollama）" -ForegroundColor Gray
}

Write-Host "`n✨ 配置验证完成!" -ForegroundColor Green
Write-Host "运行示例: cargo run --example claude_code_compat" -ForegroundColor Yellow
