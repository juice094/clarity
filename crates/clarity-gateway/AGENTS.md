# Agent 指引 — clarity-gateway

## 构建

```bash
cargo build -p clarity-gateway
```

## 测试

```bash
cargo test -p clarity-gateway --lib
```

## 关键文件

- `src/lib.rs` — 模块声明
- `src/server.rs` — 双端口服务器、`AppState`、路由构造器
- `src/handlers/mod.rs` — 所有 Axum handler（chat、tasks、admin、files、mcp、claw、anthropic）
- `src/handlers/mcp.rs` — MCP 服务器管理 handler
- `src/handlers/claw.rs` — Claw 设备注册/心跳/列表 handler
- `src/handlers/claw_sync.rs` — 角色上下文同步端点
- `src/handlers/anthropic.rs` — Anthropic Messages API 兼容端点（`anthropic-api` feature）
- `src/session_store.rs` — SQLite 持久化会话存储
- `src/role_context_store.rs` — 角色上下文事件持久化存储
- `src/ws.rs` — WebSocket handler（含 Claw 协议消息）
- `static/` — 内嵌 Web UI 静态文件

## 约定

- 错误处理使用 `AgentError` / `ToolError`
- 异步使用 `tokio`
- 共享状态通过 `Arc<AppState>` 注入 handler
- Admin 端口（18800）仅限本地回环，可配置 `CLARITY_ADMIN_TOKEN` 认证
- MCP 配置路径可通过 `CLARITY_MCP_CONFIG_PATH` 覆盖，否则使用平台默认路径
