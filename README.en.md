# Clarity — Personal AI Standard Runtime

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org)

> **Local-first AI Agent runtime in Rust.**  
> Multi-model, MCP tool ecosystem, and data sovereignty.

Clarity orchestrates LLMs, tools, and sub-agents across TUI, desktop GUI, web IDE, headless CLI, and system-tray monitor — with persistent memory, structured planning, and parallel execution.

## Why Clarity?

| Dimension | Clarity | Typical Node.js / TS Agents |
|-----------|---------|----------------------------|
| **Runtime** | Single binary, `cargo install` | Node.js / Bun runtime required |
| **Memory Safety** | Compile-time guarantees (Rust) | Runtime GC |
| **Process Model** | Single-process (Tauri ↔ Rust core) | Frontend ↔ server dual-process |
| **Memory System** | SQLite + BM25 + vector hybrid | File-based or external DB |
| **Local LLM** | Native GGUF via Candle (no Ollama) | Depends on external runtime |

## Architecture

```
crates/
├── clarity-core      # Agent loop, tools, memory, MCP, subagents
├── clarity-memory    # BM25 + vector hybrid search, chunking
├── clarity-gateway   # Axum HTTP server, Web UI, session store
├── clarity-tauri     # Tauri 2 Desktop + Mobile GUI
├── clarity-tui       # ratatui terminal interface
├── clarity-claw      # System-tray background monitor
├── clarity-wire      # UI↔Agent event bus
└── clarity-headless  # Headless CLI for scripts/CI
```

**Invariant**: `clarity-core` has zero dependencies on any frontend crate.

## Quick Start

```bash
# Install
cargo install --path crates/clarity-tui
cargo install --path crates/clarity-gateway
cargo install --path crates/clarity-headless

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

MIT
