<div align="center">

# Clarity

**Rust-native personal AI runtime**

ReAct/Plan agents · MCP ecosystem · BM25+vector memory · Multi-entry (TUI/Web/Tray/Desktop)

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](README.en.md) | [中文](README.zh.md)

</div>

---

## What & Why

You have a dozen AI tools: chat UIs, coding assistants, task runners, memory plugins. Each owns a slice of your workflow. None owns the whole.

**Clarity is a single runtime that orchestrates LLMs, tools, and memory across every entry point you use** — terminal, desktop, browser, headless scripts, system tray. One agent core, multiple surfaces. Your sessions, memory, and tasks persist and travel with you.

Built in Rust. The core engine and CLI tools ship as single binaries with **no external runtime dependencies** (no Python, Node.js, or Ollama required). The desktop GUI (eframe/egui) is a pure Rust implementation with zero web dependencies — no Node.js, no WebView, no Electron.

> **Pre-built installers**: Windows `.msi` and `.exe` are available on [GitHub Releases](https://github.com/juice094/clarity/releases). No Rust toolchain needed.

---

## 30-Second Quick Start

```bash
# 1. Clone
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. Install a binary (pick one)
cargo install --path crates/clarity-egui      # Desktop GUI — zero runtime deps, pure Rust
cargo install --path crates/clarity-tui       # Terminal UI — zero runtime deps
cargo install --path crates/clarity-gateway   # Web IDE — zero runtime deps
cargo install --path crates/clarity-headless  # CLI for scripts — zero runtime deps

# 3. Run
clarity-egui
```

**Desktop GUI** (eframe + egui, pure Rust — no Node.js, no WebView):
```bash
cargo run -p clarity-egui
```

> **Visual design**: Unified Canvas + Floating Cards — deep-black void with semi-transparent glass-morphism panels, ice-blue accent, and Win11-native rounded window corners. No web stack, no Electron.

**No API key? No problem.** Place a `.gguf` model in `~/models/` and select **Local (GGUF)** in Settings. Clarity falls back to local inference automatically when offline.

---

## Current Direction

**阶段性目标**：将 Clarity 打造为能替代 Kimi CLI / Codex CLI 的本地开发环境，实现 Claw 模式的持续化存储与多角色认知协同。

> **定位边界**：Clarity 是「本地优先的 AI 开发运行时」，不是 OpenClaw 的全功能个人助手替代品。核心差异：Clarity 聚焦编码/工程工作流，无原生消息通道（WhatsApp/Telegram/Discord Bot）、无 Voice/Canvas、无移动端。如果你需要多通道 inbox 或语音交互，OpenClaw 更合适。

### 功能就绪清单

- ✅ **Agent 运行时**：ReAct/Plan 循环、Approval 三层（Interactive/Yolo/Plan）、MCP 工具集成
- ✅ **多前端**：TUI（ratatui）、Desktop GUI（eframe/egui）、Web IDE（Axum Gateway）、Headless CLI
- ✅ **本地 LLM**：Candle 原生 GGUF（Qwen2/DeepSeek-R1-Distill），零外部依赖
- ✅ **归一化 UI**：全宽 tab bar、左侧 sidebar（Category + Web Tabs + Tools + Thinking Log + Subagents + Teams + Cron）、统一弹窗风格、Glassmorphism Frame 系统
- ✅ **KimiCLI 兼容层**：`agent.yaml` 声明式配置、工具名映射、子代理定义
- ✅ **可靠性**：LoopDetector、USD/turn 预算上限、凭证脱敏、上下文溢出自动恢复、指数退避重试
- ✅ **Memory**：SQLite + BM25 + vector 混合搜索，6 个月时间衰减
- ✅ **Provider 自适应**：自声明能力（native tool / vision / prompt caching）、ReliableProvider 回退链
- ✅ **Streaming**：DraftEvent 三态流（Clear/Progress/Content）
- ✅ **Local KV Cache**：Sprint 28 交付 `LocalGgufProvider` LCP-based KV 缓存跨 turn 持久化
- ✅ **Jumpy World Model (J6)**：HistoricalPredictor + LlmAugmentedPredictor + HybridPredictor，k-NN 历史预测 + LLM 零样本回退
- ✅ **会话持久化**：跨会话导出/导入（JSON + `rfd` 对话框）、子代理预算进度条（`ProgressBar` + `steps/max_steps`）
- ✅ **后台任务 UI**：Cron Jobs 可折叠 sidebar section（创建/列表/删除/启用开关）、Team 协调面板
- ✅ **Markdown 渲染**：代码块高亮 + 表格渲染（`RenderBlock::Table` + `egui::Grid`，零外部依赖）
- ✅ **运行时健康**：Gateway 状态指示器（实时轮询）、面板级 panic 隔离（`render_safe` + `catch_unwind`）、rapid-Enter debounce + session-delete draft race 修复
- ✅ **CJK 字体子集化**：NotoSansSC-subset.ttf 精确裁剪至 297KB（477 codepoints），消除字重回退
- ✅ **错误气泡增强**：Retry / Switch Model 动作按钮 + 50% 红色玻璃背景 + danger 描边
- ✅ **侧边栏信息架构**：ROLES / LIVE / WORKSPACE / ANALYTICS 分组导航 + clickable_row 交互模式
- ✅ **Phosphor 图标系统**：角色/箭头图标统一使用 Phosphor 字体字形，零 emoji 策略
- ✅ **响应式标题栏**：Provider 胶囊 + Gateway 状态点，窗口缩小时自动降级

### 进行中 / 未实现

- 🔄 **Sprint 37**：`prompt_cache_key` 策略层（SHA-256 稳定 hash + provider 内部可变性）、LSP stdio 客户端、进程级成本旁路通道
- 🔄 J7/J8：Flow 节点扩展（InvokeSkill / PredictCheckpoint）+ SubagentManager 集成（设计完成，编码待开始）
- 🔄 J10：A/B 验证数据集收集（Phase 1 baseline，≥20 条轨迹）
- 🔄 egui 后端 integration 桩（Cron/Team UI → `clarity-core` backend 接线，6 个 TODO）
- ⏸️ 多窗口进程隔离、IPC 传输层（TCP/UDS/Named Pipe）
- ⏸️ 层级信息注入总线、可视化工作流（D2/Mermaid）

> 详细路线图与中间协议层状态见 [`docs/ROADMAP.md`](docs/ROADMAP.md)。

---

## Core Capabilities

| Capability | What it means |
|-----------|---------------|
| **Local-First LLM** | Native GGUF inference via Candle. Qwen2, DeepSeek-R1-Distill, and more — no Ollama, no API keys, no network required. |
| **Plan Mode** | LLM writes a structured execution plan first; runs steps in batch without per-tool interruption. |
| **Hybrid Memory** | SQLite + BM25 + vector search. Conversations persist across sessions and auto-consolidate into long-term memory. |
| **Multi-Entry** | Same agent core, five surfaces: TUI (`ratatui`), Desktop GUI (`eframe/egui`), Web IDE (`Axum`), Headless CLI, System Tray (`claw`). |
| **Approval System** | Interactive / Yolo / Plan — switch at runtime. V1 rule engine auto-approves low-risk tools. |
| **Offline Fallback** | Network monitor probes every 30s. Auto-switch to local model when offline; restore cloud provider on reconnect. |
| **First-Time UX** | Onboarding flow detects missing models, guides download, or prompts cloud provider setup. No manual config required. |
| **Dynamic Prompts** | `SystemPromptBuilder` assembles context-aware prompts (approval notices, offline status, template variables). |
| **Agent Config (YAML)** | Drop an `agent.yaml` in your working directory to declare system prompts, tool whitelists, and subagent refs — KimiCLI-style declarative config, no code changes. |
| **Model Hot-Swap** | Change provider / model in Settings without restart. API keys stored locally, never leave the machine. |
| **Loop Detector** | Output-hash based detection of repetitive tool calls. Upgrades to fatal after 3 identical outputs within a single turn. |
| **Budget Guard** | Per-turn and per-day USD cost estimation with configurable limits. Provider self-reports pricing; blocks over-budget calls before they hit the API. |
| **Credential Scrubbing** | Automatic redaction of API keys, tokens, and passwords from tool results before they enter the message history. |
| **Context Overflow Recovery** | Detects LLM context-length errors, fast-trims oldest tool results, and retries once — no user intervention. |
| **Memory Time Decay** | `search_fulltext` results weighted by age: 6-month-old memories score ≤ 50%. Configurable half-life. |
| **Multi-Format Tool Parser** | Fallback parsing for JSON, XML, MiniMax, and Perl-style tool calls when the provider does not support native `tools` parameter. |
| **Lifecycle Hooks** | `before_tool_call` / `after_tool_call` / `on_llm_input` hooks for inspection, modification, or cancellation. |
| **i18n** | Chinese / English language switching with persistent preference. |

**Supported providers**: `openai`, `anthropic`, `kimi`, `kimi-code`, `deepseek`, `ollama`, `local` (Candle GGUF). Custom providers via `~/.config/clarity/models.toml` — no code changes required.

---

## Architecture

```
crates/
├── clarity-contract   # Shared contract types: LlmProvider/Tool/AgentError traits,
│                      # FederationMessage, CapabilityToken — zero internal deps.
├── clarity-wire       # UI ↔ Agent event bus (SPMC) + ViewCommand protocol channel.
├── clarity-memory     # BM25 + vector hybrid search, chunking, four-level compaction.
├── clarity-mcp        # MCP client — stdio / SSE / HTTP / WebSocket transports.
├── clarity-llm        # LLM provider abstraction + 6 built-in providers + Candle GGUF.
├── clarity-tools      # Built-in tool library (file/shell/web/devkit/…).
├── clarity-subagents  # Sub-agent executor + parallel scheduler.
├── clarity-core       # Agent loop (ReAct/Plan), Approval, Skill, MCP integration.
├── clarity-gateway    # Axum HTTP/WebSocket server, Web UI, session store.
├── clarity-egui       # Desktop GUI (eframe/egui) — primary UI stack.
├── clarity-tui        # ratatui terminal interface.
├── clarity-claw       # System-tray background monitor.
└── clarity-headless   # Headless CLI for scripts / CI.
# clarity-tauri        # Archived — moved to external backup (see CHANGELOG).
```

**Dependency direction**

```
contract  ←  {wire, memory, mcp, llm, tools}  ←  core  ←  {gateway, egui, tui, claw, headless}
                                                   ↑
                                            subagents (consumes core)
```

**Key invariants**

- `clarity-core` has zero dependencies on any frontend or network crate.
- `clarity-contract` has zero internal dependencies; everyone else builds on it.
- Frontend crates never import each other — cross-frontend communication goes through `clarity-wire`.

This is not accidental — it is the architectural boundary that keeps the project maintainable by a single developer.

---

## Development

```bash
# Run the full validation suite (what CI runs)
cargo test --workspace --lib                          # 849 tests, 0 failed, 7 ignored
cargo clippy --workspace --lib --bins --tests -- -D warnings  # zero warnings
cargo fmt --all -- --check
cargo doc --no-deps                                   # zero doc warnings
cargo audit --deny unsound --deny yanked

# Run individual components
cargo run -p clarity-egui
cargo run -p clarity-gateway
cargo run -p clarity-tui
cargo run -p clarity-claw
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide, architecture map, and contribution workflow.

---

## Documentation Index

| Document | Audience | Purpose |
|----------|----------|---------|
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contributors | Setup, architecture, workflow, coding standards |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Developers | Code-accurate architecture reference |
| [`AGENTS.md`](AGENTS.md) | AI agents / Contributors | Environment guide, known issues, coupling notes |
| [`CHANGELOG.md`](CHANGELOG.md) | Users | Version history and migration notes |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Users / Contributors | Future direction and risk assessment |
| [`docs/methodology-shape-up.md`](docs/methodology-shape-up.md) | Maintainers | Engineering methodology (Cynefin, TOC, Shape Up) |

---

## License

[AGPL-3.0](LICENSE) — Copyright (c) 2026 juice094 and contributors.

Clarity is licensed under the GNU Affero General Public License v3.0 (or later). This means:

- **You can use, modify, and distribute** Clarity freely for personal, educational, or research purposes.
- **If you modify Clarity and provide it as a service over a network** (e.g., hosting `clarity-gateway` as a SaaS), you **must** release your modified source code under AGPL-3.0 to all users of that service.
- **You cannot** relicense Clarity or derivative works under proprietary or less-permissive terms.

This license choice reflects our commitment to keeping the project open and preventing closed-source commercialization of community-contributed work. If you need a different licensing arrangement for legitimate use cases, open an issue to discuss.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
