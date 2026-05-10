# Health Audit Report — clarity-gateway/src/handlers/ Split

> Generated: 2026-05-10
> Commit: 7acbc3ce
> Standard: AGENTS.md §9 Architecture Health Discipline

## 1. Compile Check

```
cargo check -p clarity-gateway   ✅ PASSED (0 errors, 0 warnings)
cargo test -p clarity-gateway --lib  ✅ 47 passed, 0 failed
cargo test --workspace --lib  ✅ 833 passed, 0 failed
```

## 2. Module Inventory

| Module | Lines | pub(crate) fn | pub(crate) type | pub fn | pub type | private fn | private type |
|--------|-------|---------------|-----------------|--------|----------|------------|--------------|
| admin | 320 | 7 | 10 | 0 | 0 | 0 | 0 |
| chat | 442 | 0 | 0 | 2 | 6 | 0 | 0 |
| config | 219 | 2 | 2 | 2 | 1 | 3 | 0 |
| cron | 106 | 3 | 4 | 0 | 0 | 0 | 0 |
| files | 310 | 4 | 4 | 0 | 0 | 1 | 0 |
| mcp | 205 | 4 | 3 | 0 | 0 | 0 | 0 |
| memory | 96 | 1 | 3 | 0 | 0 | 1 | 0 |
| mod | 330 | 0 | 0 | 0 | 0 | 0 | 0 |
| sessions | 71 | 3 | 0 | 0 | 0 | 0 | 0 |
| tasks | 377 | 6 | 11 | 0 | 0 | 0 | 0 |

## 3. Per-Module 50-Word README + Extractability

### chat.rs (442 lines)
**README**: Exposes OpenAI-compatible `/v1/chat/completions` and `/health` endpoints. External projects use it to integrate Clarity as a drop-in LLM backend with streaming SSE support and per-request AgentController isolation.
**Alternatives**: `axum-openapi`, `poem-openapi`
**Verdict**: 🔴 NOT extractable — tightly coupled to `AgentController`, `ConversationChatDriver`, and `AppState`.

### admin.rs (320 lines)
**README**: Provides runtime admin APIs for stats, tool discovery, model listing, approval mode, provider switching, and mesh status monitoring. External projects use it to build operator dashboards for LLM runtime governance.
**Alternatives**: `prometheus-client`, `opentelemetry-collector`
**Verdict**: 🟡 PARTIAL — `admin_switch_provider` depends on `LlmFactory` and `MeshLlmProvider`; could extract with trait boundaries.

### tasks.rs (377 lines)
**README**: Background task creation and parallel subagent execution with batch status tracking. External projects use it to fan out agent work across multiple prompts and collect aggregated results.
**Alternatives**: `tokio-task-manager`, `background-jobs`
**Verdict**: 🟡 PARTIAL — Depends on `AppState.agent` and `SubagentManager`; trait abstraction needed.

### config.rs (219 lines)
**README**: TOML-based provider configuration persistence with runtime validation. External projects use it to save and restore LLM provider settings across restarts without hardcoding secrets.
**Alternatives**: `config-rs`, `confy`
**Verdict**: 🟢 EXTRACTABLE — Only depends on `clarity_llm::runtime` and std/fs. No `AppState` coupling.

### files.rs (310 lines)
**README**: Secure file system operations with path sanitization and sensitive-file detection. External projects use it to expose a sandboxed read/write API over a working directory.
**Alternatives**: `tower-http` ServeDir, `vfs`
**Verdict**: 🟢 EXTRACTABLE — `sanitize_path` and `is_sensitive_path` are pure logic; only `AppState` usage is for session store (optional).

### sessions.rs (71 lines)
**README**: Lightweight session CRUD over a JSONL-backed session store. External projects use it to persist conversation history with automatic expiry.
**Alternatives**: `sled`, `redb`
**Verdict**: 🟢 EXTRACTABLE — Thin wrapper around `SessionStore`; trivial to port.

### mcp.rs (205 lines)
**README**: MCP server configuration management (CRUD over `mcp.json`). External projects use it to dynamically register and unregister Model Context Protocol servers.
**Alternatives**: `mcp-sdk-rs`, `mcp-client`
**Verdict**: 🟢 EXTRACTABLE — Only touches `clarity_core::mcp::config`; no agent coupling.

### cron.rs (106 lines)
**README**: Cron task scheduling with expression validation. External projects use it to schedule recurring agent runs via cron syntax.
**Alternatives**: `tokio-cron-scheduler`, `clokwerk`
**Verdict**: 🟢 EXTRACTABLE — Depends only on `clarity_core::background` types.

### memory.rs (96 lines)
**README**: Cross-session full-text memory search over BM25+vector hybrid store. External projects use it to enrich queries with persisted facts from previous sessions.
**Alternatives**: `tantivy`, `meilisearch`
**Verdict**: 🟢 EXTRACTABLE — Pure query wrapper around `clarity_core::memory`.

### mod.rs (330 lines)
**README**: Module orchestrator and integration test harness for all gateway handlers. Holds shared `sanitize_path` security helper and axum router integration tests.
**Verdict**: 🔴 INTENTIONALLY NON-EXTRACTABLE — This is the glue layer.

## 4. Cross-Dependency Map

```
chat      → AppState, AgentController, ConversationChatDriver
admin     → AppState, LlmFactory, MeshLlmProvider
tasks     → AppState, SubagentManager
config    → clarity_llm::runtime (only)
files     → AppState (session_store only), super::sanitize_path
sessions  → AppState
mcp       → clarity_core::mcp::config
cron      → clarity_core::background
memory    → clarity_core::memory
mod.rs    → ALL (tests only)
```

## 5. Coupling Hotspots

| Rank | Coupling | Impact |
|------|----------|--------|
| 1 | `AppState` — 6 submodules depend on it | Blocks independent testing; `AppState` is a god-object |
| 2 | `clarity_core::agent::Agent` — embedded in `AppState` | Forces LLM runtime coupling into HTTP layer |
| 3 | `super::sanitize_path` — `files.rs` → `mod.rs` | Minor; could become a utility crate |

## 6. Overall Verdict

**Split Grade: B+**

- ✅ Single-file god object eliminated (2360 → max 442 lines)
- ✅ Clear topical boundaries (chat/admin/tasks/files/etc.)
- ✅ Zero compiler warnings, all tests pass
- ✅ 5 of 9 submodules are extractable with <4h effort
- ⚠️ `AppState` remains a cross-cutting god-object — next priority is decoupling HTTP handlers from concrete `Agent` via trait
- ⚠️ `chat.rs` uses `pub` visibility (not `pub(crate)`) — leaks into external crate surface

## 7. Next Refactoring Priority

1. **P0**: Introduce `AgentHandle` trait — let handlers depend on a trait instead of concrete `AppState.agent`
2. **P1**: Move `sanitize_path` + `is_sensitive_path` to `clarity-tools` or a new `clarity-utils` crate
3. **P2**: Restrict `chat.rs` visibility from `pub` to `pub(crate)`
4. **P3**: Extract `config.rs`, `mcp.rs`, `cron.rs`, `memory.rs` into standalone mini-crates as proof of decoupling
