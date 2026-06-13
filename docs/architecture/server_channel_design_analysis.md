---
title: Server 模块 + Channel Adapter 工程分析
category: Design
date: 2026-05-16
tags: [design]
---

# Server 模块 + Channel Adapter 工程分析

> 日期：2026-04-24 | 分析依据：实机代码审计 + 竞品逆向 + 工程理论

---

## 一、分析目标

对第二梯队剩余两个任务进行工程层面的深度分析，明确：
1. **Server 模块**：`clarity-core` 如何向外部暴露本地服务接口
2. **Channel Adapter**：Agent 如何主动发送消息到 IM 渠道

---

## 二、Server 模块：本地 API 暴露

### 2.1 当前架构约束

```
clarity-tui    → 直接链接 clarity-core（同进程）
clarity-gateway → Axum HTTP server → 通过 AgentController 操作 core（单向依赖）
clarity-claw    → HTTP 轮询 gateway（间接依赖 core）
clarity-core    → 无网络/IPC 监听能力，纯业务逻辑层
```

**核心发现**：`AgentController`（`agent/controller.rs`）本质上已是本地服务接口——
- `Op` 通道接收命令（`UserTurn`, `Interrupt`, `Shutdown`）
- `ControllerEvent` 流式输出（`Chunk`, `Complete`, `Error`, `ToolCallStart`）
- gateway 的 `chat_completions` handler 正是通过 `AgentController::new_with_events()` 驱动 Agent

### 2.2 竞品参考

| 项目 | 服务暴露方式 | 分析 |
|------|-------------|------|
| **codex-rs** | `app-server` 支持 `stdio://` 和 `ws://`，内部 JSON-RPC 2.0 | 统一服务层，无"core 暴露服务"问题 |
| **cc-haha** | Tauri 主进程启动 sidecar（server + adapters），adapters 通过 WebSocket 连接 | Channels 是独立进程，server/adapter 通过 WebSocket 解耦 |

### 2.3 设计选项对比

| 选项 | 理论依据 | 优势 | 劣势 | 适合度 |
|------|---------|------|------|--------|
| **core 内嵌 HTTP server** | 反模式；违反分层原则 | 用户提过需求 | 破坏单向依赖；core 变重；引入 axum/tokio-net | ❌ 不推荐 |
| **通过 clarity-wire 暴露** | SPMC 观察者模式 | 复用已有基础设施 | wire 无请求/响应语义；无法支持"调用-等待结果" | ❌ 不适合 |
| **UDS/Named Pipe + JSON-RPC** | LSP 模式；本地 IPC 标准 | 多客户端；无网络栈 | Windows Named Pipe 实现复杂；需新增 transport 层 | ⚠️ 备选 |
| **JSON-RPC over stdio（MCP server 模式）** | 复用现有 JSON-RPC 基础；与 AGENTS.md "优先 stdio MCP" 约束一致 | 零网络；复用 MCP 序列化代码；AgentController 天然适配事件流 | stdio 仅单客户端 | ✅ **推荐** |

### 2.4 推荐方案：JSON-RPC over stdio

**选择理由**：
1. **架构守恒**：stdio 不是网络，core 仍保持"无网络监听"的纯度
2. **基础设施复用**：core 的 `mcp/mod.rs` 已有 JSON-RPC 序列化/反序列化代码
3. **契约一致**：与 AGENTS.md 中 `"clarity 优先 stdio MCP"` 的约束对齐
4. **语义匹配**：`AgentController` 的 `Op` 和 `ControllerEvent` 天然映射 JSON-RPC request/notification

**Trade-off**：stdio 仅支持单客户端。若需多客户端（同时被 IDE 插件、TUI、脚本调用），仍需 `clarity-gateway` 作为多路复用中介。

### 2.5 实现路径

1. **新建文件**：`crates/clarity-core/src/server/stdio.rs`
   - `StdioServer`：从 stdin 读取 JSON-RPC request，通过 stdout 输出 response/notification
   - 方法映射：`agent/run` → `Op::UserTurn`，`agent/interrupt` → `Op::Interrupt`
   - 通过 `AgentController::new_with_events()` 驱动 Agent

2. **修改文件**：`crates/clarity-core/src/lib.rs` — 新增 `pub mod server;`

3. **修改文件**：`crates/clarity-core/Cargo.toml` — 新增可选 feature `"stdio-server"`，不引入额外依赖

---

## 三、Channel Adapter：IM 渠道工具化

### 3.1 当前架构约束

| 组件 | 当前 outbound 能力 | 位置 |
|------|-------------------|------|
| `clarity-gateway` | `WebhookSender`（飞书/钉钉/通用 webhook，含 HMAC 签名） | `channels/webhook.rs` |
| `clarity-core` | `PushNotificationTool`（`file` + generic `webhook` HTTP POST） | `tools/notify.rs` |
| `clarity-core` | `WebFetchTool` / `WebSearchTool`（HTTP client 参考） | `tools/web.rs` |

**核心约束**：`gateway → core` 单向依赖，core **不可**反向调用 gateway。

### 3.2 竞品参考

| 项目 | Channel 实现方式 | 分析 |
|------|-----------------|------|
| **cc-haha** | 独立 sidecar 进程，通过 WebSocket 连接 server | Channels 不是 Agent 的工具，而是外部接入点 |
| **openclaw** | 20+ channels，内置在 server 中 | 集成度高，但 channel 逻辑与 core 耦合 |

### 3.3 设计选项对比

| 选项 | 理论依据 | 优势 | 劣势 | 适合度 |
|------|---------|------|------|--------|
| **core 新增 `ChannelSendTool`** | 工具化原则；Agent 自主决策 | 不依赖 gateway 运行时；符合现有 tool 模式 | 需重复部分格式化逻辑；core 增加 hmac/sha2 依赖 | ✅ **推荐** |
| **MCP 协议暴露 gateway channels** | 协议标准化 | 接口统一 | 杀鸡用牛刀；延迟高；gateway 需实现 MCP server | ❌ 过度设计 |
| **gateway 暴露 channel API，core HTTP 调用** | localhost 回环 | 复用 gateway 格式化逻辑 | 强运行时依赖 gateway；HTTP 失败风险；破坏 core 独立性 | ⚠️ 备选 |

### 3.4 推荐方案：独立 `ChannelSendTool`

**选择理由**：
1. **运行时独立**：Agent 发送消息不应依赖 gateway 进程是否存活
2. **工具化一致性**：Agent 通过 `ToolCall` 自主决定何时、向哪发送消息
3. **依赖已就绪**：core 已有 `reqwest`，仅需追加 `hmac`、`sha2`、`base64` 三个轻量密码学 crate
4. **代码重复可控**：飞书/钉钉的 JSON 格式化和 HMAC 签名逻辑约 50 行，允许 gateway 与 core 间有限重复

**Trade-off**：与 `gateway/channels/webhook.rs` 中的 `WebhookSender` 有少量代码重复。可通过文档注释标注双向同步点，而非强行抽象共享模块。

### 3.5 实现路径

1. **修改文件**：`crates/clarity-core/Cargo.toml`
   - 新增：`hmac = "0.12"`, `sha2 = "0.10"`, `base64 = "0.22"`

2. **新建文件**：`crates/clarity-core/src/tools/channel.rs`
   - `ChannelSendTool`：支持 `feishu | dingtalk | slack | webhook`
   - 参数：`platform`, `webhook_url`, `message`, `secret`（可选，钉钉 HMAC）
   - 平台格式化 + HMAC-SHA256 签名（约 15 行独立实现）

3. **修改文件**：`crates/clarity-core/src/tools/mod.rs` — 注册导出

---

## 四、串并行推进建议

两个任务**完全独立**，可并行推进：

| 任务 | 修改的文件 | 与另一任务的冲突点 |
|------|-----------|-------------------|
| Server (stdio JSON-RPC) | `server/stdio.rs`（新建）、`lib.rs`、`Cargo.toml` | `lib.rs` 和 `Cargo.toml` |
| ChannelSendTool | `tools/channel.rs`（新建）、`tools/mod.rs`、`Cargo.toml` | `tools/mod.rs` 和 `Cargo.toml` |

**串行点**：
1. `Cargo.toml` — 两个任务都需要添加依赖
2. `lib.rs` — 两个任务都需要注册模块

**执行策略**：
1. 并行编写各自的核心代码（不碰 `Cargo.toml` 和 `lib.rs`）
2. 完成后统一串行修改 `Cargo.toml` + `lib.rs`
3. `cargo test` + `clippy` → `git commit` → `git push`

---

## 五、结论速查

| 任务 | 推荐方案 | 关键 trade-off | 核心修改文件 |
|------|---------|---------------|-------------|
| **Server 模块** | core 内 JSON-RPC over stdio，暴露 `AgentController` | stdio 仅单客户端；多客户端仍需 gateway | `server/stdio.rs`（新建）、`lib.rs`、`Cargo.toml` |
| **Channel Adapter** | core 内 `ChannelSendTool`，独立 HTTP 发送 | 与 gateway 少量代码重复；保持 core 独立性 | `tools/channel.rs`（新建）、`tools/mod.rs`、`Cargo.toml` |
