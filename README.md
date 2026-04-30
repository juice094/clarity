<div align="center">

# Clarity

**Rust-native personal AI runtime**

ReAct/Plan agents · MCP ecosystem · BM25+vector memory · Multi-entry (TUI/Web/Tray/Desktop)

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
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

**No API key? No problem.** Place a `.gguf` model in `~/models/` and select **Local (GGUF)** in Settings. Clarity falls back to local inference automatically when offline.

---

## Current Direction

**阶段性目标**：将 Clarity 打造为能替代 Kimi CLI 的本地开发环境，实现 Claw 模式的持续化存储与多角色认知协同。

- ✅ 已具备：Agent 运行时、Approval 工作流、MCP 工具集成、多前端（TUI/egui/Gateway）
- 🔄 进行中：三栏工作台 UI（左侧角色栏 / 顶部实例标签 / 右侧通用工具栏）
- ⏸️ 未实现：跨会话 Agent 状态快照、子 Agent 上下文持久化（IS-1 后端就绪，前端待接入）、多窗口进程隔离、层级信息注入总线

> 详细路线图见 [`docs/ROADMAP.md`](docs/ROADMAP.md)。

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
| **Model Hot-Swap** | Change provider / model in Settings without restart. API keys stored locally, never leave the machine. |
| **i18n** | Chinese / English language switching with persistent preference. |

**Supported providers**: `openai`, `anthropic`, `kimi`, `kimi-code`, `deepseek`, `ollama`, `local` (Candle GGUF). Custom providers via `~/.config/clarity/models.toml` — no code changes required.

---

## Architecture

```
crates/
├── clarity-core      # Agent loop, tools, memory, MCP, subagents
├── clarity-memory    # BM25 + vector hybrid search, chunking, compilation
├── clarity-gateway   # Axum HTTP server, Web UI, session store
├── clarity-egui      # Desktop GUI (eframe/egui) — primary UI stack
# clarity-tauri     # Archived — moved to external backup (see CHANGELOG)
├── clarity-tui       # ratatui terminal interface
├── clarity-claw      # System-tray background monitor
├── clarity-wire      # UI↔Agent event bus (SPMC) + ViewCommand protocol channel
└── clarity-headless  # Headless CLI for scripts/CI
```

**Key invariant**: `clarity-core` has zero dependencies on any frontend or network crate. All frontends consume the core through a uniform API. This is not accidental — it is the architectural boundary that keeps the project maintainable by a single developer.

---

## Development

```bash
# Run the full validation suite (what CI runs)
cargo test --workspace --lib                          # 568 tests, 0 failed, 4 ignored
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

[MIT](LICENSE) — Copyright (c) 2026 juice094 and contributors.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
