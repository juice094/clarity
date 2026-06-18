# Gray-Cloud (OpenClaw) 连接验证清单

本清单用于验证 `clarity-egui` 移除硬编码凭据后，能否通过环境变量正确连接到远程 Gray-Cloud OpenClaw Gateway。

## 前提

- Gray-Cloud 服务端 OpenClaw Gateway 已启动并监听目标地址/端口。
- ROG-X 上的新 token 已生效，旧 token 已作废。

## 环境变量

在运行验证的 PowerShell 中设置：

```powershell
$env:OPENCLAW_REMOTE_URL="ws://<gray-cloud-tailscale-ip>:18789"
$env:OPENCLAW_REMOTE_TOKEN="<new-token>"
```

> 协议建议直接用 `ws://` 或 `wss://`。若写 `http://`/`https://`，代码会自动转换。

## L1 — 网络层可达性

```powershell
Test-NetConnection -ComputerName <gray-cloud-tailscale-ip> -Port 18789
```

- [ ] TCP 测试成功 (`TcpTestSucceeded : True`)
- [ ] 若失败，检查 Tailscale、防火墙、Gray-Cloud Gateway 进程

## L2 — 协议层握手

运行项目提供的验证脚本：

```powershell
cd C:\Users\22414\dev\clarity
.\scripts\test-openclaw-connection.ps1
```

- [ ] 环境变量检查通过
- [ ] TCP 端口可达
- [ ] WebSocket 连接建立
- [ ] `connect` JSON-RPC 返回 `ok: true`

### 预期输出

```text
[+] L0: 检查环境变量
    OK: OPENCLAW_REMOTE_URL = ws://...
    OK: OPENCLAW_REMOTE_TOKEN 已设置 (长度: ...)
[+] L1: TCP 端口可达性探测 (...)
    OK: TCP 端口可达。
[+] L2: WebSocket JSON-RPC connect 握手
    OK: WebSocket 连接已建立。
    OK: 已发送 connect 请求。
    INFO: 收到响应: {"type":"res","ok":true,...}
    OK: OpenClaw 认证成功。
[+] 验证结果
    OK: Gray-Cloud (OpenClaw) 连接验证全部通过。
```

## L3 — GUI 集成验证

在同一 PowerShell 中启动 egui：

```powershell
cargo run -p clarity-egui
```

- [ ] 启动日志中 `Gray-Cloud (OpenClaw)` 设备已被注册
- [ ] UI 中选择 `Gray-Cloud (OpenClaw)`
- [ ] 右栏 `Claw Terminal` 面板触发 WebSocket 连接
- [ ] 日志/Toast 出现 `Connected to Claw Gateway: ws://...`
- [ ] 自动拉取历史，Toast 显示 `Loaded N messages from session`
- [ ] 在 Terminal 输入框发送测试命令，能收到 Gray-Cloud 响应

### 观察日志位置

- 终端标准输出（带 `clarity_egui` 前缀）
- `C:/Users/22414/.clarity/logs/` 下的运行日志

## L4 — 边界与异常验证

### L4.1 错误 token

```powershell
$env:OPENCLAW_REMOTE_TOKEN="wrong-token"
cargo run -p clarity-egui
```

- [ ] UI 收到 `Auth failed` 提示
- [ ] 应用不崩溃

### L4.2 网络不可达

```powershell
$env:OPENCLAW_REMOTE_URL="ws://192.0.2.1:18789"
cargo run -p clarity-egui
```

- [ ] UI 提示 `WebSocket connect` 错误
- [ ] 应用不崩溃

### L4.3 空 token

```powershell
Remove-Item Env:\OPENCLAW_REMOTE_TOKEN
cargo run -p clarity-egui
```

- [ ] `Gray-Cloud (OpenClaw)` 设备状态显示 `Offline`
- [ ] 不会发起 WebSocket 连接

## 结果分类

| 类别 | 现象 | 下一步 |
|------|------|--------|
| A. 完全可用 | L1-L4 全部通过 | 进入功能完善：实时状态指示 + 聊天消息路由 |
| B. 网络可达但协议失败 | L1 通过，L2/L3 握手失败 | 对比 Gray-Cloud 实际协议修复 JSON-RPC 格式 |
| C. 网络不可达 | L1 失败 | 检查 Tailscale/IP/防火墙/Gateway 进程 |
| D. 认证失败 | L2 返回 auth error | 确认 token 是否已在 ROG-X 环境变量生效 |

## 反馈模板

请把验证结果复制到对话中：

```text
验证类别：A / B / C / D
L1 结果：通过 / 失败（原因）
L2 结果：通过 / 失败（日志摘要）
L3 结果：通过 / 失败（现象）
L4 结果：通过 / 失败（边界情况）
```
