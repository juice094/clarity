# Claw 协议策略

> **Status**: 已决策 / 实施中（v0.3.4-rc）  
> **Scope**: `clarity-claw`、`clarity-gateway`、`clarity-openclaw`、前端 Claw 连接  
> **Decision**: Gateway WebSocket 是 Clarity 内部唯一协议；OpenClaw JSON-RPC 仅作为与外部 KimiClaw/OpenClaw Gateway 互通的 fallback。  
> **Date**: 2026-06-20

---

## 1. 背景

Clarity 早期同时存在两种「Claw」连接方言：

| 方言 | 协议 | 代表端点 | 来源 |
|------|------|----------|------|
| **ZeroClaw / Gateway WebSocket** | 原生 Clarity Gateway WebSocket | `clarity-gateway` `:18790` | Clarity 自己实现 |
| **OpenClaw** | JSON-RPC over WebSocket | 外部 KimiClaw / OpenClaw Gateway `:18789` | 外部生态遗产 |

两者在 egui 中曾被要求提供**完全一致**的体验，导致：
- `clarity-openclaw` 同时维护 `GatewayClient` + `ClawClient` 两套 parser
- UI 层需要知道 `chat.send` vs `sessions.send` 的区别
- 协议细节（如 `use_sessions_send`）泄漏到 `App` 状态

这不是设计需要，是历史遗产。

---

## 2. 决策

**Clarity 内部 mesh 只使用一种协议：Gateway WebSocket。**

```text
clarity-egui ──┐
clarity-tui  ──┼──► clarity-gateway ──► clarity-core / clarity-wire
clarity-claw ──┘         ▲
                          │
               Gateway WebSocket（唯一内部协议）
                          │
                    clarity-openclaw
                          │
               OpenClaw JSON-RPC（外部 fallback）
                          │
                   外部 KimiClaw / OpenClaw
```

- `clarity-claw` ↔ `clarity-gateway`：Gateway WebSocket
- `clarity-egui` ↔ 任意后端：`ClawConnectionManager` 自动探测 dialect，但**发送方法由 dialect 决定**，UI 不再指定
- OpenClaw JSON-RPC 仅用于连接外部 Kimi/OpenClaw Gateway；不参与 Clarity 内部架构决策

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

### 3.2 OpenClaw JSON-RPC（外部适配层）

用于：
- 连接外部 KimiClaw / OpenClaw Gateway
- 基本聊天、历史、配对

**不用于**：
- Clarity 内部 mesh 通信
- 角色上下文同步（由 Syncthing-rust 覆盖离线场景）
- 审批、WireMessage、MCP tool events 等 Clarity 内部语义

语义：
- `connect` / `connect.challenge`
- `sessions.send`（`key`）
- `session.message` / `chat` / `agent` 事件

---

## 4. 代码影响

### 4.1 `clarity-openclaw`

- `ClawConnectionManager` 保留自动探测能力
- `ProtocolCommand::Chat` 不再携带 `use_sessions_send`
- Gateway 管理器固定使用 `chat.send`
- OpenClaw 管理器固定使用 `sessions.send`
- OpenClaw 翻译层只保留基本聊天/历史/错误/配对映射，逐步移除对 Gateway 内部语义的模仿

### 4.2 前端

- 删除 `App.claw_ws_uses_sessions_send` 等协议泄漏字段
- `ClawClientHandle::send_chat` 只接受 `session_key` + `message`
- UI 不再根据协议选择发送方法

### 4.3 `clarity-claw`

- **只做 Gateway WebSocket 客户端**；它是 Clarity 内部 mesh 的系统托盘常驻节点
- 不与外部 KimiClaw/OpenClaw Gateway 直接交互
- 不持有 OpenClaw JSON-RPC fallback
- 外部互通由 `clarity-gateway` 侧的 `clarity-openclaw` 模块负责协议转换
- 当前任务/线程轮询仍走 Gateway HTTP admin 端点；聊天与角色上下文同步走 Gateway WebSocket

---

## 5. 未来工作

1. **合并 parser**：长期目标是把 `GatewayClient` 与 `ClawClient` 统一为单套实现，OpenClaw 仅保留一个薄翻译层
2. **协议协商**：Gateway 可在握手时通过 `Sec-WebSocket-Protocol` 声明 `clarity-v1`，OpenClaw 端保持现有行为
3. **外部互通**：若外部 OpenClaw Gateway 需要接入 Clarity mesh，由 clarity-openclaw 提供协议转换，而非让 mesh 迁就 OpenClaw 语义

---

## 6. 参考

- `crates/clarity-openclaw/src/connection_manager.rs`
- `crates/clarity-egui/src/claw.rs`
- `crates/clarity-egui/src/panels/right_ide_panel/claw_settings_panel.rs`
- `docs/architecture/protocol-layer.md`
