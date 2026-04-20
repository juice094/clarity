# Project Clarity

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?logo=rust)](https://www.rust-lang.org)

> **Local-first AI Agent runtime in Rust.**  
> Multi-model, MCP tool ecosystem, and data sovereignty.

---

## Project Status (2026-04-20)

**Phase: Architecture consolidation complete, entering capability integration.**

| Metric | Status | Note |
|--------|--------|------|
| Build | ✅ | `cargo check --workspace` passes |
| Tests | ✅ | **348+** lib + integration tests passed, 0 failed |
| Lint | ✅ | `clippy --workspace --lib --bins --tests` zero warnings |
| Codebase | ~2.9 MB | 99 Rust source files (~27,200 LOC) |
| Binary | ~23 MB | Release `clarity-gateway.exe` |
| Crates | 6 | workspace layout |

### Feature Matrix

| Module | Status | Description |
|--------|--------|-------------|
| **clarity-core / Agent** | ✅ | ReAct loop, tool calling, stream-first responses |
| **clarity-core / Approval** | ✅ | Interactive / Yolo / Plan modes |
| **clarity-core / Compaction** | ✅ | Context compression to prevent token explosion |
| **clarity-core / Subagents** | ✅ | LaborMarket (coder/explore/plan) + Runner; model-aware routing |
| **clarity-core / MCP Client** | ✅ | Stdio/HTTP/SSE transport; auto-injects into `ToolRegistry` via `mcp.json`; Resources + Prompts types |
| **clarity-core / Background Tasks** | ✅ | `DefaultAgentTaskExecutor` runs real Agents in worker pool; supports per-task model selection |
| **clarity-core / LLM Routing** | ✅ | `ModelRegistry` TOML config + `LlmFactory::create(alias)` + runtime hot-swap |
| **clarity-core / Local LLM** | ✅ | Kalosm GGUF inference + LlamaServer HTTP bridge (zero-dependency) |
| **clarity-core / Skill System** | ✅ | Markdown+YAML `SKILL.md` orchestration layer; loader + registry + context builder + tool whitelist |
| **clarity-tui** | ✅ | Terminal UI with mouse scroll, command registry, tab completion, input history, dark theme |
| **clarity-gateway** | ✅ | OpenAI-compatible Chat Completions API with SSE streaming + structured tool events |
| **clarity-gateway / Session Store** | ✅ | SQLite-based session persistence; HTTP Chat Completions supports `session_id` round-trip |
| **clarity-memory** | ✅ | File / SQLite / Hybrid backends; BM25 + FTS5 hybrid search |
| **clarity-wire** | ✅ | Soul-UI broadcast channel |
| Gateway Channels | ⚠️ | Webhook ready; Discord / Telegram temporarily excluded from default build due to upstream `rustls-webpki` advisories |
| Web UI | ✅ | Embedded Web IDE (`chat.html`) with tool-call cards + config modal |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Application Layer                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ clarity-tui │  │clarity-gateway│ │   Web IDE           │  │
│  │  (Terminal) │  │  (HTTP API)   │ │   (chat.html)       │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
└─────────┼────────────────┼──────────────────────────────────┘
          │                │
          ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│                        Core Engine                           │
│                      clarity-core                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │    Agent    │  │ ToolRegistry│  │   LlmProvider       │  │
│  │   (ReAct)   │  │  (Tools)    │  │ (Multi-provider +   │  │
│  │             │  │             │  │  ModelRegistry)     │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
│         │                │                                   │
│  ┌──────▼────────────────▼─────────────────────┐             │
│  │   Skill       - Markdown+YAML orchestration │             │
│  │   Wire        - Soul-UI communication      │             │
│  │   Approval    - Tool-call approval flow    │             │
│  │   Compaction  - Context compression        │             │
│  │   Subagents   - Agent delegation           │             │
│  │   MCP Client  - External tool servers      │             │
│  └─────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│                        Storage Layer                         │
│                     clarity-memory                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  FileStore  │  │ SqliteStore │  │    HybridStore      │  │
│  │  (JSON)     │  │(SQLite+FTS5+│  │  (Cache + Archive)  │  │
│  │             │  │   BM25)     │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Prerequisites

- Rust 1.75+ (`rustup update stable`)
- (Optional) SQLite if you want persistent memory

### Build

```bash
cargo build --workspace --release
```

### Run TUI

```bash
cargo run --package clarity-tui
```

### Run Gateway

```bash
# API server on :18790, admin UI on :18800
cargo run --package clarity-gateway
```

### Run Tests

```bash
cargo test --workspace --lib
cargo test --package clarity-gateway  # includes integration tests
```

---

## Core Concepts

### Agent

The `Agent` is the central ReAct loop executor. It receives user input, calls tools, and streams events via `WireMessage`.

```rust
use clarity_core::{Agent, registry::ToolRegistry};

let agent = Agent::new(ToolRegistry::with_builtin_tools());
let response = agent.run("List files in current directory").await?;
```

### Skill System

Skills are Markdown+YAML files that inject domain-specific instructions and restrict the tool whitelist.

```yaml
---
id: rust-dev
name: Rust Developer
description: Assists with Rust code review and refactoring
tools:
  - file_read
  - file_write
  - bash
---

# Instructions
You are a senior Rust engineer. Prefer `?` over `unwrap()`.
```

Load in TUI with `/skill use rust-dev`.

### MCP (Model Context Protocol)

Configure external tool servers in `~/.config/clarity/mcp.json`:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
    }
  }
}
```

Tools are automatically registered into `ToolRegistry` at startup.

### Session Persistence (Gateway)

HTTP Chat Completions now supports `session_id` for multi-turn conversations:

```bash
curl http://localhost:18790/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "kimi-latest",
    "messages": [{"role":"user","content":"Hello"}],
    "session_id": "my-session-001"
  }'
```

The response will echo back `"session_id": "my-session-001"` and subsequent requests with the same ID will load prior history.

---

## Workspace Layout

| Crate | Description |
|-------|-------------|
| `clarity-core` | Agent engine, tool registry, LLM providers, MCP client, subagents |
| `clarity-tui` | Terminal UI application |
| `clarity-gateway` | HTTP/WebSocket gateway, session store, channel integrations |
| `clarity-memory` | Memory storage backends, chunking, BM25/FTS5 search |
| `clarity-wire` | UI broadcast channel (Soul-UI communication) |
| `clarity-claw` | (Experimental) CLI helpers |

---

## License

MIT License — see [LICENSE](LICENSE) for details.
