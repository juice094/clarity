# Clarity — Personal AI Standard Runtime

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org)
[![GitHub release](https://img.shields.io/github/v/release/juice094/clarity?logo=github)](https://github.com/juice094/clarity/releases)
[![Issues](https://img.shields.io/github/issues/juice094/clarity)](https://github.com/juice094/clarity/issues)

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
├── clarity-llm        # LLM provider abstraction + built-ins + Candle GGUF
├── clarity-tools      # Built-in tool library (file/shell/web/devkit)
├── clarity-channels   # External message channels
├── clarity-subagents  # Sub-agent executor + parallel scheduler
├── clarity-core       # Agent loop, Approval, Skill, MCP integration
├── clarity-telemetry  # Unified telemetry
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

No API key? Put a `.gguf` model in `~/models/` and select **Local (GGUF)** in settings.

## Development

```bash
cargo test --workspace --lib --exclude clarity-slint
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo fmt --all -- --check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full developer guide.

## Documentation

| Document | Purpose |
|----------|---------|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Code-accurate architecture reference |
| [`docs/architecture/architecture-positioning.md`](docs/architecture/architecture-positioning.md) | Project positioning and ecosystem |
| [`AGENTS.md`](AGENTS.md) | Agent development guide |
| [`CHANGELOG.md`](CHANGELOG.md) | Version history |
| [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md) | Future direction |
| [`docs/planning/PROJECT_STATUS.md`](docs/planning/PROJECT_STATUS.md) | Current status and metrics |

## License

[GNU Affero General Public License v3.0 (or later)](LICENSE) — Copyright (c) 2026 juice094 and contributors.

See the top-level [`README.md`](README.md) §License for the network-copyleft details.

---

## Community & Support

- **Bug reports**: [GitHub Issues](https://github.com/juice094/clarity/issues) — use the bug template.
- **Feature discussions**: [GitHub Discussions](https://github.com/juice094/clarity/discussions).
- **Security issues**: Please see [SECURITY.md](SECURITY.md) and report privately.
- **Contributing**: Read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
