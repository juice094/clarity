# Project Clarity — 项目现状审计与技术重构报告

> 编制日期：2026-04-03（更新）
> 目的：纠正旧文档中的功能幻觉与规划过拟合，基于实际代码状态与行业技术趋势，重建项目认知基线。本版报告已纳入 Phase 1/2 的重构成果。

---

## 1. 执行摘要

Project Clarity 是一个以 Rust 实现的本地优先 AI Agent 框架原型。经过对代码库的全面审计，结论是：**项目已拥有一个可编译、可运行的基础骨架，但旧版说明文档（README、ARCHITECTURE、DEV_LOG、SESSION_SUMMARY、AI_HANDOFF、HUMAN_GUIDE）存在显著的功能夸大、代码量虚报与进度幻觉。**

本报告将旧资料归档至 `archive/`，并基于实际代码状态重写项目说明。同时，结合 2024–2025 年 AI Agent 领域的技术演进（尤其是 MCP 标准化、Ratatui TUI 最佳实践、SQLite 本地记忆架构），提出一条与当前代码基础相匹配的务实发展路线。

**自报告初版以来，项目已完成 Phase 1（核心闭环）与 Phase 2（体验打磨）的大部分工作：**
- ✅ Memory × Agent 闭环集成
- ✅ Gateway 去 stub 化
- ✅ 全 workspace warning / clippy 清零
- ✅ 真实 SSE 流式响应（core + TUI）
- ✅ MCP Client 骨架落地（stdio JSON-RPC 2.0）
- ✅ TUI 组件化拆分（ChatPane / InputPane / StatusBar / GeneratingIndicator）

---

## 2. 现状审计：代码、编译与测试

### 2.1 代码规模（实际 vs 旧文档宣称）

| 来源 | 宣称 | 实际 |
|------|------|------|
| `DEV_LOG.md` | `clarity-core` ~8,000 行；`clarity-tui` ~3,500 行；`clarity-gateway` ~4,500 行 | **全 crates 目录 `.rs` 文件总大小约 283 KB，折合全部源码（含测试、注释）约 10,000–12,000 行。** 各 crate 均远小于宣称数字。 |
| `SESSION_SUMMARY.md` | "69 files changed, 14,848 insertions(+)" | 该数字是某次 Git diff 的统计快照，被误读为项目整体规模。 |

**结论**：旧文档将某个会话的增量改动或主观估计，直接作为各 crate 的完成代码量呈现，造成严重的规模幻觉。

### 2.2 编译与测试状态

```bash
cargo check --workspace      # ✅ 通过，零 warning
cargo test --workspace       # ✅ 全部通过（core 40 + gateway 5 + memory 33 + tui 0）
cargo build --workspace      # ✅ 通过
cargo clippy --workspace     # ✅ 零警告
```

- **`clarity-memory`**: 33 个单元测试全部通过，是该项目中代码完整度最高的 crate。
- **`clarity-core`**: 40 个单元测试 + 1 个 SSE 端到端集成测试 + 7 个 Doc-tests 全部通过。
- **`clarity-gateway`**: 5 个测试通过。
- **`clarity-tui`**: 尚无单元测试（依赖终端环境，以集成/手工验证为主）。

### 2.3 代码健康度

- **Warnings 已清零**：`cargo clippy --workspace` 无任何 warning。此前大量的 `unused import`、`dead_code` 已清理完毕。
- **Gateway 已去 stub**：`chat_completions` handler 现在创建真正的 `Agent` 实例并调用 `agent.run()`；`admin_tools` 动态从 `ToolRegistry` 读取工具列表。
- **Memory 已闭环**：`Agent::run()` 和 `Agent::run_streaming()` 均会在对话前注入 `# Relevant Memories`，对话后调用 `memory_ticker.tick()` 触发编译流水线。

---

## 3. 近期重构成果（2026-04-03）

### 3.1 Memory × Agent 闭环集成

- `Agent::run()` 在构建 `messages` 前，通过 `memory_store.search(query, 5)` 检索相关记忆。
- 将检索结果格式化为 `\n\n# Relevant Memories\n- ...` 追加到 System Prompt。
- 对话结束后，调用 `memory_ticker.tick()`，将 working memory 沉淀到 long-term memory（SQLite + FTS5）。

### 3.2 Gateway 去 stub 化

- `crates/clarity-gateway/src/handlers.rs`
  - `chat_completions`: 从请求体提取 `message`，构造 `Agent` 实例，调用 `agent.run()` 返回真实 LLM 响应。
  - `admin_tools`: 从 `ToolRegistry` 动态读取工具列表和 schema，不再返回硬编码 mock 数据。

### 3.3 真实 SSE 流式响应

- `LlmProvider` trait 新增 `stream()` 方法，返回 `mpsc::Receiver<Result<String, AgentError>>`。
- `KimiLlm` 和 `OpenAiCompatibleLlm` 均实现了 SSE 解析：
  - OpenAI 协议：解析 `choices[0].delta.content`
  - Anthropic 协议：解析 `delta.text` / `content_block.text`
- `Agent::run_streaming(query, on_chunk)`：先用 `complete()` 处理 tool call 轮次，最终响应用 `llm.stream()` 通过增量回调 `on_chunk` 吐出 chunk。
- TUI 已移除"每 5 字符假流式"，改为 `unbounded_channel` + 真实 `run_streaming` callback，事件循环直接消费 LLM 增量输出。
- 新增端到端集成测试 `tests/streaming_e2e_test.rs`：启动本地 mock HTTP server，验证 `OpenAiCompatibleLlm` 经过真实 HTTP + SSE 链路后的完整行为。

### 3.4 MCP Client 骨架落地

- `crates/clarity-core/src/mcp.rs` 实现完整的 MCP stdio 客户端：
  - `McpTransport::spawn()`: 启动子进程，管理 stdin/stdout/stderr reader tasks。
  - `McpClient::connect_stdio()`: 初始化 JSON-RPC 2.0 握手 → `tools/list` → 缓存工具定义。
  - `McpToolAdapter`: 实现 `Tool` trait，将 MCP tool 映射为 Clarity native tool。
  - `McpManager`: 多连接管理器，支持同时维护多个 MCP server 连接。
- **当前状态**：代码结构完整，已通过单元测试验证 schema 转换和 manager 管理逻辑，但**尚未与真实 MCP server 进行端到端联调**。

### 3.5 TUI 组件化

- 新增组件文件：
  - `widgets/chat_pane.rs`: 聊天历史渲染（支持滚动偏移、流式光标）
  - `widgets/input_pane.rs`: 输入框状态与渲染（光标移动、字符插入删除）
  - `widgets/status_bar.rs`: 顶部状态栏（模型名、会话 ID）
  - `widgets/generating_indicator.rs`: 生成中弹窗指示器
- `app.rs` 将输入管理委托给 `InputPane`，渲染逻辑迁移至 `ui.rs`。
- 删除已废弃的 `widgets/chat.rs` 和 `widgets/input.rs`。

---

## 4. 旧文档幻觉点核查清单

| 旧文档说法 | 实际情况 | 评级 |
|------------|----------|------|
| "TUI 尚未集成真实 LLM 调用（目前是模拟响应）" (`DEV_LOG.md`) | TUI 已调用 `agent.run()`，返回真实 LLM 响应。 | **已修复** |
| "TUI 接入真实 LLM，支持流式响应效果" (`SESSION_SUMMARY.md`) | 此前是字符级模拟流式；**现已改为真实 SSE 流式**。 | **已修复** |
| "clarity-gateway 75% 完成" (`AI_HANDOFF.md`) | Gateway 的 Chat Completions 和 Admin Tools API 已返回真实数据，不再是硬编码占位符。 | **已修复** |
| "已实现 7 个内置工具" (`DEV_LOG.md`)：file_read, file_write, file_edit, bash, powershell, glob, grep | `registry.rs` 实际注册了 **6 个**：FileReadTool、FileWriteTool、FileEditTool、GlobTool、GrepTool、BashTool。没有独立的 PowerShellTool。 | **夸大（未变）** |
| "MCP Engine / McpClient / McpTransport / 支持 SSE/Stdio 传输" (`ARCHITECTURE.md`) | `mcp.rs` 代码结构已完整（~900 行），实现了 stdio transport 和 `McpToolAdapter`，但 SSE transport 尚未实现。 | **部分修复** |
| "人格和记忆在真实对话中的工作效果需验证" (`SESSION_SUMMARY.md`) | 人格系统的 `SystemPromptBuilder` 在 `Agent::run()` 中被调用；`memory_store` 和 `memory_ticker` 已闭环集成。 | **已修复** |
| 规划了 "WASM 插件系统、多平台桥接（Telegram/飞书/QQ/微信）、Tauri 桌面端、书桌系统、云端 OpenClaw 数据迁移" | 代码中无任何相关实现，甚至无设计草图。 | **规划过拟合（未变）** |

---

## 5. 关联技术调研：先进与成熟思路

### 5.1 MCP (Model Context Protocol)：从私有协议到行业标准

**行业现状**：
- Anthropic 于 2024 年 11 月发布 MCP 1.0，截至 2025 年底已有 **5,800+ 服务器、300+ 客户端**，并被 OpenAI、Google、Microsoft、AWS 采纳，最终捐赠给 Linux Foundation 的 Agentic AI Foundation (AAIF) 进行 vendor-neutral 治理[^1][^2]。
- MCP 被比喻为 "USB-C for AI"，核心思想是**将 Agent 与具体工具实现解耦**：开发者写一次 MCP Server，任何兼容 Agent 都能使用。这解决了工具集成从 O(n²) 到 O(n) 的复杂度问题[^3]。

**对 Clarity 的启示**：
- 当前 `clarity-core` 的 `Tool` trait 是一种**私有协议**。每新增一个外部能力都需要修改 `clarity-core` 源码并重新编译。
- **务实路线**：不应继续自建 MCP 客户端/传输层的"轮子"，而应直接接入现有的 Rust MCP SDK 生态：
  - [`rust-mcp-schema`](https://github.com/rust-mcp-stack/rust-mcp-schema)：类型安全的 MCP Schema 实现，同步跟踪官方 spec。
  - [`rust-mcp-sdk`](https://github.com/rust-mcp-stack/rust-mcp-sdk)：基于 `rust-mcp-schema` 的异步 toolkit，可用于快速构建 MCP Client/Server。
- 这样做的好处：直接兼容 community 已有的数千个 MCP servers（文件系统、数据库、GitHub、Slack 等），避免重复造轮子。

### 5.2 Ratatui TUI：异步架构与 Elm-like 模式

**行业最佳实践**：
- Ratatui 是 Rust 终端 UI 的事实标准。高级应用普遍采用 **组件化架构 + 异步事件循环**。
- 社区推荐模式（如 `d-holguin/async-ratatui`）采用 **Elm Architecture**：`Model` 集中状态 + `update` 处理事件 + `view` 负责渲染，后台任务通过 `tokio::spawn` + `mpsc` channel 与主循环通信[^4][^5]。

**对 Clarity 的启示**：
- 当前 `clarity-tui` 已具备 `tokio::spawn` + `mpsc` 异步架构，并已拆分为 `ChatPane`、`InputPane`、`StatusBar` 等组件。
- 下一步可考虑：
  1. 将 `App` 中剩余的业务逻辑（命令解析、Agent 调用启动）进一步解耦为独立 `Controller`。
  2. 引入 theme/config 文件（如 `clarity.toml`），替代硬编码的模型名、人格类型。

### 5.3 Agent Memory Systems：SQLite 本地优先与渐进式披露

**行业现状**：
- 2024–2025 年，AI Agent 的记忆系统形成了相对成熟的三层模型：
  - **Working Memory**：当前对话上下文。
  - **Episodic Memory**：会话历史、时间线。
  - **Semantic Memory**：提取的事实、用户偏好、实体关系。
- 对于**本地优先 (local-first)** 的 Agent，**SQLite + FTS5 + 向量扩展** 已成为主流方案。OpenClaw、claude-mem、fsck.com episodic-memory 等项目均采用此路线[^6][^7][^8]。

**对 Clarity 的启示**：
- `clarity-memory` 已经具备了很好的基础：SQLite + FTS5、`store.rs`、`session_store.rs`、`compiler.rs`（四级编译流水线）、`ticker.rs`、`extractor.rs`。
- 记忆系统与 Agent Loop 已完成初步闭环，但仍有优化空间：
  1. 引入 **渐进式披露**：先检索索引/摘要，再筛选最相关详情注入上下文，以节省 token。
  2. 引入 `sqlite-vec` 扩展，实现轻量级向量相似度搜索。

---

## 6. 差距分析与务实路线图

### 6.1 当前真实基线

用一句话概括：**Clarity 的核心 Agent Loop、真实 SSE 流式、Memory 闭环、Gateway 去 stub、MCP Client 骨架、TUI 组件化均已落地。这是一个可运行的原型，但仍需端到端实测打磨。**

### 6.2 务实的重构与发展路线

#### Phase 1: 核心闭环 ✅ 已完成

1. **Memory × Agent 集成**
   - `Agent::run()` 中已加入 `memory_store.search(query)` 调用，结果注入 System Prompt。
   - 对话结束后调用 `memory_ticker.tick()` 触发编译流水线。

2. **Gateway 去 stub 化**
   - `chat_completions` handler 调用真实 `Agent::run()`。
   - `admin_tools` 动态读取 `ToolRegistry`。

3. **代码清理**
   - `cargo clippy --workspace` 零警告。
   - 删除未使用的 dead code 和旧 widget 文件。

#### Phase 2: 体验打磨 ✅ 大部完成

1. **真实流式响应** ✅
   - `LlmProvider` 新增 `stream()` 方法。
   - TUI 直接消费真实增量 chunk，端到端集成测试通过。

2. **MCP Client 接入** ⚠️ 代码完成，待实测
   - `mcp.rs` 实现了 stdio transport、JSON-RPC 2.0、`McpToolAdapter`。
   - 下一步：与真实 MCP server（如 `npx -y @modelcontextprotocol/server-filesystem`）进行端到端联调。

3. **TUI 组件化** ✅
   - 已拆分为 `ChatPane`、`InputPane`、`StatusBar`、`GeneratingIndicator`。

#### Phase 3: 能力扩展（中期，1–3 个月）

1. **MCP 实测与 SSE transport**
   - 安装官方参考 MCP server 验证 `McpClient`/`McpManager` 端到端能力。
   - 可选：实现 HTTP/SSE transport，使 Clarity 能连接远程 MCP server。

2. **多 Agent 管理**
   - 当前 `clarity-tui` 只创建单个 Agent。应支持在 TUI 中切换不同 Agent Profile（每个 profile 有独立的 config、memory DB、working directory）。

3. **向量检索增强**
   - `clarity-memory` 当前依赖 FTS5。可引入 `sqlite-vec` 扩展，实现轻量级向量相似度搜索，无需外部向量数据库。

4. **Gateway 的 Session 持久化**
   - 将 WebSocket session 与 `clarity-memory` 的 `session_store` 打通，支持跨连接恢复对话历史。

#### 仍不建议在 Phase 3 之前做的事

- ❌ WASM 插件系统：Rust 的 WASM 宿主/插件生态仍较复杂，且当前工具数量极少，没有插件化的实际需求。
- ❌ 多平台桥接（Telegram/飞书/QQ/微信）：需要大量外部 SDK 集成、消息格式适配、并发模型设计。在 Gateway 尚未完成 session 持久化前，这是空中楼阁。
- ❌ Tauri 桌面端：TUI 已足够验证核心交互，Web 界面的优先级应低于 Gateway 的 API 完善。
- ❌ "书桌系统" 等创意功能：属于产品层面的差异化，应在核心框架稳定后再考虑。

---

## 7. 附录

### 7.1 旧文档归档清单

以下文件已移至 `archive/` 目录，保留作为历史参考，但不再作为项目当前状态的权威说明：

| 归档文件 | 原作用 | 归档原因 |
|----------|--------|----------|
| `archive/README.md` | 项目入口说明 | 含夸大描述（如 7 个工具、完整 MCP 支持），已被新 README 替代。 |
| `archive/ARCHITECTURE.md` | 架构设计文档 | 架构图描绘了未实现的组件（Event Bus、完整 MCP Engine、零拷贝设计）。 |
| `archive/DEV_LOG.md` | 开发日志 | 代码量严重夸大，进度描述与代码实际状态脱节。 |
| `archive/HUMAN_GUIDE.md` | 用户操作手册 | 基于过时的"待集成 LLM"假设编写。 |
| `archive/SESSION_SUMMARY.md` | 会话总结 | 将字符级模拟流式描述为"真实流式"，且中长期规划过拟合。 |
| `archive/AI_HANDOFF.md` | AI 交接文档 | 完成度数字（75%–85%）与实际 stub 代码严重不符。 |

### 7.2 参考资料

[^1]: *The Complete Guide to Model Context Protocol (MCP)*, Gupta Deepak, Dec 2025.  
[^2]: *Advancing Multi-Agent Systems Through Model Context Protocol*, arXiv:2504.21030, Mar 2025.  
[^3]: *Agentic Tool Use in Large Language Models*, arXiv:2604.00835, Apr 2026.  
[^4]: *Ratatui: Building Rich Terminal User Interfaces in Rust*, BrightCoding, Sep 2025.  
[^5]: *async-ratatui*, GitHub: d-holguin/async-ratatui, 2024.  
[^6]: *Persistent Memory for AI Agents: Comparing PAG, MEMORY.md and SQLite*, SparkCo.AI, Feb 2026.  
[^7]: *Best AI Agent Memory Systems in 2026: 8 Frameworks Compared*, Vectorize.io, Mar 2026.  
[^8]: *A memory architecture for agentic system*, GitHub Gist: spikelab, Feb 2026.
