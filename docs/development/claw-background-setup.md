# 持久化 Claw 后台与外部设备接入指南

> **Scope**: 在 Windows 本机让 `clarity-gateway` 作为后台服务常驻，并通过 `clarity-claw` 系统托盘节点 + 公网/内网端口让手机等外部设备接入。  
> **验证环境**: Windows 11 + Git Bash + 管理员权限  
> **验证日期**: 2026-07-04

---

## 1. 构建

```bash
# Gateway（ release，已优化）
cargo build -p clarity-gateway --release

# Claw 系统托盘常驻节点（必须显式启用 tray feature）
cargo build -p clarity-claw --features tray --release
```

构建产物：

- `target/release/clarity-gateway.exe`
- `target/release/clarity-claw.exe`

---

## 2. 启动 Gateway

```bash
.\target\release\clarity-gateway.exe
```

默认绑定：

| 地址 | 用途 |
|------|------|
| `0.0.0.0:18790` | 公共 API + `/ws` + `/openclaw/ws` |
| `127.0.0.1:18800` | Admin UI / API（仅本机） |

Gateway 启动后会自动在当前工作目录创建 `.clarity/` 运行时数据目录，包括：

- `.clarity/sessions.db`
- `.clarity/role_context.db`
- `.clarity/openclaw-admin-token`（OpenClaw 管理 token）

---

## 3. 启动 Claw 系统托盘节点

```bash
.\target\release\clarity-claw.exe
```

行为：

1. 单实例守卫：若已有实例在运行则直接退出。
2. 通过 `CLARITY_GATEWAY_URL` 或默认值 `http://127.0.0.1:18790` 连接 Gateway。
3. 使用 WebSocket `register_device` 注册本机设备。
4. 每 30 秒发送心跳。
5. 进入系统托盘事件循环，监听 `.clarity/tasks` 文件变更。

验证设备已注册：

```bash
curl -s http://127.0.0.1:18790/api/v1/claw/devices
```

示例输出：

```json
[{
  "id": "claw-ROG-X",
  "name": "ROG-X",
  "host": "ROG-X",
  "version": "0.4.0",
  "status": "online",
  "last_heartbeat": "2026-07-04T13:04:49.294074100+00:00"
}]
```

---

## 4. 开放外部设备接入

### 4.1 确认本机 IP

```bash
ipconfig
```

常见可用地址：

- 同一 Wi-Fi：`10.3.118.208`
- Tailscale 虚拟网：`100.107.247.38`

### 4.2 防火墙放行

PowerShell / CMD（管理员）：

```powershell
netsh advfirewall firewall add rule name="Clarity Gateway Public API" dir=in action=allow protocol=tcp localport=18790 profile=any
```

验证规则：

```powershell
netsh advfirewall firewall show rule name="Clarity Gateway Public API"
```

> 注意：`:18800` 只绑定 `127.0.0.1`，无需、也不应暴露给外部。

### 4.3 外部设备连接方式

| 场景 | 连接地址 | 前提 |
|------|----------|------|
| 同一局域网手机 | `ws://<Wi-Fi IP>:18790/ws` | 防火墙已放行，手机和 PC 同子网 |
| 跨网络（推荐） | `ws://<Tailscale IP>:18790/ws` | 手机安装 Tailscale 并加入同一 tailnet |
| OpenClaw 兼容客户端 | `ws://<IP>:18790/openclaw/ws` | 使用 `.clarity/openclaw-admin-token` 作为 admin token |

---

## 5. 量化连通性测试

已提供脚本 `scripts/test_claw_connectivity.py`：

```bash
python -m venv .venv
.venv\Scripts\pip install websockets
.venv\Scripts\python scripts/test_claw_connectivity.py
```

测试覆盖：

1. HTTP `/health`
2. HTTP `/api/v1/claw/devices`
3. 原生 Gateway WebSocket `/ws`（welcome / ping / register_device）
4. OpenClaw WebSocket `/openclaw/ws`（challenge → connect → chat.send）

2026-07-04 实测结果：

```text
[1/4] HTTP /health                status=200 elapsed=15.0ms
[2/4] HTTP /api/v1/claw/devices   status=200 elapsed=9.3ms
[3/4] Native Gateway WebSocket /ws  elapsed=16.6ms
[4/4] OpenClaw WebSocket /openclaw/ws  elapsed=8311.6ms  hello.ok=True  chat_reply.ok=False
```

第 4 步 `chat.send` 因当前没有配置可用 LLM provider 而返回 `AGENT_ERROR`，但协议握手、请求-响应帧格式均正常。

---

## 6. 当前已知缺口

| 缺口 | 影响 | 建议下一步 |
|------|------|-----------|
| **未配置 LLM provider** | `chat.send` 运行时返回 `401 Unauthorized` | 配置 `models.toml` / 环境变量 `DEEPSEEK_API_KEY` 等，或启用本地 GGUF 模型 |
| **SQLite 持久化 fallback** | Gateway 启动时 `sessions.db` 与 `role_context.db` 初始化报错 `Execute returned results`，退回到内存存储 | 排查 `PersistentSessionStore::init_schema` 与 `RoleContextStore::init_schema` 中 `rusqlite::Connection::execute` 的使用；可能是 bundled SQLite 对某条语句返回了结果集 |
| **无 Windows 服务/systemd 包装** | 用户注销后 Gateway/Claw 会退出 | 用 `sc.exe` 或 `nssm` 注册为 Windows Service，或配合任务计划程序开机启动 |
| **OpenClaw streaming 未实现** | `/openclaw/ws` 当前只返回最终 `Res` 帧 | 如需 KimiClaw 式流式体验，需把 `OpenClawServerTransport` 的 `ChatChunk`/`Done` 事件转为 server-sent `chat` event |
| **无 TLS** | 跨公网传输 admin token 和设备 token 为明文 | 同一 Tailscale 内可缓解；公网部署需前置反向代理 + HTTPS |
| **没有真实手机客户端** | 无法端到端验证移动端 UI | 先用 `scripts/test_claw_connectivity.py` 模拟，再让手机浏览器访问 `http://<IP>:18790/chat.html` 做最小验证 |

---

## 7. 推荐下一步（按优先级）

1. **修复 SQLite 初始化错误**，确保 Gateway 重启后会话与角色上下文不丢失。
2. **配置可用 LLM provider**，让 `/openclaw/ws` 的 `chat.send` 真正产生 assistant 回复。
3. **把 Gateway 注册为 Windows 服务**，实现开机自启、后台常驻。
4. **为 `clarity-claw` 增加 `--headless` 模式**（不启动 tray 图标），方便作为服务运行。
5. **移动端最小验证**：手机浏览器打开 `http://<本机IP>:18790/chat.html`，确认 Web IDE 能加载并走 WebSocket。

---

## 8. 参考

- `docs/architecture/claw-protocol.md`
- `crates/clarity-gateway/src/ws.rs`
- `crates/clarity-gateway/src/openclaw_server/handler.rs`
- `crates/clarity-claw/src/main.rs`
- `scripts/test_claw_connectivity.py`
