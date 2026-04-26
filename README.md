<div align="center">

# Clarity

**Personal AI Standard Runtime — plan, execute, monitor, remember.**

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-purple.svg)](https://tauri.app)

[English](#readme) | [中文](#readme-zh)

</div>

---

## What & Why

You have a dozen AI tools: chat UIs, coding assistants, task runners, memory plugins. Each owns a slice of your workflow. None owns the whole.

**Clarity is a single runtime that orchestrates LLMs, tools, and memory across every entry point you use** — terminal, desktop, browser, headless scripts, system tray. One agent core, multiple surfaces. Your sessions, memory, and tasks persist and travel with you.

Built in Rust. The core engine and CLI tools ship as single binaries with **no external runtime dependencies** (no Python, Node.js, or Ollama required). The desktop GUI (Tauri 2) uses the system WebView2 engine — pre-installed on Windows 11, auto-downloaded on first run for Windows 10.

> **Pre-built installers**: Windows `.msi` and `.exe` are available on [GitHub Releases](https://github.com/juice094/clarity/releases). No Rust toolchain needed.

---

## 30-Second Quick Start

```bash
# 1. Clone
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. Install a binary (pick one)
cargo install --path crates/clarity-tui       # Terminal UI — zero runtime deps
cargo install --path crates/clarity-gateway   # Web IDE — zero runtime deps
cargo install --path crates/clarity-headless  # CLI for scripts — zero runtime deps

# 3. Run
clarity-tui
```

**Desktop GUI** (requires Node.js for build; runtime needs system WebView):
```bash
cd crates/clarity-tauri/frontend && npm install && npm run dev
# In another terminal:
cargo run -p clarity-tauri
```

**No API key? No problem.** Place a `.gguf` model in `~/models/` and select **Local (GGUF)** in Settings. Clarity falls back to local inference automatically when offline.

---

## Core Capabilities

| Capability | What it means |
|-----------|---------------|
| **Local-First LLM** | Native GGUF inference via Candle. Qwen2, DeepSeek-R1-Distill, and more — no Ollama, no API keys, no network required. |
| **Plan Mode** | LLM writes a structured execution plan first; runs steps in batch without per-tool interruption. |
| **Hybrid Memory** | SQLite + BM25 + vector search. Conversations persist across sessions and auto-consolidate into long-term memory. |
| **Multi-Entry** | Same agent core, five surfaces: TUI (`ratatui`), Desktop GUI (`Tauri 2`), Web IDE (`Axum`), Headless CLI, System Tray (`claw`). |
| **Approval System** | Interactive / Yolo / Plan — switch at runtime from GUI or TUI. |
| **Offline Fallback** | Network monitor probes every 30s. Offline? Auto-switch to local model. Back online? Restore cloud provider. |
| **i18n** | Chinese / English language switching with persistent preference. |

**Supported providers**: `openai`, `anthropic`, `kimi`, `kimi-code`, `deepseek`, `ollama`, `local` (Candle GGUF).

---

## Architecture

```
crates/
├── clarity-core      # Agent loop, tools, memory, MCP, subagents
├── clarity-memory    # BM25 + vector hybrid search, chunking, compilation
├── clarity-gateway   # Axum HTTP server, Web UI, session store
├── clarity-tauri     # Tauri 2 Desktop GUI (React + i18n)
├── clarity-tui       # ratatui terminal interface
├── clarity-claw      # System-tray background monitor
├── clarity-wire      # UI↔Agent event bus (SPMC)
└── clarity-headless  # Headless CLI for scripts/CI
```

**Key invariant**: `clarity-core` has zero dependencies on any frontend or network crate. All frontends consume the core through a uniform API. This is not accidental — it is the architectural boundary that keeps the project maintainable by a single developer.

---

## Development

```bash
# Run the full validation suite (what CI runs)
cargo test --workspace --lib                          # 502 tests, 0 failed
cargo clippy --workspace --lib --bins --tests -- -D warnings  # zero warnings
cargo fmt --all -- --check
cargo audit

# Run individual components
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
| [`docs/execution-plan-v2.md`](docs/execution-plan-v2.md) | Maintainers | Deep project analysis (Cynefin, TOC, Shape Up) |

---

## License

[MIT](LICENSE) — Copyright (c) 2026 juice094 and contributors.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
