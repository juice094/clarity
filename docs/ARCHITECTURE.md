# Clarity Architecture

> Code-accurate architecture reference | Last updated: 2026-05-14
> Reflects S3-S7 completion: ViewState state machine + RenderLine pipeline + line-mode feature + focus-aware shortcuts (ADR-011/012/013/014)

---

## 1. Design Principles

| Principle | Implementation |
|-----------|---------------|
| **Single Responsibility** | 8 independent crates; `clarity-core` is a 27k-line god crate pending decomposition |
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
│• egui   │• Axum   │• ratatui    │• `clarity-headless`               │
│  0.31    │• SSE/WS │• crossterm  │• `--prompt` / `--file`            │
│• eframe │• static │• commands   │• `--output json/markdown`         │
│  0.31    │  files  │  /plan etc.  │• `--provider local` (GGUF)        │
│• Tauri 2 │          │              │                                    │
│  archived│          │              │                                    │
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

### 2.1a Code Health Metrics (v0.3.0 baseline)

| Metric | Value | Target |
|--------|-------|--------|
| `unwrap()` / `expect()` (non-test) | ~1,069 | Freeze new; reduce risk-class gradually |
| `pub fn` doc coverage | ~92% | ≥90% |
| clippy warnings | 0 | 0 |
| `unsafe` count | 1 | 0 new |
| Rust tests passed | 849 / 0 failed | 100% |
| `clarity-egui` tests | 66 / 0 failed | Phase 2 baseline injected |
| `cargo doc` warnings | 0 | 0 |

### 2.1 Crate Dependency Graph

```
clarity-core
    ├── clarity-memory (BM25, vector, chunking)
    └── clarity-wire   (SPMC event bus)

clarity-gateway ──→ clarity-core
clarity-egui  ────→ clarity-core + clarity-wire
clarity-tui ──────→ clarity-core + clarity-wire
clarity-claw ─────→ clarity-core
clarity-headless ─→ clarity-core
```

**Reusability rating**:
- `clarity-wire` / `clarity-memory`: **A+** — minimal deps, clean interfaces, ready for crates.io
- `clarity-core`: **B** — strong trait boundaries (`LlmProvider`, `Tool`, `MemoryStore`) but 27k lines and high `unwrap()` density (~1,069) limit downstream reliability
- Application crates (`gateway`, `egui`, `tui`, `claw`, `headless`): **D** — thin shells, not intended as libraries

**Invariant**: `clarity-core` has **zero** dependencies on any frontend or network crate.

### 2.2 Crate Details

| Crate | Lines (~) | Tests | Key Types |
|-------|-----------|-------|-----------|
| `clarity-core` | ~27,000 | 381+ | `Agent`, `ToolRegistry`, `LlmProvider`, `McpManager`, `BackgroundTaskManager` |
| `clarity-memory` | ~2,800 | 79+ | `SqliteStore`, `HybridStore`, `Chunker`, `MemoryCompiler` |
| `clarity-wire` | ~400 | 8 | `WireMessage`, `WireBroadcaster` |
| `clarity-gateway` | ~3,200 | 43+ | `AppState`, `PersistentSessionStore`, API handlers |
| `clarity-egui` | ~4,200 | 66+ | egui app, `ViewState`, panels, widgets, theme, `RenderBlock`→`RenderLine` bridge |
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

### 3.7 UI State Machine (`src/ui/view_state.rs`)

> Introduced in S3 Phase 1.5. Replaces 33+ legacy boolean flags with typed enum aggregates.

```rust
pub struct ViewState {
    pub main: AppView,                    // Chat | Dashboard
    pub left: Option<SidePanel>,          // Sidebar | Workspace
    pub right: Option<SidePanel>,         // Team | Task | Dashboard (mutually exclusive)
    pub modal: Option<ModalType>,         // Approval | Snapshot | Skill | Mcp | ...
    pub turn: TurnState,                  // Idle | Loading | Compacting | Stopping | Restoring
    pub expansion: PanelExpansion,        // per-panel collapse states
}
```

**Enums**:
- `SidePanel` — 7 variants: `Sidebar`, `Workspace`, `Team`, `Task`, `Dashboard`, `PreviewDrawer`, `SubAgentProgress`
- `ModalType` — 12 variants: `Approval`, `Snapshot`, `Login`, `TaskCreate`, `TaskView`, `TeamCreate`, `CronCreate`, `SubAgentView`, `AddProvider`, `KimiCodeLogin`, `Skill`, `Mcp`
- `TurnState` — 5 variants with priority: `Stopping` > `Compacting` > `Loading` > `Restoring` > `Idle`
- `AppView` — `Chat`, `Dashboard`
- `PanelExpansion` — struct bundling 9 `*_expanded` booleans

**Bridge pattern (S3 P1.5.4d)**:
- Forward sync: `view_state` → legacy store booleans (`team_panel_open`, `task_panel_open`, etc.)
- Legacy bools are read-only mirrors; all write-side authority lives in `ViewState`
- Final removal of legacy fields scheduled for P1.5.2 (bridge reversal reversal)

**Key ADRs**: ADR-014 (right-panel Tab consolidation + Skill/Mcp relocation), ADR-013 (focus-aware shortcut routing).

**Detailed docs**: `docs/architecture/viewstate-migration.md` | `docs/architecture/shortcut-focus-routing.md`

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

## 5. Desktop GUI (clarity-egui)

### 5.1 Architecture

**Single-process, immediate-mode**: egui 0.31 + eframe 0.31. No JavaScript runtime; Rust core and UI share memory space.

```
clarity-egui App ──→ clarity-core Agent (same process)
    ├── Chat Panel (virtual list + streaming)
    ├── Session Sidebar (category tree + web tabs + thinking log)
    ├── Workspace Panel (file tree + preview drawer)
    ├── Right Panel (Tab D: Team / Task / Dashboard)
    ├── Settings Panel (provider + local model + approval)
    ├── Command Palette (Ctrl+Shift+P)
    └── Modal stack (Approval / Snapshot / Skill / MCP / ...)
```

State is managed through `ViewState` (see §9) with a forward-sync bridge to legacy store booleans during the S3 transition.

### 5.2 Frontend Panels

| Panel | Status | State Owner |
|-----------|--------|-------------|
| Chat Area | ✅ | `ChatStore` + `SessionStore` |
| Session Sidebar | ✅ | `SessionStore` |
| Workspace (file tree) | ✅ | `UiStore` + fs |
| Right Panel (Tab D) | ✅ | `ViewState.right: Option<SidePanel>` |
| Settings | ✅ | `SettingsStore` + `GuiSettings` |
| Skill Modal | ✅ | `ViewState.modal: Option<ModalType::Skill>` |
| MCP Modal | ✅ | `ViewState.modal: Option<ModalType::Mcp>` |
| Approval Modal | ✅ | `ViewState.modal: Option<ModalType::Approval>` |
| Plan Timeline | ✅ | `UiStore` |
| Command Palette | ✅ | `CommandPalette` widget |

### 5.3 Theme System

- Rust-native `Theme` struct with 40+ tokens (color / spacing / typography / radius / shadow)
- Dark / Light / Auto (follows OS via `window.theme()`)
- Icon font: `lucide-icons` crate (ADR-010); all icons are glyphs, not image assets
- Glassmorphism surfaces via `Frame::new().fill(Color32::from_white_alpha(...))`

### 5.4 RenderLine Pipeline (S4-S7)

**Dual-track rendering** controlled by `line-mode` Cargo feature:

```
line-mode OFF: Message::parsed → Vec<RenderBlock> → message_bubble() → per-message card
line-mode ON:  Message::lines  → Vec<RenderLine>  → render_lines() → row-atoms
```

**13-variant `RenderLine` enum** (ADR-012): `Text`, `CodeLine`, `ToolCallHeader`, `ToolCallArg`, `Thinking`, `ApprovalPrompt`, `StatusLine`, `ArtifactRef`, `CrossInstanceRef`, `SlashCompletion`, `StreamingCursor`, `Divider`, `Empty`, `BlockSlot`.

**Virtual scrolling**: `LineViewport::visible_range(scroll_offset, viewport_height, line_height)` computes a half-open `[start, end)` index range; egui `render_lines()` skips invisible rows to maintain 60 fps at 10K lines.

**Keyboard navigation** (S7): `LineCursor` with j/k/g/G bindings; focus-scoped via `FocusScope::Panel(ChatStream)`.

**Escape hatch**: `RenderLine::BlockSlot` delegates un-line-atomisable blocks (tables, images) to the legacy `RenderBlock` pipeline.

**Detailed docs**: `docs/architecture/renderline-pipeline.md` | `docs/architecture/ui-axis.md`

---

## 6. Data Flows

### 6.1 Chat Completion (Streaming)

```
User Input → Gateway/TUI/egui → Agent::run_streaming()
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

### 6.4 MCP End-to-End

```
Gateway startup / Config load → McpConfig::load_default()
    → For each enabled server: McpClientBuilder::from_mcp_entry()
    → McpClientInstance (Stdio / Http / Sse)
    → McpRegistry::register(name, client)
    → registry.connect_all() → initialize handshake
    → McpManager::sync_tools() → ToolRegistry::register(mcp_tools)
    → Agent loop: ToolRegistry::execute("mcp_tool_name")
        → McpRegistry::get() → client.call_tool()
        → McpError / ToolCallResult → scrub_credentials()
        → ToolResult back to Agent
```

### 6.5 Memory Compaction

```
Agent::run() turn end → TurnContext.messages grows
    → MemoryTicker::notify_turn() threshold reached
    → MemoryCompiler::compile_all()
        ├── Today: summarize last 24h → append to week
        ├── Week: aggregate 7 days → compress to long-term
        ├── Long-term: delta compression → dedup via SHA256 fingerprint
        └── Facts: LLM-powered extraction → SQLite + FTS5
    → MemoryStore::save() / SessionStore::append_summary()
    → Next turn: relevant facts injected via retrieve_facts()
```

### 6.6 Plan-Parallel Execution

```
User Input → Agent::plan() → LLM generates JSON Plan (steps[])
    → Approval (Plan mode) or auto-execute (Yolo)
    → ParallelExecutor::execute_plan(plan)
        → For each independent step:
            ├── SubagentRegistry::build_subagent(type, spec)
            ├── LaborMarket::resolve_type() → registered builder
            └── spawn on tokio task
        → FuturesUnordered / Semaphore(max_concurrency)
        → Aggregate SubagentResult[]
    → PlanResult (ordered merge of parallel outputs)
    → Final response → Client
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
| TLS | `rustls-tls` (pure Rust); `openssl` eliminated from dependency tree |

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
cargo run -p clarity-egui
cargo run -p clarity-gateway
cargo run -p clarity-tui
cargo run -p clarity-claw
cargo run -p clarity-headless -- --prompt "Hello" --provider local

# Desktop GUI (egui — sole stack; Tauri archived)
cargo run -p clarity-egui
```

---

*This document is the single source of truth for Clarity architecture. If you modify crate boundaries, module structures, or key types, update this file.*

---

## Update Log

| Date | Change | Trigger |
|------|--------|---------|
| 2026-04-26 | Initial version | v0.3.0 release audit |
| 2026-05-14 | Tauri → egui as sole UI stack; added §3.7 `ViewState`; updated test counts; deprecated Tauri build commands | S3 Phase 1.5 state-machine migration (ADR-014) |
