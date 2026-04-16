# Project Clarity

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?logo=rust)](https://www.rust-lang.org)

> **Local-first AI Agent runtime in Rust.**  
> 一个基于 Rust 的本地优先 AI Agent 框架，支持多模型、MCP 工具生态与数据主权。

---

## 📋 Project Status (2026-04-15)

**Phase: Core features landed, entering integration hardening.**

| Metric | Status | Note |
|--------|--------|------|
| Build | ✅ | `cargo check --workspace` passes |
| Tests | ✅ | **252+** passed, 0 failed |
| Lint | ✅ | `clippy --workspace --lib --bins --tests` zero warnings |
| Codebase | ~750 KB | 91 Rust source files |
| Crates | 5 | workspace layout |

### Feature Matrix

| Module | Status | Description |
|--------|--------|-------------|
| **clarity-core / Agent** | ✅ | ReAct loop, tool calling, stream-first responses |
| **clarity-core / Approval** | ✅ | Interactive / Yolo / Plan modes |
| **clarity-core / Compaction** | ✅ | Context compression to prevent token explosion |
| **clarity-core / Subagents** | ✅ | LaborMarket (coder/explore/plan) + Runner |
| **clarity-core / MCP Client** | ✅ | Stdio/HTTP tested E2E with `filesystem` server; auto-injects into `ToolRegistry` via `mcp.json` |
| **clarity-core / Background Tasks** | ✅ | `DefaultAgentTaskExecutor` runs real Agents in worker pool |
| **clarity-tui** | ✅ | Terminal UI with mouse scroll, command registry, tab completion, input history, dark theme |
| **clarity-gateway** | ✅ | OpenAI-compatible Chat Completions API with `stream=true` SSE via `AgentController` |
| **clarity-memory** | ✅ | File / SQLite / Hybrid backends, 57 tests passing |
| **clarity-wire** | ✅ | Soul-UI broadcast channel, 8 tests passing |
| Gateway Channels | ⚠️ | Discord / Telegram / Webhook code present, needs real-world testing |
| Web UI | 📅 | Planned for Phase 3 |

---

## 🏗 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Application Layer                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ clarity-tui │  │clarity-gateway│ │   Future: Web UI    │  │
│  │  (Terminal) │  │  (HTTP API)   │ │   (Planned)         │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
└─────────┼────────────────┼──────────────────────────────────┘
          │                │
          ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│                        Core Engine                           │
│                      clarity-core                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │    Agent    │  │ ToolRegistry│  │   LlmProvider       │  │
│  │   (ReAct)   │  │  (Tools)    │  │ (Multi-provider)    │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
│         │                │                                   │
│  ┌──────▼────────────────▼─────────────────────┐             │
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
│  │  (JSON)     │  │(SQLite+FTS5)│  │  (Cache + Archive)  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## ✨ Core Features

### Agent Engine (`clarity-core`)

- **ReAct Loop**: Full think-act-observe cycle with cancellation support.
- **Stream-first**: Prefers `stream()`, falls back to `complete()` automatically. No double requests.
- **Wire Communication**: Decouples execution (Soul) from UI via broadcast channels.
- **Context Compaction**: Auto-compresses long conversations before token limits are hit.
- **Approval Modes**: Interactive, Yolo, or Plan-level control over dangerous tools.
- **Multi-LLM**: Kimi, Kimi Code, Anthropic, OpenAI-compatible, DeepSeek.
- **Prompt Cache Key**: Session-aware cache routing for supported providers.
- **Personality Hot-swap**: Default `Direct` engineering persona; switch via `/personality [direct|hanako|butter|ming]`.

### Subagent System (`clarity-core/src/subagents/`)

- **LaborMarket**: Type registry for `coder`, `explore`, `plan` subagents.
- **SubagentStore**: State persistence.
- **SubagentBuilder**: Config-driven builder with Git context injection.
- **Runner**: Foreground, background, and resume execution.

### Memory System (`clarity-memory`)

- Backends: File, SQLite (with FTS5), Hybrid.
- `PersistentMemoryStore`: Integrated into `clarity-core`.
- `MemoryTicker`: Threshold-based memory triggers.

### Tool System

- **8 Built-in Tools**: `file_read`, `file_write`, `file_edit`, `glob`, `grep`, `bash`, `web_search`, `web_fetch`.
- **MCP Integration**: Load external MCP servers via `~/.config/clarity/mcp.json`; tools are automatically namespaced and injected into the registry.
- **Tool Approval**: Dangerous ops require confirmation (unless in Yolo mode).

---

## 🚀 Quick Start

### Requirements

- Rust 1.75+
- Windows / Linux / macOS
- Node.js + `npx` (only if you want to use MCP stdio servers like `@modelcontextprotocol/server-filesystem`)

### Build & Test

```bash
cd clarity
cargo build --workspace
cargo test --workspace --lib --tests  # ~252+ tests passing
cargo clippy --workspace              # zero warnings
```

### Run the TUI

```powershell
# Option 1: Kimi Code (recommended for coding tasks)
$env:KIMI_CODE_API_KEY="sk-kimi-your-key"
cargo run -p clarity-tui

# Option 2: Moonshot Open Platform
$env:KIMI_API_KEY="sk-xxx"
cargo run -p clarity-tui

# Option 3: Claude / DeepSeek / OpenAI
$env:ANTHROPIC_AUTH_TOKEN="sk-ant-xxx"
$env:DEEPSEEK_API_KEY="sk-xxx"
$env:OPENAI_API_KEY="sk-xxx"
cargo run -p clarity-tui
```

### TUI Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message / confirm input |
| `Esc` | Return to Normal mode |
| `↑/↓` | Browse history (Input mode) or scroll chat (Normal mode) |
| Mouse wheel | Scroll chat |
| `Tab` | Auto-complete `/` commands |
| `Ctrl+C` | Stop generation (when generating) or return to Normal mode |
| `Ctrl+D` | Quit |
| `/help` | List commands: `/model`, `/stop`, `/clear`, `/personality` |

---

## 🔧 MCP Configuration

Create `~/.config/clarity/mcp.json`:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  }
}
```

On startup, `clarity-tui` will connect to the server and register tools (e.g. `filesystem_read_file`) into the agent's tool registry automatically.

---

## 📚 Documentation

- [`docs/mcp_integration_guide.md`](docs/mcp_integration_guide.md) — MCP design & integration
- [`docs/channel_architecture.md`](docs/channel_architecture.md) — Gateway channel architecture
- [`docs/archive/`](docs/archive/) — Historical phase reports and reality-check documents

---

## 🗓 Roadmap

### Phase 1: Integration Hardening (Current)
- [x] TUI real-LLM validation (Kimi Code / Moonshot)
- [x] Stream-first + Prompt Cache
- [x] Personality refactor (`Direct` persona)
- [x] TUI interaction polish (commands, history, safe Ctrl+C)
- [x] MCP Client + filesystem server E2E
- [x] Gateway Chat Completions SSE streaming
- [x] BackgroundTaskManager real-agent execution
- [ ] Gateway channel end-to-end testing (Discord/Telegram/Webhook)

### Phase 2: Stabilization (Next 2–4 weeks)
- [ ] Error handling polish
- [ ] Performance benchmarks
- [ ] Cross-platform CI matrix
- [ ] English documentation expansion

### Phase 3: Capability Expansion (1–2 months)
- [ ] MCP SSE transport
- [ ] Vector search optimization
- [ ] Multi-agent profile management
- [ ] TUI configuration file support

---

## 🤝 Contributing

Issues and PRs are welcome. If you find a discrepancy between the docs and the code, believe the code — and please open an issue so we can fix the docs.

---

## 📜 License

MIT — see [LICENSE](LICENSE).

---

*Last updated: 2026-04-15*  
*Maintained by the Clarity Team and AI Assistant.*
