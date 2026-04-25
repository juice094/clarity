# Clarity — Personal AI Standard Runtime

> An opinionated, multi-entry AI runtime: plan → execute → monitor → remember.

Clarity is a **personal AI standard runtime** that orchestrates LLMs, tools, and sub-agents across multiple entry points — TUI, desktop GUI, web IDE, headless CLI, and system-tray monitor — with persistent memory, structured planning, and parallel execution.

**Rust drives the core**, **Tauri 2 drives the GUI**, **ratatui drives the TUI** — native performance across all platforms. Zero runtime dependencies: `cargo install` produces a fully working binary.

## Core Differentiators

- **Local-First LLM**: Native GGUF inference via Candle — loads Qwen2/DeepSeek-R1-Distill 7B+ models locally without Ollama or external APIs
- **Plan Mode**: LLM generates a structured execution plan first; runs steps in batch without per-tool approval
- **Hybrid Memory**: SQLite + BM25 + vector search persist across sessions with automatic consolidation
- **Approval System**: Interactive / Yolo / Plan — runtime hot-swappable via GUI or TUI
- **Single-Process Desktop**: Tauri 2 frontend directly embeds the Rust agent core — no separate server process

## Architecture

```
crates/
├── clarity-core      # Agent loop, tools, memory, MCP, subagents
├── clarity-memory    # BM25 + vector hybrid search, chunking
├── clarity-gateway   # Axum HTTP server, Web UI, session store
├── clarity-tauri     # Tauri 2 Desktop + Mobile GUI (React frontend)
├── clarity-tui       # ratatui terminal interface
├── clarity-claw      # System-tray background monitor
├── clarity-wire      # UI↔Agent event bus
└── clarity-headless  # Headless CLI for scripts/CI
```

**Key invariant**: `clarity-core` has zero dependencies on any frontend or network crate. All frontends consume the core through a uniform API.

## Quick Start

```bash
# Install
 cargo install --path crates/clarity-tui      # TUI
 cargo install --path crates/clarity-gateway  # Gateway + Web IDE
 cargo install --path crates/clarity-headless # Headless CLI

# Configure API key (or use local GGUF via --features local-llm)
mkdir -p .clarity
cat > .clarity/user_config.json << 'EOF'
{ "provider": "kimi-code", "api_key": "sk-kimi-..." }
EOF

# Run
clarity-gateway   # http://127.0.0.1:18800
clarity-tui       # in another terminal
```

Supported providers: `kimi`, `kimi-code`, `openai`, `anthropic`, `deepseek`, `ollama`, `local` (GGUF via Candle).

## Development

```bash
cargo test --workspace --lib          # 498+ tests, 0 failed
cargo clippy --workspace --lib --bins --tests -- -D warnings  # zero warnings
cargo run -p clarity-tui
cargo run -p clarity-gateway
```

## Documentation

| Document | Purpose |
|----------|---------|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Code-accurate architecture reference |
| [`AGENTS.md`](AGENTS.md) | Agent development guide, environment, coupling notes |
| [`CHANGELOG.md`](CHANGELOG.md) | Version history |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Future direction |
| [`docs/README.md`](docs/README.md) | Full documentation index |

## License

MIT
