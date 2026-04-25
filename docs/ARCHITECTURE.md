# Clarity Architecture

> Code-accurate architecture reference | Last updated: 2026-04-20

---

## 1. Design Principles

| Principle | Implementation |
|-----------|---------------|
| **Single Responsibility** | 8 independent crates, each with one clear role |
| **Dependency Inversion** | `gateway → core`, `tui → core`; `core` knows nothing about frontends |
| **Local-First** | Native GGUF inference via Candle; no external runtime required |
| **Stream-First** | `Agent::run_streaming()` calls `llm.stream()` first, falls back to `complete()` |
| **Zero Runtime Dependencies** | `cargo install` produces a fully working binary |

---

## 2. Crate Topology

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                                │
├──────────┬──────────┬──────────────┬────────────────────────────────────┤
│ Desktop  │  Web     │    TUI       │           Headless / CLI           │
│ (GUI)    │  (IDE)   │  (Terminal)  │           (Scripts/CI)             │
│          │          │              │                                    │
│• Tauri 2 │• Axum   │• ratatui    │• `clarity-headless`               │
│• React 18│• SSE/WS │• crossterm  │• `--prompt` / `--file`            │
│• Single │• static │• commands   │• `--output json/markdown`         │
│  process │  files  │  /plan etc.  │• `--provider local` (GGUF)        │
└─────┬────┴────┬─────┴──────┬───────┴────────────┬───────────────────────┘
      │         │            │                    │
      └─────────┴────────────┴────────────────────┘
                        │
          ┌─────────────┴─────────────┐
          │      clarity-gateway      │
          │  • Axum HTTP server       │
          │  • REST API (/v1/*)       │
          │  • WebSocket (/ws)        │
          │  • Session Store (SQLite) │
          │  • Static file serving    │
          └─────────────┬─────────────┘
                        │
          ┌─────────────┴─────────────┐
          │       clarity-core        │
          │  • Agent (ReAct / Plan)   │
          │  • ToolRegistry           │
          │  • LLM Providers          │
          │  • MCP Client (stdio/SSE) │
          │  • Background Tasks       │
          │  • Subagents / Teams      │
          │  • Skills (Markdown+YAML) │
          │  • Approval (3 modes)     │
          │  • CompactionService      │
          └─────────────┬─────────────┘
                        │
          ┌─────────────┴─────────────┐
          │       Storage Layer       │
          ├──────────┬────────────────┤
          │clarity-  │  clarity-memory│
          │memory    │  (if separate) │
          │          │                │
          │• SQLite  │  • BM25 search │
          │• FTS5    │  • Vector index│
          │• BM25    │  • Chunking    │
          │• File    │  • Compilation │
          └──────────┴────────────────┘
```

### 2.1 Crate Dependency Graph

```
clarity-core
    ├── clarity-memory (BM25, vector, chunking)
    └── clarity-wire   (SPMC event bus)

clarity-gateway ──→ clarity-core
clarity-tauri ────→ clarity-core + clarity-wire
clarity-tui ──────→ clarity-core + clarity-wire
clarity-claw ─────→ clarity-core
clarity-headless ─→ clarity-core
```

**Invariant**: `clarity-core` has **zero** dependencies on any frontend or network crate.

### 2.2 Crate Details

| Crate | Lines (~) | Tests | Key Types |
|-------|-----------|-------|-----------|
| `clarity-core` | ~8,500 | 260+ | `Agent`, `ToolRegistry`, `LlmProvider`, `McpManager`, `BackgroundTaskManager` |
| `clarity-memory` | ~2,800 | 79+ | `SqliteStore`, `HybridStore`, `Chunker`, `MemoryCompiler` |
| `clarity-wire` | ~400 | 8 | `WireMessage`, `WireBroadcaster` |
| `clarity-gateway` | ~3,200 | 43+ | `AppState`, `PersistentSessionStore`, API handlers |
| `clarity-tauri` | ~1,500 + frontend | — | Tauri commands, `LspManager`, `ComputerUse` bridge |
| `clarity-tui` | ~1,800 | 6+ | `App`, `ui()`, command registry |
| `clarity-claw` | ~600 | 6+ | Tray monitor, `notify` watcher |
| `clarity-headless` | ~380 | 10+ | CLI args, `build_provider()` |

---

## 3. Core Modules (clarity-core)

### 3.1 Agent Loop (`src/agent/`)

```
agent/
├── mod.rs           # Agent struct, run(), run_streaming(), run_parallel()
├── controller.rs    # AgentController, Op enum, ControllerEvent
├── plan.rs          # Plan / PlanStep JSON generation + execute_plan()
├── execution.rs     # Tool execution with approval flow
├── state.rs         # AgentState enum (Idle, Running, etc.)
└── compaction.rs    # Context compression to prevent token explosion
```

**Key behavior**: `Agent::run()` is the main ReAct loop. `ApprovalMode::Plan` bypasses per-tool approval by generating a JSON plan first, then executing steps in batch.

### 3.2 Tools (`src/tools/`)

| Tool | Category | Approval | Note |
|------|----------|----------|------|
| `FileReadTool` / `FileWriteTool` / `FileEditTool` | File | Yes (Interactive) | Path traversal protected |
| `BashTool` / `PowerShellTool` | Shell | Yes | `resolve_path()` validates working directory |
| `GlobTool` / `GrepTool` | Search | No | |
| `WebFetchTool` / `WebSearchTool` | Web | No | HTTP fetch + search |
| `WebBrowserTool` | Web | Yes | Lightweight: navigate + get_text/html only |
| `ComputerUseTool` | Desktop | Yes | Python bridge (`computer_bridge.py`) |
| `TaskCreateTool` / `TaskListTool` / `TaskOutputTool` / `TaskStopTool` | Task | No | Real `TaskStore` persistence |
| `TeamCreateTool` / `TeamDeleteTool` / `TeamListTool` | Team | No | |
| `ChannelSendTool` | Notify | No | Slack/Discord/钉钉/飞书/Webhook |
| `PlanTool` | Plan | No | Internal plan generation helper |
| `ThinkTool` / `AskUserTool` | Meta | No | |

### 3.3 LLM Providers (`src/llm/`)

```
llm/
├── api.rs               # LlmProvider trait, Message, StreamDelta
├── mod.rs               # LlmFactory, resolve_local_model_path()
├── model_registry.rs    # ModelRegistry TOML config, ProtocolType
├── sse.rs               # SseParser state machine
├── ollama.rs            # Ollama HTTP API client
├── deepseek.rs          # DeepSeek OpenAI-compatible client
├── kalosm.rs            # Stub — returns error, redirects to LocalGgufProvider
├── local_gguf.rs        # Candle native GGUF inference (feature-gated)
├── llama_server.rs      # Llama.cpp HTTP server bridge
└── openai_compatible.rs # Generic OpenAI-compatible client
```

**Provider matrix**:

| Provider | Cloud | Local | Feature Gate |
|----------|-------|-------|--------------|
| OpenAI | ✅ | ❌ | default |
| Anthropic | ✅ | ❌ | default |
| Kimi (Moonshot) | ✅ | ❌ | default |
| DeepSeek | ✅ | ❌ | default |
| Ollama | ❌ | ✅ | default |
| LocalGguf (Candle) | ❌ | ✅ | `local-llm` |
| LlamaServer | ❌ | ✅ | default |

### 3.4 MCP Client (`src/mcp/`)

- **Stdio transport**: Spawn process, pipe stdin/stdout
- **SSE transport**: HTTP endpoint discovery + reconnect loop
- **HTTP transport**: Direct POST/GET
- **Security**: `validate_mcp_command()` rejects shell metacharacters, relative paths, non-existent absolute paths
- **Auto-loading**: Gateway startup loads `~/.config/clarity/mcp.json`

### 3.5 Background Tasks (`src/background/`)

```
background/
├── mod.rs           # BackgroundTaskManager
├── executor.rs      # DefaultAgentTaskExecutor
├── scheduler.rs     # Priority queue + cron-like scheduling
└── store.rs         # TaskStore (SQLite persistence)
```

Tasks survive TUI/Web closure. `claw` monitors `.clarity/tasks/` via `notify` + OS notifications.

### 3.6 Memory Integration (`src/memory/`)

`clarity-core` imports `clarity-memory`:
- `PersistentMemoryStore` — SQLite + FTS5
- `SharedMemoryTicker` — Session-isolated memory ticker with compile callback
- `MemoryCompiler` — Four-level pipeline: today → week → longterm → facts

---

## 4. Gateway Architecture (clarity-gateway)

### 4.1 Dual-Port Server

| Port | Purpose | Binding |
|------|---------|---------|
| 18790 | Public API | `0.0.0.0` |
| 18800 | Admin + Web UI | `127.0.0.1` only |

### 4.2 API Surface

```
/v1/chat/completions     # OpenAI-compatible SSE streaming
/v1/parallel             # Parallel subagent execution
/v1/tasks                # Background task CRUD
/ws                      # WebSocket real-time events
/api/files/*             # File tree / read / write
/api/tools               # Tool registry introspection
/api/config              # Runtime configuration
/api/approval-mode       # Get/set approval mode
```

### 4.3 Session Store

`PersistentSessionStore` (SQLite):
- CRUD sessions
- Append messages
- Request counting
- Expiration cleanup

---

## 5. Desktop GUI (clarity-tauri)

### 5.1 Architecture

**Single-process**: Tauri 2 frontend directly embeds the Rust core. No separate server process.

```
Tauri Commands ──→ clarity-core Agent
    ├── agent_run_streaming    # SSE-style events (agent:chunk/done/error)
    ├── list_tasks / cancel_task
    ├── computer_screenshot / click / type / scroll
    ├── lsp_start / send / recv / stop / list
    ├── file_tree / file_read / file_write
    └── settings_load / settings_save
```

### 5.2 Frontend Components

| Component | Status | Backend |
|-----------|--------|---------|
| Chat Panel | ✅ | `agent_run_streaming` |
| Session Sidebar | ✅ | JSON file persistence |
| Task Panel | ✅ | Polling + `list_tasks` |
| Settings Panel | ✅ | JSON file persistence |
| File Browser | ✅ | `file_tree` |
| Diff Viewer | ✅ | Frontend-only (React) |
| Computer Use Panel | ✅ | Python bridge (`computer_bridge.py`) |
| LSP Panel | ✅ | `lsp_manager.rs` |

### 5.3 Theme System

- CSS variables dual-theme (`:root` dark + `[data-theme="light"]`)
- `window.matchMedia("prefers-color-scheme: dark")` for Auto mode
- SettingsPanel Cancel restores DOM theme to last saved value

---

## 6. Data Flows

### 6.1 Chat Completion (Streaming)

```
User Input → Gateway/TUI/Tauri → Agent::run_streaming()
    → LlmProvider::stream()
    → SSE deltas (content / reasoning / tool_calls)
    → If tool call: Approval check → ToolRegistry::execute()
    → Tool result → LLM → ... (loop)
    → Final response → Client
```

### 6.2 Plan Mode

```
User Input → Agent::plan()
    → LLM generates JSON Plan (steps[])
    → User approval (Plan mode) or auto-execute (Yolo)
    → Agent::execute_plan()
    → For each step: ToolRegistry::execute() (no per-tool approval)
    → Aggregate results → PlanResult
```

### 6.3 Background Task Lifecycle

```
TaskCreateTool::execute() → TaskStore::create()
    → BackgroundTaskManager picks up pending task
    → Spawns DefaultAgentTaskExecutor in worker pool
    → Agent runs with isolated context
    → TaskStore::update_status() on completion/failure
    → claw detects file change → OS notification
```

---

## 7. Security Model

| Layer | Mechanism |
|-------|-----------|
| Path traversal | `resolve_path()` validates paths stay within working directory |
| Gateway files | `sanitize_path()` restricts to CWD prefix after `canonicalize()` |
| MCP commands | `validate_mcp_command()` rejects metacharacters, relative paths |
| Sensitive files | Auto-detection of `.env`, SSH keys, kubeconfig |
| Tool approval | `requires_approval()` gate for ComputerUse, WebBrowser |
| TLS | reqwest default system TLS (never disabled) |

---

## 8. Extension Points

### 8.1 Add a New Tool

```rust
// crates/clarity-core/src/tools/my_tool.rs
#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "..." }
    fn parameters(&self) -> Value { /* JSON Schema */ }
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        // Implementation
    }
}
```

Register in `ToolRegistry::with_builtin_tools()`.

### 8.2 Add a New LLM Provider

Implement `LlmProvider` trait from `llm/api.rs`:

```rust
#[async_trait]
impl LlmProvider for MyProvider {
    async fn complete(&self, messages: &[Message]) -> Result<LlmResponse, AgentError>;
    async fn stream(&self, messages: &[Message]) -> Result<BoxStream<'_, StreamDelta>, AgentError>;
}
```

Register in `LlmFactory::create()` and `ModelRegistry`.

### 8.3 Add a New MCP Transport

Implement `McpTransport` trait (stdio/SSE/HTTP already exist).

---

## 9. Build & Test

```bash
# Full test suite
cargo test --workspace --lib          # see README.md for current counts

# With local LLM feature
cargo test --workspace --lib --features local-llm   # 502 passed

# Clippy (zero warnings)
cargo clippy --workspace --lib --bins --tests -- -D warnings

# Security audit
cargo audit

# Run entry points
cargo run -p clarity-gateway
cargo run -p clarity-tui
cargo run -p clarity-claw
cargo run -p clarity-headless -- --prompt "Hello" --provider local

# Desktop GUI
cd crates/clarity-tauri/frontend && npm run build
cargo tauri dev
```

---

*This document is the single source of truth for Clarity architecture. If you modify crate boundaries, module structures, or key types, update this file.*
