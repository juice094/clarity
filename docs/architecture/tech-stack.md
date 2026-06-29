---
title: 技术栈与 Crate 拓扑
category: Architecture
date: 2026-06-13
tags: [architecture, tech-stack, crates]
---

# 技术栈与 Crate 拓扑

> 代码级精确架构参考请查阅 [`ARCHITECTURE.md`](./ARCHITECTURE.md)；项目定位与生态关系请查阅 [`architecture-positioning.md`](./architecture-positioning.md)。

---

## 技术栈

| 层级 | 技术 |
|------|------|
| 编程语言 | Rust 2024 edition，MSRV 1.85 |
| 异步运行时 | `tokio` full |
| 序列化 | `serde` / `serde_json` / `toml` / `serde_yaml` |
| 错误处理 | `thiserror` / `anyhow` |
| HTTP/WebSocket | `axum` 0.7，`tower-http`，`reqwest` + `rustls-tls` |
| 桌面 GUI | `eframe` / `egui` 0.31，`lucide-icons` |
| 终端 UI | `ratatui` 0.30，`crossterm` 0.29 |
| 本地推理 | `candle-core` / `candle-transformers` / `tokenizers` / `hf-hub`（feature `local-llm`） |
| 记忆存储 | `rusqlite`（bundled-full，FTS5），BM25 + 向量混合 |
| 遥测 | `clarity-telemetry`（SQLite/GreptimeDB/ConfigAudit） |
| 加密 | `chacha20poly1305`（`clarity-secrets`） |
| 锁/同步 | `parking_lot` 为主，保留少量 `std::sync` |

---

## Crate 拓扑

| Crate | 类型 | 职责 | 关键说明 |
|-------|------|------|----------|
| `clarity-contract` | lib | 共享契约层：`LlmProvider`、`Tool`、`AgentError`、`FederationMessage`、`ThreadId`、`RolloutItem` | 零内部依赖 |
| `clarity-wire` | lib | UI ↔ Agent 事件总线（SPMC）、`ViewCommand`/`WireMessage` | 跨前端通信唯一通道 |
| `clarity-memory` | lib | SQLite/文件/混合记忆、BM25+向量、chunking、四级压缩 | feature `sqlite` / `embedding` |
| `clarity-mcp` | lib | MCP 客户端：stdio / SSE / HTTP / WebSocket | 含命令校验安全层 |
| `clarity-llm` | lib | LLM provider 抽象 + 内置 provider + Candle GGUF | feature `local-llm` / `local-llm-cuda` |
| `clarity-tools` | lib | 内置工具库：file / shell / web / devkit / team / task / … | 从 `clarity-core` 拆出 |
| `clarity-secrets` | lib | ChaCha20-Poly1305 加密 Secret 存储（`enc2:`） | 用于 `models.toml` 加密 key |
| `clarity-channels` | lib | 外部通道抽象；当前实现 WeChat iLink（`chkit`）；Webhook 默认启用 | Discord/Slack/Telegram 默认禁用 |
| `clarity-subagents` | lib | 子代理执行器、并行调度、团队协调 | 消费 `clarity-core` |
| `clarity-thread-store` | lib | Thread 持久化抽象：`ThreadStore` trait（API 设计受 Codex 启发） | 依赖 `clarity-rollout` |
| `clarity-rollout` | lib | JSONL rollout 持久化：事件日志、压缩、回放（设计受 Codex 启发） | 仅依赖 `clarity-contract` |
| `clarity-core` | lib | Agent 循环（ReAct/Plan）、Approval、Skill、MCP 集成、Thread 生命周期 | **零前端/网络依赖** |
| `clarity-telemetry` | lib | 统一遥测：WideEvent、metrics、traces、config audit | feature `sqlite` / `greptime`；当前由 `clarity-gateway` 使用 |
| `clarity-gateway` | bin/lib | Axum HTTP/WebSocket 服务端、Web IDE、session store | 双端口：18790 公共 / 18800 管理 |
| `clarity-egui` | bin | 桌面 GUI（主前端栈），eframe + egui 纯 Rust | 替代已归档的 Tauri |
| `clarity-tui` | bin | ratatui 终端界面 | 远程/SSH 优选 |
| `clarity-claw` | lib + bin | 统一客户端 Claw 节点：Gateway WebSocket 客户端、OpenClaw/KimiClaw JSON-RPC 兼容层、设备发现/身份/配对、角色上下文同步；二进制为系统托盘常驻节点 | UI 无关，可被多个入口复用 |
| `clarity-headless` | bin | 无头 CLI（脚本 / CI 场景） | `--prompt` / `--file` / `--output json` |
| `clarity-mobile-core` | lib | 移动端 UniFFI FFI 核心 | 暴露 Runtime/事件/配置/记忆接口给 Kotlin/Swift |
| `clarity-slint` | bin | 桌面 GUI 实验栈，Slint | 不参与默认 CI |
| `clarity-tauri` | bin | Tauri 前端 | **已归档**，被 workspace 排除 |
| `clarity-anthropic-proxy` | bin | Anthropic Messages API 网关 | 默认 DeepSeek device；协议转换在 `clarity-llm::anthropic` |

---

## 架构依赖方向

```text
                         ┌──────────────────────────────────────┐
                         │         clarity-contract             │
                         │       （零内部依赖 · 共享契约）         │
                         └──────────────────┬───────────────────┘
                                            ▲
    ┌───────────┬──────────┬─────────┬──────┴──────┬──────────┬──────────┐
    ▼           ▼          ▼         ▼             ▼          ▼          ▼
clarity-wire clarity-memory clarity-mcp clarity-llm clarity-tools clarity-channels
clarity-secrets clarity-rollout
    │
    ▼
clarity-thread-store
    │
    ▼
clarity-core
    │
    ├── clarity-subagents（消费 core）
    ├── clarity-telemetry（当前由 gateway 使用）
    │
    ▼
{clarity-egui, clarity-tui, clarity-gateway, clarity-claw, clarity-headless, clarity-mobile-core}

clarity-claw：统一客户端 Claw 节点（lib + bin）；库提供 Gateway WebSocket 客户端、OpenClaw/KimiClaw 兼容层、设备发现/身份/配对、角色上下文同步；二进制为系统托盘常驻节点
clarity-telemetry：当前由 clarity-gateway 使用
clarity-slint：实验栈，不参与默认 CI
clarity-tauri：已归档，被 workspace 排除
clarity-anthropic-proxy：Anthropic Messages API 网关（默认 DeepSeek device；协议转换在 `clarity-llm::anthropic`）
```

**不可违反的不变量**：

1. `clarity-core` 不依赖任何前端 crate（`egui`、`tui`、`axum`）或网络 crate。
2. `clarity-contract` 不依赖任何内部 crate。
3. 前端 crate 之间不互相 import；跨前端状态/事件走 `clarity-wire`。

---

*最后更新：2026-06-19*
