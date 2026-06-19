<!-- DOC-CONTRACT: 本文件维护 clarity-openclaw crate 的 Agent 运行上下文与代码风格。 -->

# Agent Guidance for clarity-openclaw

> **Scope:** `crates/clarity-openclaw`  
> **Type:** lib  
> **License:** AGPL-3.0-or-later

## 设计约束

- **无前端依赖**：`clarity-openclaw` 禁止依赖 `clarity-egui`、`clarity-tui`、`eframe`、`egui`、`ratatui` 等 UI crate。
- **无核心反向依赖**：`clarity-openclaw` 是协议/网络层库，不依赖 `clarity-core`，避免核心 crate 被网络实现污染。
- **可跨入口复用**：`clarity-egui`、`clarity-tui`、`clarity-gateway`、`clarity-claw`、`clarity-headless` 均可能依赖本 crate。
- **WebSocket 连接禁止阻塞 UI 帧**：所有网络 I/O 运行在独立后台线程/任务中，对外暴露 `std::sync::mpsc` 或 `tokio` 异步接口。

## 模块职责

| 模块 | 路径 | 职责 |
|------|------|------|
| `client` | `src/client.rs` | OpenClaw WebSocket JSON-RPC 客户端、连接、重连、消息发送、事件接收 |
| `device` | `src/device.rs` | Ed25519 设备身份、配对签名、已配对 token 持久化 |
| `discovery` | `src/discovery.rs` | 从 `~/.kimi_openclaw`、环境变量发现本地/远程 Gateway |
| `types` | `src/types.rs` | UI 无关的连接参数与设备描述类型 |

## 代码风格

- 所有 `pub` 项必须带 `///` 文档注释（workspace `missing_docs = "deny"`）。
- 禁止新增 `unsafe`、`unwrap`、`expect`、`panic`；例外需加 `// SAFE:` 或同等说明。
- 单元测试必须覆盖协议解析、累积流 delta、设备身份签名等核心逻辑。
