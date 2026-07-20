# Claw 协议策略

> **Status**: 已决策 / 实施中（v0.4.0-rc）  
> **Scope**: `clarity-claw`、`clarity-gateway`、前端 Claw 连接  
> **Decision**: Gateway WebSocket 是 Clarity 内部唯一协议；OpenClaw JSON-RPC 仅作为与外部 KimiClaw/OpenClaw Gateway 互通的 fallback，同时 `clarity-gateway` 自身也提供 OpenClaw 兼容端点作为 Kimi Desktop 删除后的本地 fallback。  
> **Date**: 2026-06-20 / 2026-07-06 更新

---

## 0. 最新进展（2026-07-06）

治理结论已落地并经过量化验证：

- **协议单头**：Gateway WebSocket 是 Clarity 内部唯一协议；OpenClaw JSON-RPC 仅作为外部 KimiClaw / OpenClaw Gateway 互通的 fallback。Hermes 是可选记忆后端，不参与协议层。
- **原生 Gateway `/ws` 回路修复**：`GatewayWebSocketTransport` 改为通过 `AgentController` + `ConversationChatDriver` 跑流式 turn，避免直接调用 `agent.run_streaming()` 时在 Gateway 上下文因记忆检索挂起；新增 60s 超时保护；`TransportEvent::Done` 现在映射为 `WsResponse::Done` 并发送给客户端，保证移动端能收到 `TurnEnd`。
- **服务端双端点统一**：`clarity-gateway` 的 `/ws` 与 `/openclaw/ws` 共享同一 `ClawTransport` 适配层（`crates/clarity-gateway/src/transports`），`chat.send` / `chat.history` / `role_context.sync` 逻辑不再重复。
- **关键缺陷修复**：`clarity-core::agent::run` 的同步路径在 `run_sync_loop` 失败时未调用 `finish_turn()`，会导致 Agent 卡在 `Running` 状态并引发后续请求的 "Agent is already running a turn" 错误。已在 `crates/clarity-core/src/agent/run.rs` 修复，与 streaming 路径保持一致。
- **模型选择纠偏**：原 `scripts/benchmark_ollama.py` 仅测量裸文本生成速度，推荐 `llama3.2:1b`；但实际 Agent 循环包含 system prompt + 工具调用，1B 模型会陷入工具幻觉循环并在 45s 超时。新增 `scripts/benchmark_ollama_agentic.py` 在真实 Agent 载荷下评测，结果 `qwen2.5:7b` 准确率 100% / 延迟 436ms，为 OpenClaw 实际可用的最优本地模型。当前 `.clarity/active_alias.json` 已切到 `qwen2.5-7b`。
- **端到端验证**：
  - `python scripts/test_claw_connectivity.py` 通过（OpenClaw chat.send 约 2.9–4.2s）
  - `python scripts/test_reconnect.py` 通过（重连后 session history 可复用）
  - 原生 `/ws` 回路验证通过：`scripts/test_gateway_ws_chat.py` 收到 welcome、流式 chat 分片与 `{"type":"done"}` 结束帧
- **Admin 认证 test isolation**：将 `admin_auth` 中间件从全局环境变量改为读取 `AppState.admin_token`，消除 `CLARITY_ADMIN_TOKEN` 环境变量导致 gateway 单元测试 401 失败的问题。

---

## 1. 背景

Clarity 早期同时存在两种「Claw」连接方言：

| 方言 | 协议 | 代表端点 | 来源 |
|------|------|----------|------|
| **ZeroClaw / Gateway WebSocket** | 原生 Clarity Gateway WebSocket | `clarity-gateway` `:18790` | Clarity 自己实现 |
| **OpenClaw** | JSON-RPC over WebSocket | 外部 KimiClaw / OpenClaw Gateway `:18789` | 外部生态遗产 |

两者在 egui 中曾被要求提供**完全一致**的体验，导致：
- `clarity-claw` 库同时维护 `GatewayClient` + `ClawClient` 两套 parser
- UI 层需要知道 `chat.send` vs `sessions.send` 的区别
- 协议细节（如 `use_sessions_send`）泄漏到 `App` 状态

这不是设计需要，是历史遗产。

---

## 2. 决策

**Clarity 内部 mesh 只使用一种协议：Gateway WebSocket。**

```text
clarity-egui ──┐
clarity-tui  ──┼──► clarity-gateway ──► clarity-core / clarity-wire
clarity-claw ──┘         │
                          │ Gateway WebSocket（唯一内部协议）
                          │
                          ▼
                ws://127.0.0.1:18790/openclaw/ws
                          │
              外部 KimiClaw / OpenClaw Gateway（可选外部 fallback）
```

- `clarity-claw` ↔ `clarity-gateway`：Gateway WebSocket
- `clarity-egui` ↔ 任意后端：`ClawConnectionManager` 自动探测 dialect，但**发送方法由 dialect 决定**，UI 不再指定
- Hermes 是 `clarity-memory` 的可选 SQLite 后端（`hermes` feature），与 Claw 协议层无关；不存在 "Hermes 协议路线"。
- OpenClaw JSON-RPC 仅用于两种外部兼容场景：
  1. 连接外部 Kimi/OpenClaw Gateway（历史兼容）。
  2. 连接 `clarity-gateway` 自身的 OpenClaw 兼容端点 `ws://127.0.0.1:18790/openclaw/ws`，在 Kimi Desktop 被移除后由 Clarity 独立承担 Claw 功能。

---

## 3. 两种方言的职责边界

### 3.1 Gateway WebSocket（一等公民）

用于：
- 本地 `clarity-claw` 与 `clarity-gateway` 通信
- 前端与本地 Gateway 通信
- 角色上下文同步、多设备存活节点、白名单路由、审批流

语义：
- `welcome` 首帧
- `chat.send`（`sessionKey`）
- `WireMessage` 直传
- `RoleContextSynced` 事件

### 3.2 OpenClaw JSON-RPC（外部/本地 fallback 适配层）

用于：
- 连接外部 KimiClaw / OpenClaw Gateway（历史兼容）。
- 连接 `clarity-gateway` 内建的 OpenClaw 兼容端点，作为 Kimi Desktop 删除后的本地 fallback。
- 基本聊天、历史、会话管理、设备配对。

**不用于**：
- Clarity 内部 mesh 通信（仍走 Gateway WebSocket）。
- 角色上下文同步（由 Syncthing-rust 覆盖离线场景）。
- 审批、WireMessage、MCP tool events 等 Clarity 内部语义。

语义：
- `connect` / `connect.challenge`
- `chat.send`（`sessionKey`，KimiClaw/ACP 风格）
- `sessions.send`（`key`，通用 OpenClaw 风格）
- `sessions.list` / `sessions.preview` / `sessions.reset/delete/compact`
- `device.pair.request` / `device.pair.list`
- `session.message` / `chat` / `agent` 事件

### 3.3 `clarity-gateway` OpenClaw 兼容端点

`clarity-gateway` 在公共 API 路由上暴露 `GET /openclaw/ws`：

- 地址：`ws://127.0.0.1:18790/openclaw/ws`
- 认证：admin token（持久化在 `.clarity/openclaw-admin-token`）或已配对 device proof。
- 实现：复用 `clarity-contract::openclaw_protocol` 中的共享协议类型；`chat.send` 直接调用 Gateway 共享的 `Agent`。
- 目的：当 Kimi Desktop 被删除后，`clarity-headless acp-bridge` 和 `clarity-claw` 仍可发现并使用该端点，使 Clarity 独立承担 Claw 功能。

---

## 4. 代码影响

### 4.1 `clarity-claw`（统一客户端节点）

- `ClawConnectionManager` 保留自动探测能力
- `ProtocolCommand::Chat` 不再携带 `use_sessions_send`
- Gateway 管理器固定使用 `chat.send`
- OpenClaw 管理器固定使用 `sessions.send`
- OpenClaw 翻译层只保留基本聊天/历史/错误/配对映射，逐步移除对 Gateway 内部语义的模仿
- 合并后 `clarity-claw` 既是 UI 无关的客户端库，也是系统托盘常驻二进制

### 4.2 前端

- 删除 `App.claw_ws_uses_sessions_send` 等协议泄漏字段
- `ClawClientHandle::send_chat` 只接受 `session_key` + `message`
- UI 不再根据协议选择发送方法

### 4.3 `clarity-claw`

- **只做 Gateway WebSocket 客户端**；它是 Clarity 内部 mesh 的系统托盘常驻节点
- 不与外部 KimiClaw/OpenClaw Gateway 直接交互
- 不持有 OpenClaw JSON-RPC fallback
- 外部互通由 `clarity-claw` 库内的 OpenClaw/KimiClaw 兼容层负责协议转换
- 当前任务/线程轮询仍走 Gateway HTTP admin 端点；聊天与角色上下文同步走 Gateway WebSocket

### 4.4 `clarity-gateway::transports`（服务端适配层）

为消除 `/ws` 与 `/openclaw/ws` 的重复协议逻辑，`clarity-gateway` 新增服务端 `ClawTransport` 适配器：

- `GatewayWebSocketTransport`：实现于 `crates/clarity-gateway/src/transports/gateway_ws.rs`，将原生 Gateway `/ws` 的流式聊天、历史查询、角色上下文同步统一适配到 `ClawTransport`。
- `OpenClawServerTransport`：实现于 `crates/clarity-gateway/src/transports/openclaw.rs`，为 `/openclaw/ws` JSON-RPC 端点提供同样的聊天/历史/同步能力，同时负责把 transport 事件转换为 OpenClaw JSON-RPC 帧。
- 共享转换函数位于 `crates/clarity-gateway/src/transports/common.rs`，包括 `WsRequest` ↔ `MessageContext`、`TransportEvent` ↔ `WsResponse`、`TransportEvent` ↔ `OpenClawFrame`、会话消息 ↔ `HistoryMessage`。
- 两个服务端 adapter 均被 `GovernedTransport` 包装，复用 Gateway 全局 `ConnectionMetrics` 并输出统一审计日志。

这样，`/ws` 的 `handle_chat_with_wire` 与 `/openclaw/ws` 的 `handle_chat_send` / `handle_chat_history` 不再直接操作 `Agent`，而是通过同一套 `ClawTransport` 接口驱动，后续协议升级只需修改 adapter 和转换函数。

---

## 5. 未来工作

1. **合并 parser**：长期目标是把 `GatewayClient` 与 `ClawClient` 统一为单套实现，OpenClaw 仅保留一个薄翻译层。
2. **协议协商**：Gateway 可在握手时通过 `Sec-WebSocket-Protocol` 声明 `clarity-v1`，OpenClaw 端保持现有行为。
3. **外部互通**：若外部 OpenClaw Gateway 需要接入 Clarity mesh，由 `clarity-claw` 库内的 OpenClaw/KimiClaw 兼容层提供协议转换，而非让 mesh 迁就 OpenClaw 语义。
4. **OpenClaw server 能力补齐**：`chat.send` 与 `chat.history` 已通过 `OpenClawServerTransport` 接入 `session_store`；`sessions.*` 仍为最小实现，后续可按需接入 `thread_store` / `session_store` 实现完整会话生命周期。

---

## 6. 参考

- `crates/clarity-claw/src/connection_manager.rs`
- `crates/clarity-egui/src/claw.rs`
- `crates/clarity-egui/src/panels/right_ide_panel/claw_settings_panel.rs`
- `docs/architecture/protocol-layer.md`
