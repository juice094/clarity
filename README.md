<div align="center">

# 🦀 Clarity

**Rust-native personal AI runtime**

ReAct/Plan agents · MCP ecosystem · BM25+vector memory · Multi-entry (TUI/Web/Tray/Desktop)

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](README.en.md) | [中文](README.zh.md)

</div>

---

## 📋 项目简介

You have a dozen AI tools: chat UIs, coding assistants, task runners, memory plugins. Each owns a slice of your workflow. None owns the whole.

**Clarity is a single runtime that orchestrates LLMs, tools, and memory across every entry point you use** — terminal, desktop, browser, headless scripts, system tray. One agent core, multiple surfaces. Your sessions, memory, and tasks persist and travel with you.

Built in Rust. The core engine and CLI tools ship as single binaries with **no external runtime dependencies** (no Python, Node.js, or Ollama required). The desktop GUI (eframe/egui) is a pure Rust implementation with zero web dependencies.

> **Pre-built installers**: Windows `.msi` and `.exe` are available on [GitHub Releases](https://github.com/juice094/clarity/releases). No Rust toolchain needed.

### 定位边界

**阶段性目标**：将 Clarity 打造为能替代 Kimi CLI / Codex CLI 的本地开发环境，实现 Claw 模式的持续化存储与多角色认知协同。

Clarity 是「本地优先的 AI 开发运行时」，不是 OpenClaw 的全功能个人助手替代品。核心差异：Clarity 聚焦编码/工程工作流，无原生消息通道（WhatsApp/Telegram/Discord Bot）、无 Voice/Canvas、无移动端。如果你需要多通道 inbox 或语音交互，OpenClaw 更合适。

---

## 🎯 技术特性

| 特性 | 说明 | 状态 |
|:---|:---|:---:|
| **Agent 运行时** | ReAct/Plan 循环，Approval 三层（Interactive/Yolo/Plan），MCP 工具集成 | ✅ |
| **多前端** | TUI（ratatui）、Desktop GUI（eframe/egui）、Web IDE（Axum Gateway）、Headless CLI、System Tray（claw） | ✅ |
| **本地 LLM** | Candle 原生 GGUF（Qwen2 / DeepSeek-R1-Distill），零外部依赖 | ✅ |
| **混合记忆** | SQLite + BM25 + vector 搜索，6 个月时间衰减，跨会话持久化 | ✅ |
| **离线回退** | 网络监测 30s 探针，自动切换本地模型；恢复后自动切回云端 | ✅ |
| **预算保护** | Per-turn / per-day USD 成本上限，Provider 自报价，超预算前拦截 | ✅ |
| **凭证脱敏** | 自动红屏 API key、token、password，阻止进入消息历史 | ✅ |
| **上下文恢复** | 检测到 LLM 上下文长度错误，快速修剪最旧 tool results，自动重试一次 | ✅ |
| **循环检测** | Output-hash 重复检测，单轮内 3 次相同输出升级 fatal | ✅ |
| **模型热切换** | Settings 内切换 provider / model，无需重启，API key 本地存储 | ✅ |
| **KimiCLI 兼容** | `agent.yaml` 声明式配置，工具名映射，子代理定义 | ✅ |
| **Jumpy World Model** | HistoricalPredictor + LlmAugmentedPredictor + HybridPredictor，k-NN + LLM 回退 | ✅ |
| **归一化 UI** | 全宽 tab bar、左侧 sidebar、统一弹窗、Glassmorphism Frame、Markdown 渲染 | ✅ |
| **i18n** | 中英语言切换，持久化偏好 | ✅ |

> 详细路线图与中间协议层状态见 [`docs/ROADMAP.md`](docs/ROADMAP.md)。

---

## 📁 项目结构

```
crates/
├── clarity-contract   # 共享契约：LlmProvider/Tool/AgentError traits, FederationMessage
├── clarity-wire       # UI ↔ Agent 事件总线 (SPMC) + ViewCommand 协议通道
├── clarity-memory     # BM25 + vector 混合搜索，chunking，四级压缩
├── clarity-mcp        # MCP 客户端 — stdio / SSE / HTTP / WebSocket
├── clarity-llm        # LLM provider 抽象 + 6 内置 provider + Candle GGUF
├── clarity-tools      # 内置工具库（file/shell/web/devkit/…）
├── clarity-subagents  # 子代理执行器 + 并行调度器
├── clarity-core       # Agent 循环 (ReAct/Plan)，Approval，Skill，MCP 集成
├── clarity-gateway    # Axum HTTP/WebSocket server，Web UI，session store
├── clarity-egui       # Desktop GUI（主 UI 栈）
├── clarity-tui        # ratatui 终端界面
├── clarity-claw       # 系统托盘后台监控
└── clarity-headless   # Headless CLI（脚本 / CI）
# clarity-tauri        # Archived — moved to external backup
```

---

## 🚀 快速开始

### 环境要求

- Rust 1.85+（如从源码构建）
- 或直接从 [GitHub Releases](https://github.com/juice094/clarity/releases) 下载预构建安装包

### 从源码安装

```bash
# 1. Clone
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. 安装一个前端（pick one）
cargo install --path crates/clarity-egui      # Desktop GUI — zero runtime deps
cargo install --path crates/clarity-tui       # Terminal UI
cargo install --path crates/clarity-gateway   # Web IDE
cargo install --path crates/clarity-headless  # Headless CLI

# 3. Run
clarity-egui
```

**Desktop GUI** (eframe + egui, pure Rust):
```bash
cargo run -p clarity-egui
```

> **Visual design**: Unified Canvas + Floating Cards — deep-black void with semi-transparent glass-morphism panels, ice-blue accent, and Win11-native rounded window corners.

**No API key? No problem.** Place a `.gguf` model in `~/models/` and select **Local (GGUF)** in Settings. Clarity falls back to local inference automatically when offline.

---

## 🏗️ 架构说明

### 依赖方向

```
contract  ←  {wire, memory, mcp, llm, tools}  ←  core  ←  {gateway, egui, tui, claw, headless}
                                                   ↑
                                            subagents (consumes core)
```

### 关键不变量

- `clarity-core` has **zero dependencies** on any frontend or network crate.
- `clarity-contract` has **zero internal dependencies**; everyone else builds on it.
- Frontend crates **never import each other** — cross-frontend communication goes through `clarity-wire`.

This is not accidental — it is the architectural boundary that keeps the project maintainable by a single developer.

---

## 🔧 开发验证

```bash
# Run the full validation suite (what CI runs)
cargo test --workspace --lib                          # 927 tests
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo fmt --all -- --check
cargo doc --no-deps
cargo audit --deny unsound --deny yanked

# Run individual components
cargo run -p clarity-egui
cargo run -p clarity-gateway
cargo run -p clarity-tui
cargo run -p clarity-claw
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide, architecture map, and contribution workflow.

---

## 📚 文档索引

| 文档 | 受众 | 用途 |
|:---|:---|:---|
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contributors | Setup, architecture, workflow, coding standards |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Developers | Code-accurate architecture reference |
| [`AGENTS.md`](AGENTS.md) | AI agents / Contributors | Environment guide, known issues, coupling notes |
| [`CHANGELOG.md`](CHANGELOG.md) | Users | Version history and migration notes |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Users / Contributors | Future direction and risk assessment |
| [`docs/methodology-shape-up.md`](docs/methodology-shape-up.md) | Maintainers | Engineering methodology (Cynefin, TOC, Shape Up) |

---

## 📄 License

[AGPL-3.0](LICENSE) — Copyright (c) 2026 juice094 and contributors.

If you modify Clarity and provide it as a service over a network, you must release your modified source code under AGPL-3.0 to all users of that service. For alternative licensing, open an issue to discuss.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
