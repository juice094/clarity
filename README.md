<div align="center">

# 🦀 Clarity

> **Rust-native personal AI runtime — one core, every surface.**

ReAct agents · MCP tools · BM25+vector memory · Multi-entry (TUI/Desktop/Web/Tray)

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](README.en.md) · [中文](README.zh.md)

</div>

---

## 📋 简介

A single runtime that orchestrates LLMs, tools, and memory across every entry point — terminal, desktop, browser, headless scripts, system tray. Zero external runtime dependencies: no Python, Node.js, or Ollama required.

> **Pre-built installers**: Windows `.msi` / `.exe` on [GitHub Releases](https://github.com/juice094/clarity/releases).

**定位边界**：Clarity 是「本地优先的 AI 开发运行时」，聚焦编码/工程工作流。无原生消息通道（WhatsApp/Telegram/Discord Bot）、无 Voice/Canvas、无移动端。需要多通道 inbox 或语音交互 → [OpenClaw](https://github.com/openclaw/openclaw) 更合适。

---

## 🌟 核心亮点

| 亮点 | 说明 |
|:---|:---|
| 🧠 **Agent 运行时** | ReAct/Plan 循环 + MCP 工具生态，Approval 三层（Interactive/Yolo/Plan） |
| 🖥️ **纯 Rust 多前端** | TUI (ratatui) · Desktop (eframe/egui) · Web IDE (Axum) · Headless CLI · System Tray |
| 🤖 **本地 LLM** | Candle 原生 GGUF 推理，离线自动回退，零外部依赖 |
| 🧩 **混合记忆** | SQLite + BM25 + vector 搜索，6 个月时间衰减，跨会话持久化 |
| 💰 **预算保护** | Per-turn / per-day USD 上限，超预算前自动拦截 |

> [完整技术特性与路线图 → docs/ROADMAP.md](docs/ROADMAP.md)

---

## 🔧 技术栈

| 层级 | 技术 |
|:---|:---|
| Agent 核心 | ReAct/Plan loop, MCP stdio/SSE/WebSocket |
| 本地推理 | Candle (GGUF: Qwen2, DeepSeek-R1-Distill) |
| 记忆存储 | SQLite (WAL) + Tantivy BM25 + 向量搜索 |
| Desktop GUI | eframe/egui (pure Rust, zero web deps) |
| 事件总线 | clarity-wire SPMC channel |

---

## 📁 项目结构

```
crates/
├── clarity-core       # Agent loop, Approval, Skill, MCP
├── clarity-egui       # Desktop GUI (主前端)
├── clarity-tui        # Terminal UI
├── clarity-gateway    # Web IDE (Axum)
├── clarity-llm        # LLM providers + Candle GGUF
├── clarity-memory     # BM25 + vector hybrid search
├── clarity-mcp        # MCP client (stdio/SSE/HTTP/WS)
├── clarity-tools      # Built-in tools
└── clarity-wire       # UI ↔ Agent event bus
```

---

## 🚀 快速开始

```bash
# 1. Clone
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. Build & run Desktop GUI
cargo run -p clarity-egui

# 3. Or install a frontend
cargo install --path crates/clarity-egui  # or clarity-tui / gateway / headless
```

**No API key?** Place a `.gguf` model in `~/models/` → select **Local (GGUF)** in Settings → offline inference.

---

## 🏗️ 架构

```
contract ← {wire, memory, mcp, llm, tools} ← core ← {gateway, egui, tui, claw, headless}
                                                    ↑
                                             subagents (consumes core)
```

`clarity-core` has **zero frontend/network dependencies**. `clarity-contract` has **zero internal dependencies**. Frontend crates never import each other — cross-frontend communication goes through `clarity-wire`.

---

## 🤝 参与贡献

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, architecture map, and coding standards. Quick validation:

```bash
cargo test --workspace --lib              # 927 tests
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo audit --deny unsound --deny yanked
```

---

## 📄 License

[AGPL-3.0](LICENSE) — Copyright (c) 2026 juice094. If you modify Clarity and provide it as a service over a network, you must release your modified source code to all users. For alternative licensing, open an issue.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
