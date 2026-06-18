#Requires -Version 5.1
<#
.SYNOPSIS
    验证 Clarity -> Gray-Cloud (OpenClaw) 远程连接。

.DESCRIPTION
    该脚本执行分层验证：
    1. 检查 OPENCLAW_REMOTE_URL / OPENCLAW_REMOTE_TOKEN 环境变量。
    2. TCP 端口可达性探测。
    3. WebSocket + JSON-RPC connect 握手验证。

.EXAMPLE
    $env:OPENCLAW_REMOTE_URL="ws://<tailscale-ip>:18789"
    $env:OPENCLAW_REMOTE_TOKEN="<token>"
    .\scripts\test-openclaw-connection.ps1
#>

[CmdletBinding()]
param()

function Write-Step {
    param([string]$Message)
    Write-Host "`n[+] $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host "    OK: $Message" -ForegroundColor Green
}

function Write-Fail {
    param([string]$Message)
    Write-Host "    FAIL: $Message" -ForegroundColor Red
}

function Write-Info {
    param([string]$Message)
    Write-Host "    INFO: $Message" -ForegroundColor Yellow
}

# ── L0: 环境变量检查 ─────────────────────────────────────────────────────
Write-Step "L0: 检查环境变量"

$remoteUrl = $env:OPENCLAW_REMOTE_URL
$remoteToken = $env:OPENCLAW_REMOTE_TOKEN

if ([string]::IsNullOrWhiteSpace($remoteUrl)) {
    Write-Fail "OPENCLAW_REMOTE_URL 未设置。"
    Write-Info "请先执行: `$env:OPENCLAW_REMOTE_URL=`"ws://<gray-cloud-ip>:18789`""
    exit 1
}
Write-Ok "OPENCLAW_REMOTE_URL = $remoteUrl"

if ([string]::IsNullOrWhiteSpace($remoteToken)) {
    Write-Fail "OPENCLAW_REMOTE_TOKEN 未设置。"
    Write-Info "请先执行: `$env:OPENCLAW_REMOTE_TOKEN=`"<your-token>`""
    exit 1
}
Write-Ok "OPENCLAW_REMOTE_TOKEN 已设置 (长度: $($remoteToken.Length))"

# 规范化 URL
$wsUrl = $remoteUrl
if ($wsUrl -match "^https://") {
    $wsUrl = $wsUrl -replace "^https://", "wss://"
} elseif ($wsUrl -match "^http://") {
    $wsUrl = $wsUrl -replace "^http://", "ws://"
}

if (-not ($wsUrl -match "^(ws|wss)://")) {
    Write-Fail "URL 协议必须是 ws:// 或 wss:// (或 http/https，会自动转换)。"
    exit 1
}
Write-Info "规范化后的 WebSocket URL: $wsUrl"

# 解析 host/port
$uri = [System.Uri]$wsUrl
$hostName = $uri.Host
$port = $uri.Port

# ── L1: TCP 可达性 ───────────────────────────────────────────────────────
Write-Step "L1: TCP 端口可达性探测 ($hostName`:$port)"

try {
    $tcpClient = New-Object System.Net.Sockets.TcpClient
    $connectTask = $tcpClient.ConnectAsync($hostName, $port)
    $timeout = [System.TimeSpan]::FromSeconds(5)
    if (-not $connectTask.Wait($timeout)) {
        Write-Fail "TCP 连接超时。"
        exit 1
    }
    if (-not $tcpClient.Connected) {
        Write-Fail "TCP 连接失败。"
        exit 1
    }
    Write-Ok "TCP 端口可达。"
    $tcpClient.Close()
} catch {
    Write-Fail "TCP 探测异常: $_"
    exit 1
}

# ── L2: WebSocket + JSON-RPC 握手 ────────────────────────────────────────
Write-Step "L2: WebSocket JSON-RPC connect 握手"

$client = New-Object System.Net.WebSockets.ClientWebSocket
$cts = New-Object System.Threading.CancellationTokenSource
$ct = $cts.Token

try {
    $connectTask = $client.ConnectAsync([System.Uri]$wsUrl, $ct)
    $connectTask.Wait()
    Write-Ok "WebSocket 连接已建立。"

    $connectReq = @{
        type = "req"
        method = "connect"
        params = @{
            role = "operator"
            auth = @{ token = $remoteToken }
        }
    } | ConvertTo-Json -Depth 10 -Compress

    $bytes = [System.Text.Encoding]::UTF8.GetBytes($connectReq)
    $segment = New-Object System.ArraySegment[byte](, $bytes)
    $sendTask = $client.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $ct)
    $sendTask.Wait()
    Write-Ok "已发送 connect 请求。"

    $buffer = New-Object byte[] 4096
    $seg = New-Object System.ArraySegment[byte](, $buffer)
    $recvTask = $client.ReceiveAsync($seg, $ct)
    if (-not $recvTask.Wait([System.TimeSpan]::FromSeconds(10))) {
        Write-Fail "等待 connect 响应超时。"
        $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "timeout", $ct).Wait()
        exit 1
    }

    $result = $recvTask.Result
    $text = [System.Text.Encoding]::UTF8.GetString($buffer, 0, $result.Count)
    Write-Info "收到响应: $text"

    try {
        $json = $text | ConvertFrom-Json
        if ($json.ok -eq $true) {
            Write-Ok "OpenClaw 认证成功。"
        } else {
            $errorMsg = $json.error
            if ([string]::IsNullOrWhiteSpace($errorMsg)) { $errorMsg = "unknown auth error" }
            Write-Fail "认证失败: $errorMsg"
            exit 1
        }
    } catch {
        Write-Fail "响应不是合法 JSON: $_"
        exit 1
    }

    $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "done", $ct).Wait()
} catch {
    Write-Fail "WebSocket 握手异常: $_"
    exit 1
} finally {
    $client.Dispose()
    $cts.Dispose()
}

Write-Step "验证结果"
Write-Ok "Gray-Cloud (OpenClaw) 连接验证全部通过。可以继续 L3 GUI 集成验证。"
