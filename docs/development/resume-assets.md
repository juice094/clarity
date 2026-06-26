> **用途**：Clarity 项目简历技术资产清单，按 8 个维度整理。
> **生成日期**：2026-06-25
> **数据基线**：`AGENTS.md` §6.2 / §11、`docs/okf/clarity-worktree/`、实机验证

---

# Clarity 项目 —— 简历技术资产清单

## 1. 项目定位

Clarity 是一个 **Rust 原生的本地优先（local-first）个人 AI 运行时**，用同一套 Agent 内核支撑 TUI、桌面 GUI、Web IDE、无头 CLI、系统托盘和移动端 FFI 六种入口。它聚焦**编码/工程工作流**，在本地完成 LLM 编排、工具调用（MCP + 内置工具）、记忆持久化、审批流程与多 Agent 协作，**无需 Python/Node.js/Ollama 等外部运行时**。

目标用户是需要在本地安全、可控地运行 AI 协作者的开发者；典型场景包括代码审查、自动化脚本、本地知识库问答、CI 集成与多步骤工程任务执行。

---

## 2. 模块结构与职责

| 模块/Crate | 一句话职责 |
|-----------|-----------|
| `clarity-contract` | 共享契约层：`LlmProvider`/`Tool`/`AgentError` 等 trait，零内部依赖（`crates/clarity-contract/src/`） |
| `clarity-wire` | UI ↔ Agent 的 SPMC 事件总线，承载 `WireMessage`/`ViewCommand`（`crates/clarity-wire/src/`） |
| `clarity-core` | Agent 内核：ReAct/Plan 循环、Approval、Skill、MCP 集成、后台任务、Thread 生命周期（`crates/clarity-core/src/agent/`） |
| `clarity-llm` | LLM provider 抽象 + 6+ 内置 provider + Candle GGUF 本地推理（`crates/clarity-llm/src/`） |
| `clarity-memory` | 混合记忆：SQLite + BM25 + 向量搜索 + chunking + 四级压缩归档（`crates/clarity-memory/src/`） |
| `clarity-mcp` | MCP 客户端：stdio / SSE / HTTP / WebSocket 四传输（`crates/clarity-mcp/src/`） |
| `clarity-tools` | 内置工具库：file / shell / web / devkit / team / task（`crates/clarity-tools/src/`） |
| `clarity-subagents` | 子代理执行器 + 并行调度，消费 `clarity-core`（`crates/clarity-subagents/src/`） |
| `clarity-thread-store` | Thread 持久化抽象，`ThreadStore` trait（`crates/clarity-thread-store/src/`） |
| `clarity-rollout` | JSONL rollout 持久化：事件日志、压缩、回放（`crates/clarity-rollout/src/`） |
| `clarity-openclaw` | OpenClaw/KimiClaw Gateway WebSocket 客户端与设备身份（`crates/clarity-openclaw/src/`） |
| `clarity-secrets` | ChaCha20-Poly1305 加密 Secret 存储（`enc2:`）（`crates/clarity-secrets/src/`） |
| `clarity-telemetry` | 统一遥测：WideEvent、metrics、traces、config audit（`crates/clarity-telemetry/src/`） |
| `clarity-gateway` | Axum HTTP/WebSocket 服务端 + Web IDE + session store（`crates/clarity-gateway/src/`） |
| `clarity-egui` | 桌面 GUI 主入口：eframe/egui + Pretext 三栏布局（`crates/clarity-egui/src/`） |
| `clarity-tui` | ratatui 终端界面（`crates/clarity-tui/src/`） |
| `clarity-claw` | 系统托盘常驻节点，仅通过 Gateway WebSocket 通信（`crates/clarity-claw/src/`） |
| `clarity-headless` | 无头 CLI，脚本/CI 场景（`crates/clarity-headless/src/`） |
| `clarity-mobile-core` | 移动端 UniFFI FFI 核心，暴露 Runtime/事件/配置/记忆接口（`crates/clarity-mobile-core/src/`） |
| `clarity-slint` | 实验性 Slint 桌面 GUI，不参与默认 CI（`crates/clarity-slint/src/`） |
| `clarity-anthropic-proxy` | Anthropic Messages API → DeepSeek 代理工具（`crates/clarity-anthropic-proxy/src/`） |
| `clarity-tauri` | **已归档**，被 workspace 排除（`crates/clarity-tauri/`） |

---

## 3. 核心架构模式

| 架构模式 | 解决什么问题 | 在项目中的体现 |
|---------|-------------|---------------|
| **Contract-First** | 避免循环依赖，给 LLM/Tool/错误类型一个稳定契约 | `clarity-contract` 零内部依赖，所有 crate 都基于它构建；`crates/clarity-contract/src/lib.rs` |
| **严格分层架构** | 隔离前端、内核、基础设施，保证核心可测试、可移植 | `contract → infrastructure → core → presentation`；前端 crate 禁止互相 import（`AGENTS.md` §3 不变量） |
| **事件驱动（SPMC）** | 同一 Agent 状态被多个前端消费，避免 N×M 耦合 | `clarity-wire` 单生产者多消费者通道；`WireMessage` 是跨前端唯一协议（`crates/clarity-wire/src/`） |
| **多前端共享内核** | 一套 Agent 逻辑同时服务 GUI/TUI/Web/CLI/托盘 | 所有前端消费 `clarity-core` + `clarity-wire`，不重复实现业务逻辑 |
| **插件化工具生态（MCP）** | 让外部工具服务器以标准协议接入，不污染核心 | `clarity-mcp` 支持 stdio/SSE/HTTP/WS 四传输；`clarity-tools` 内置工具也走同一 `Tool` trait |
| **CQRS 式 UI 状态** | 命令与视图状态分离，支持流式更新与历史回放 | `ViewCommand` 驱动渲染，`ViewState` 单源化；`clarity-egui/src/ui/` 与 `clarity-core/src/ui/` |
| **本地优先 + 单二进制** | 降低部署成本，消除运行时依赖冲突 | 每个 bin crate 单二进制分发；默认内置 Candle GGUF；`cargo install` 即可运行 |
| **审批模式四层模型** | 在高风险工具调用与用户自由之间取得平衡 | `Interactive/Smart/Plan/Yolo` 四级 Approval；`clarity-core/src/approval/` |

---

## 4. 关键技术栈

### 语言/运行时
- **Rust 2024 edition**，MSRV 1.85
- **tokio** 异步运行时

### 前端/交互
- **egui 0.31 / eframe 0.31** — 纯 Rust 即时模式桌面 GUI，零 Web 依赖
- **ratatui 0.30** — 终端 UI
- **Axum 0.7 / tower-http / SSE / WebSocket** — Web IDE 与服务端
- **Slint** — 实验性 GUI 栈

### AI/LLM
- **Candle** — 本地 GGUF 推理（Qwen2 / Qwen2.5 / DeepSeek-R1-Distill）
- **MCP（Model Context Protocol）** — 工具生态标准协议
- **ReAct / Plan Agent 循环** — 推理+行动模式

### 记忆/存储
- **SQLite（WAL）** — 结构化持久化
- **BM25** — 关键词检索
- **向量搜索** — 语义检索
- **四级压缩归档** — 上下文压缩与 token 爆炸防护

### 安全/通信
- **ChaCha20-Poly1305** — `enc2:` 加密 Secret 存储
- **rustls-tls** — 纯 Rust TLS；openssl 已从依赖树移除
- **tokio-tungstenite** — WebSocket
- **ed25519-dalek** — 设备身份签名

### 移动端/FFI
- **UniFFI 0.29** — Kotlin/Swift 绑定生成

### 构建/部署
- **cargo workspace** 管理 22+ crate
- **cargo-wix** — Windows MSI 打包
- **GitHub Actions CI** — 12-job 流水线

### 能体现技术深度的非大众选型
1. **Candle GGUF 本地推理**：不依赖 llama.cpp/Ollama，纯 Rust 推理栈
2. **BM25 + 向量混合记忆**：自研检索组合，而非直接上 Qdrant/Pinecone
3. **SPMC 事件总线替代 RPC**：前端与内核在同一进程内通过内存通道解耦
4. **Pretext 文字测量后端**：为 egui 聊天界面做精确高度估算与对齐回归
5. **UniFFI 移动 FFI**：把 Rust Agent 运行时封装为 Android/iOS 可调用的库

---

## 5. 解决过的 3 个最硬核技术问题

### 问题 1：Pretext 三栏布局下的消息气泡高度精确估算
- **问题描述**：`clarity-egui` 聊天界面需要在不知道最终渲染宽度的情况下，提前估算 `MessageBubble` 高度以支撑虚拟滚动和右侧轨道布局；传统 egui 文本测量在复杂样式（行内代码、链接、不同字号）下偏差大，导致 1000+ 条消息时滚动跳变。
- **方案**：接入 `pretext-core` / `pretext-fontdb`，用 egui 字体栈作为 measurement backend，实现 `pretext::EguiFontMetrics`；将 `MessageBubble` 与 `widgets/rich_paragraph.rs` 迁移为 pretext-aware；默认启用 estimate + render 双阶段测量。
- **关键技术**：文字测量后端抽象、字体 metric 映射、对齐回归测试、release 性能基准。
- **可量化结果**：23 样本对齐回归测试通过，1000 条消息 release 基准下聚合高度偏差 ≈ **1.45%**，estimate 阶段 ≈ **74.4 µs/msg**，render 阶段 ≈ **135.7 µs/msg**。
- **相关路径**：`crates/clarity-egui/src/widgets/rich_paragraph.rs`、`crates/clarity-egui/src/components/chat/message_bubble.rs`（若存在）、`crates/clarity-egui/src/layout.rs`、`AGENTS.md` §11.1。

### 问题 2：Claw 协议 dialect 统一与解耦
- **问题描述**：早期 `clarity-egui` 直接通过协议标志 `claw_ws_uses_sessions_send` 决定发送方式，导致 UI 层泄漏 Claw/OpenClaw 协议细节；同时 `clarity-claw` 角色模糊，既想做内部托盘节点又想兼任外部 OpenClaw 适配器。
- **方案**：决策 Gateway WebSocket 为 Clarity 内部唯一协议；OpenClaw JSON-RPC 仅作为外部 KimiClaw/OpenClaw Gateway 互通的 fallback；由 `ClawConnectionManager` 根据检测到的 dialect 自行决定发送方法；删除 egui 层协议泄漏字段；明确 `clarity-claw` 只做 Gateway WebSocket 客户端/系统托盘节点，移除未使用的 federation coordinator/nodes/runtime 骨架。
- **关键技术**：协议 dialect 检测、职责边界重构、依赖方向审计、 dead code 清理。
- **可量化结果**：消除了 UI ↔ 协议之间的反向依赖；`clarity-claw` 代码体积收缩；协议映射关系写入 `docs/architecture/claw-protocol.md`。
- **相关路径**：`crates/clarity-openclaw/src/`、`crates/clarity-claw/src/`、`docs/architecture/claw-protocol.md`、`AGENTS.md` §11.1。

### 问题 3：零外部依赖的本地优先 AI 运行时
- **问题描述**：大多数 AI Agent 项目依赖 Python/Node.js/Ollama/llama.cpp 等外部运行时，导致安装复杂、版本冲突、离线场景受限。
- **方案**：用 Rust 完整实现 Agent 内核；LLM 推理用 Candle 原生 GGUF；记忆用 SQLite + 自研 BM25/向量；前端用 egui/ratatui/Axum 纯 Rust 栈；每个入口编译为单二进制；通过 `models.toml` + `enc2:` Secret + `ReliableProvider` failover 构建完整的本地 provider 体系。
- **关键技术**：Candle GGUF 集成、WAL SQLite、BM25+向量混合检索、ChaCha20-Poly1305、provider 抽象与 failover 链。
- **可量化结果**：22 个活跃 workspace crate、**1,554 lib tests / 275 bin tests / 34 doc tests / 26 集成测试全绿**、Clippy **零 warning**；单个 `cargo install` 即可运行。
- **相关路径**：`crates/clarity-llm/src/`、`crates/clarity-memory/src/`、`crates/clarity-secrets/src/`、`docs/development/provider-config.md`。

---

## 6. 性能/规模/稳定性相关数据

| 指标 | 数值 | 来源/备注 |
|------|------|----------|
| 活跃 workspace crate 数 | 22 + 1 归档（`clarity-tauri`）+ 1 集成测试 crate | `Cargo.toml` / `AGENTS.md` |
| Rust 源文件数 | ~200+ | `docs/ARCHITECTURE.md` §2.1a（待精确统计） |
| lib 测试通过数 | **1,554 passed / 0 failed / 0 ignored** | 2026-06-25 实机验证 |
| bin 测试通过数 | **275 passed / 0 failed / 2 ignored** | 2026-06-25 实机验证 |
| doc 测试通过数 | **34 passed / 0 failed / 3 ignored** | 2026-06-25 实机验证 |
| 集成测试通过数 | **26 passed / 0 failed** | 2026-06-25 实机验证 |
| Clippy warning | **0** | `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` |
| Pretext 高度偏差 | ≈ **1.45%** | 23 样本对齐回归测试（`AGENTS.md` §11.1） |
| Pretext estimate 耗时 | ≈ **74.4 µs/msg** | 1000 条消息 release 基准 |
| Pretext render 耗时 | ≈ **135.7 µs/msg** | 1000 条消息 release 基准 |
| unsafe 代码 | **1 处**（白名单，在 `clarity-memory`） | `AGENTS.md` §7.1 |
| 桌面 GUI 默认窗口 | 1280×800 | `AGENTS.md` §11.1 |
| 请求量/QPS/生产部署规模 | 待验证 | 项目定位为本地运行时，无公开服务端运行数据 |

---

## 7. 简历专业化关键词

### 技术关键词
Rust 2024、tokio、egui、eframe、ratatui、Axum、tower-http、WebSocket、SSE、Candle、GGUF、MCP（Model Context Protocol）、SQLite、BM25、Vector Search、UniFFI、ChaCha20-Poly1305、rustls、ReAct、Plan、SPMC、WAL、JSONL、OAuth Device Flow、Cargo Workspace、CI/CD。

### 能力关键词
Contract-First 架构、严格分层架构、事件驱动架构、多前端共享内核、本地优先（Local-First）、零外部依赖、单二进制分发、混合检索（BM25 + Vector）、RAG、Multi-Agent 调度、审批模式设计、协议 dialect 统一、FFI 桥接、性能基准测试、回归测试、内存安全、零 unsafe 扩展、 Secret 加密存储、Provider Failover。

---

## 8. 适合写在简历上的 3-5 条 Bullet Point

1. **主导 Rust 原生本地优先 AI 运行时架构设计**，用 22 个 workspace crate 实现 TUI/桌面 GUI/Web IDE/CLI/系统托盘/移动端 FFI 六入口共享同一 Agent 内核，实现 `cargo install` 即可运行，零 Python/Node.js/Ollama 外部依赖。

2. **实现混合记忆系统与本地 LLM 推理栈**，整合 SQLite + BM25 + 向量搜索 + 四级压缩归档，并接入 Candle GGUF 本地推理；配套 `models.toml` per-alias 配置、`enc2:` 加密 Secret、`ReliableProvider` 链式 failover。

3. **落地跨前端 SPMC 事件总线协议**，设计 `WireMessage`/`ViewCommand` 统一 UI ↔ Agent 通信，使 egui/ratatui/Axum 等前端不互相 import、不重复实现业务逻辑，支撑流式响应与状态回放。

4. **统一 Claw/OpenClaw 协议边界**，决策 Gateway WebSocket 为内部唯一协议、OpenClaw JSON-RPC 为外部 fallback，由 `ClawConnectionManager` 自动检测 dialect，消除 UI 层协议泄漏，并输出 `docs/architecture/claw-protocol.md`。

5. **建立零 warning、零失败的工程基线**：推动 `cargo clippy -D warnings` 全绿、维护 1,554+ lib / 275 bin / 34 doc / 26 集成测试全通过；引入 Pretext 文字测量使 1000 条消息渲染高度偏差降至 1.45%。

---

## 不确定/待验证项

- 精确 Rust 源文件数量（约 200+，需 `find crates -name '*.rs' | wc -l` 精确统计）。
- 实际生产运行中的请求量、并发数、QPS（项目为本地运行时，一般无公开服务端指标）。
- 部分子模块（如 `clarity-telemetry` GreptimeDB 后端）是否已在 CI 默认流程中启用。
- `clarity-anthropic-proxy` 的日均调用量或生产稳定性数据（该 crate 为工具二进制，无公开指标）。
