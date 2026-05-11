# Clarity — Personal AI Standard Runtime

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org)

> **Local-first AI Agent runtime in Rust.**  
> Multi-model, MCP tool ecosystem, and data sovereignty.

Clarity orchestrates LLMs, tools, and sub-agents across TUI, desktop GUI, web IDE, headless CLI, and system-tray monitor — with persistent memory, structured planning, and parallel execution.

## Why Clarity?

| Dimension | Clarity | Typical Node.js / TS Agents |
|-----------|---------|----------------------------|
| **Runtime** | Single binary, `cargo install` | Node.js / Bun runtime required |
| **Memory Safety** | Compile-time guarantees (Rust) | Runtime GC |
| **Process Model** | Single-process (eframe ↔ Rust core) | Frontend ↔ server dual-process |
| **Memory System** | SQLite + BM25 + vector hybrid | File-based or external DB |
| **Local LLM** | Native GGUF via Candle (no Ollama) | Depends on external runtime |

## Architecture

```
crates/
├── clarity-contract   # Shared trait/types contract (zero internal deps)
├── clarity-wire       # UI ↔ Agent event bus (SPMC) + ViewCommand channel
├── clarity-memory     # BM25 + vector hybrid search, chunking, compaction
├── clarity-mcp        # MCP client (stdio / SSE / HTTP / WebSocket)
├── clarity-llm        # LLM provider abstraction + 6 built-ins + Candle GGUF
├── clarity-tools      # Built-in tool library (file/shell/web/devkit)
├── clarity-subagents  # Sub-agent executor + parallel scheduler
├── clarity-core       # Agent loop, Approval, Skill, MCP integration
├── clarity-gateway    # Axum HTTP/WebSocket server, Web UI, session store
├── clarity-egui       # Desktop GUI (eframe/egui) — primary UI stack
├── clarity-tui        # ratatui terminal interface
├── clarity-claw       # System-tray background monitor
└── clarity-headless   # Headless CLI for scripts/CI
```

**Invariant**: `clarity-core` has zero dependencies on any frontend or network crate. `clarity-contract` has zero internal deps. Frontends never import each other — they cross-talk through `clarity-wire`.

## Quick Start

```bash
# Install
cargo install --path crates/clarity-egui
cargo install --path crates/clarity-tui
cargo install --path crates/clarity-gateway
cargo install --path crates/clarity-headless

# Run Desktop GUI (pure Rust, zero Node.js deps)
cargo run --package clarity-egui

# Run TUI
cargo run --package clarity-tui

# Run Gateway (API on :18790, admin UI on :18800)
cargo run --package clarity-gateway
```

## Development

```bash
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests -- -D warnings
```

## Documentation

| Document | Purpose |
|----------|---------|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Code-accurate architecture reference |
| [`AGENTS.md`](AGENTS.md) | Agent development guide |
| [`CHANGELOG.md`](CHANGELOG.md) | Version history |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Future direction |

## License

[GNU Affero General Public License v3.0 (or later)](LICENSE) — Copyright (c) 2026 juice094 and contributors.

See the top-level [`README.md`](README.md) §License for the network-copyleft details.
