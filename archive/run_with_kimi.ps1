# Clarity with Kimi Code 启动脚本
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-7wIafvpXmFAZAdBwBHsCQyXaPJ0zQrGbETdKgOjnhQdtXfkbRh2zayGqkFAeAvTz"
$env:ANTHROPIC_MODEL="kimi-for-coding"

Write-Host "🚀 Starting Clarity TUI with Kimi Code..." -ForegroundColor Green
Write-Host "Base URL: $env:ANTHROPIC_BASE_URL"
Write-Host "Model: $env:ANTHROPIC_MODEL"
Write-Host ""

cargo run -p clarity-tui
