# Agent 指引 — clarity-claw

## 概述

`clarity-claw` 是 Clarity 分布式节点（Claw）的 client-side 统一入口：

- **库**：UI 无关的 Claw 客户端（Gateway WebSocket、OpenClaw/KimiClaw 兼容、设备发现、配对、角色上下文同步）。
- **二进制**：系统托盘常驻节点。

Server-side 对应物是 `clarity-gateway`；共享契约见 `clarity-contract::claw_context` 与 `clarity-contract::federation`。

## 构建

```bash
# 库默认 feature
cargo check -p clarity-claw --lib

# 系统托盘二进制
cargo check -p clarity-claw --bin clarity-claw --features tray

# 带 mesh 同步
cargo check -p clarity-claw --lib --features mesh
```

## 测试

```bash
cargo test -p clarity-claw --lib
cargo test -p clarity-claw --bin clarity-claw --features tray
```

## 关键文件

- `src/lib.rs` — 客户端库公共 API 与 Gateway 交互辅助函数
- `src/client.rs` — OpenClaw JSON-RPC 客户端
- `src/connection_manager.rs` — 协议方言自动探测与管理器
- `src/gateway_client.rs` — Gateway WebSocket 原生客户端
- `src/discovery.rs` — 本地/远程 Gateway 发现
- `src/device.rs` — Ed25519 设备身份与配对 token
- `src/acp_bridge.rs` — Kimi ACP 云桥：将云端消息转发到本地 Clarity Gateway 或 OpenClaw Gateway
- `src/openclaw_gateway/` — Kimi Desktop OpenClaw Gateway JSON-RPC 客户端
  - `client.rs` — 带自动重连的 WebSocket 客户端与握手
  - `chat.rs` — `chat.send` / `chat.history` / `chat.abort`
  - `session.rs` — 会话管理
  - `device.rs` — 设备配对 API
  - `kimi_file.rs` — Kimi 文件下载
  - `protocol.rs` / `types.rs` — 帧协议与数据类型
- `src/mesh_client.rs` / `src/mesh/` — 角色上下文离线同步（`mesh` feature）
- `src/tray/mod.rs` — 系统托盘事件循环（`tray` feature）
- `src/main.rs` — 托盘二进制入口

## 约定

- 错误处理使用 `anyhow`。
- 异步使用 `tokio`。
- 所有 `pub` 项必须带 `///` 文档注释（workspace `missing_docs = "deny"`）。
- 禁止新增 `unsafe`、`unwrap`、`expect`、`panic`；例外需加 `// SAFE:` 或同等说明。
- **库层面禁止依赖 `clarity-core` / `clarity-wire`**：
  - 这两个 crate 仅通过 `tray` feature 供二进制目标使用。
  - 新增模块时应自查是否无意中引入了它们。
- 前端依赖本 crate 时必须使用 `default-features = false`，按需启用 `mesh` 等功能，避免拖入 `tao`/`tray-icon`/`notify-rust` 等托盘 GUI 依赖。
