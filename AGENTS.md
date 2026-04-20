# Agent Guidance for Project Clarity

## Quick Reference

```bash
cd C:\Users\<user>\Desktop\clarity
cargo test --workspace --lib          # 334+ tests
cargo clippy --workspace --lib --bins --tests  # zero warnings
cargo run -p clarity-tui               # run TUI (needs API key)
cargo run -p clarity-gateway           # run Gateway (needs API key)
```

## Environment Variables for LLM

```powershell
# Kimi Code (programming plan, keys starting with sk-kimi-)
$env:KIMI_CODE_API_KEY="sk-kimi-..."

# Moonshot Open Platform
$env:KIMI_API_KEY="sk-..."

# Anthropic / DeepSeek / OpenAI
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="..."
$env:OPENAI_API_KEY="..."
```

## Recent Major Changes (2026-04-20)

1. **Agent loop Skill integration (Phase 3)**:
   - `Agent` now holds an optional `SkillRegistry` and a shared `active_skill` (`Arc<RwLock<Option<String>>>`).
   - `build_system_prompt()` automatically injects the active skill's context (name, description, allowed tools, Markdown body) into the system prompt.
   - `filter_tools_value()` enforces the skill's tool whitelist in `run_sync_loop`, `run()`, `run_with_messages_sync()`, and `run_streaming_loop()`.
   - TUI commands `/skill list` and `/skill use <id>` are live; `/skill` without args shows the current active skill.
   - `SkillRegistry` is loaded at TUI startup from `./skills/` or `~/.config/clarity/skills/` (empty registry if no directory found).
   - Three built-in example skills shipped: `deploy-rust-service`, `code-review`, `bug-investigate`.

2. **Architecture consolidation — cyclic dependency decoupling**:
   - Extracted `ToolCall` + `FunctionCall` to `clarity-core/src/types.rs`, breaking `agent ↔ approval` cycle.
   - Extracted `Message`/`MessageRole`/`LlmProvider`/`LlmResponse`/`StreamDelta` to `clarity-core/src/llm/api.rs`, breaking `agent ↔ llm` cycle.
   - Fixed `compaction.rs` imports to break `agent ↔ compaction` cycle.
   - All modules now import from canonical locations; `agent/mod.rs` and `llm/mod.rs` re-export for backwards compatibility.

2. **SSE parsing extraction**: New `clarity-core/src/llm/sse.rs` (~150 LOC) contains a dedicated `SseParser` state machine that assembles content deltas, reasoning_content deltas (Kimi), and tool call deltas incrementally. `llm/mod.rs` reduced from ~1212 to ~970 LOC.

3. **Run-loop deduplication**: Extracted `Agent::run_sync_loop()` as the shared core loop called by both `run()` and `run_with_messages_sync()`, eliminating ~120 lines of near-duplicate logic.

4. **BM25 + Hybrid Search**: `SqliteStore::search_similar` now uses FTS5 for recall followed by in-memory BM25 re-ranking. This replaces the previous O(n) full-table scan + TF-IDF rebuild. 63 + 264 tests passing.

5. **RAG Chunking**: New `Chunker` in `clarity-memory/src/chunking.rs` supports configurable chunk size, overlap, separator, and source tracking (`Chunk::source_id`).

6. **Gateway Session Store**: New `clarity-gateway/src/session_store.rs` provides SQLite-backed session persistence (CRUD, message append, request counting, expiration cleanup).

7. **Skill system overhaul**:
   - **Deleted** old trait-based `skill/` module (`ThinkSkill`, `TodoSkill`, `SkillRegistry` HashMap) — 54 orphaned tests removed.
   - **Renamed** `Agent::get_skill_definitions()` → `get_tool_descriptions()` to eliminate terminology confusion.
   - **New** Markdown+YAML `SKILL.md` orchestration layer in `clarity-core/src/skills/`:
     - `SkillLoader` parses YAML frontmatter + Markdown body.
     - `SkillRegistry` is thread-safe (tokio::sync::RwLock), read-only after load, with keyword search.
     - `Skill::build_context()` generates system-prompt injection text with allowed tool whitelists.
     - Built-in examples: `deploy-rust-service`, `code-review`, `bug-investigate`.
     - 12 new tests passing.

8. **Code quality fixes**:
   - `std::io::Error::new(ErrorKind::Other, ...)` → `std::io::Error::other(...)` (12 occurrences).
   - `&PathBuf` → `&Path` in `plan.rs`.
   - Eliminated wildcard-in-or-patterns in `task.rs`.
   - Fixed sqlite.rs move-borrow error in `add_fact`.

## Previous Major Changes (2026-04-17)

- **Security hardening — MCP command validation**: `validate_mcp_command()` now runs before spawning any MCP stdio server. Rejects shell metacharacters, relative paths, and non-existent absolute paths. Override via `CLARITY_MCP_ALLOWLIST` env var.
- **Dependabot alerts resolved**: `cargo audit` now reports **0 vulnerabilities**. Updated `rustls-webpki` → 0.103.12, `rand` → 0.8.6/0.9.4, and 40+ transitive deps. `discord`/`telegram` features temporarily removed from `clarity-gateway` default build due to upstream `serenity`/`rustls-webpki` CVEs.
- **Fixed tool-calling pipeline in Gateway chat**:
  - `get_tool_descriptions()` now correctly parses `ToolRegistry::get_tool_schemas()` array format.
  - `OpenAiCompatibleLlm` (`complete` + `stream`) now correctly forwards `tool_calls` and `tool_call_id` fields.
  - Disabled Kimi `thinking` mode to prevent `400 Bad Request` on multi-round tool calls.
  - Added `Op::ConversationTurn(Vec<Message>)` to `AgentController`.
- **MCP auto-loading is live**: Gateway startup automatically loads `~/.config/clarity/mcp.json`.
- **Personality system integrated**: `Direct` engineering mode is the default.
- **Stream-first LLM architecture**: `Agent::run_streaming()` calls `llm.stream()` first and only falls back to `complete()`.

## Architecture Notes & Coupling Warnings

> **Status update (2026-04-20):** Several previously flagged coupling issues have been resolved. Remaining items are tracked below.
>
> ### Resolved ✅
> - ~~`agent ↔ approval` cycle~~ — Fixed by extracting `ToolCall`/`FunctionCall` to `types.rs`.
> - ~~`agent ↔ llm` cycle~~ — Fixed by extracting `Message`/`LlmProvider`/`LlmResponse`/`StreamDelta` to `llm/api.rs`.
> - ~~`agent ↔ compaction` cycle~~ — Fixed by correcting import paths in `compaction.rs`.
> - ~~`run()` / `run_with_messages_sync()` duplication~~ — Fixed by extracting `Agent::run_sync_loop()`.
> - ~~Inline SSE parsing in `OpenAiCompatibleLlm`~~ — Fixed by extracting `llm/sse.rs` (`SseParser`).
>
> ### Remaining ⚠️
> 1. **`clarity-core` ↔ `clarity-gateway`**: `AgentController` lives in `core`, but its `Op` enum (`Op::ConversationTurn`) had to be extended to support Gateway's OpenAI-compatible message history. Gateway-driven requirements can still ripple back into core agent abstractions.
> 2. **`Agent::run_streaming` vs `run_streaming_with_messages`**: Two public entry points remain. Consider extracting a pure "agent loop" trait in future refactors to avoid duplicating compaction / wire / memory logic.
> 3. **`AppState` bloat**: `AppState` currently carries `agent`, `session_manager`, `tool_registry`, and `task_manager`. The `tool_registry` field is actually redundant because `agent.registry()` already holds it (kept for the admin API convenience).
> 4. **`std::sync::RwLock` in async contexts**: `Agent.llm` field uses `Arc<std::sync::RwLock<...>>`. This is currently safe (write locks are brief and rare), but should migrate to `tokio::sync::RwLock` for correctness.
>
> **Recommendation for future refactors**: Extract a `ChatDriver` or `ConversationEngine` trait from `Agent` so that `Gateway` and `TUI` can inject their own message-building strategies without modifying core enums.

## Security Notes

- **MCP stdio command validation is active** (since 2026-04-17). Before spawning any MCP server, Clarity validates the `command` field:
  - Shell metacharacters and `..` sequences are rejected.
  - Relative paths are rejected.
  - Absolute paths must exist and point to a file.
  - Bare names (e.g. `npx`, `uvx`) are allowed and resolved via `PATH`.
  - Override with the `CLARITY_MCP_ALLOWLIST` environment variable (comma-separated absolute paths or prefixes).

## Known Issues

- ~~Personality system produces verbose `<mood>` XML metadata~~ **Fixed** by `Direct` mode.
- ~~MCP client is skeletal~~ **Fixed** — stdio/HTTP transport and dynamic registration are working.
- ~~Web UI missing~~ **Fixed** — Gateway serves an embedded Web IDE (`chat.html`) with Monaco Editor and SSE streaming.
- ~~`agent ↔ approval` / `agent ↔ llm` / `agent ↔ compaction` cyclic dependencies~~ **Fixed** (2026-04-20).
- ~~Old Skill system dead code~~ **Fixed** — removed `skill/` module; new `skills/` orchestration layer landed.
- ~~Gateway SSE does not forward `tool_calls` deltas to the client~~ **Fixed** (2026-04-20) — SSE now emits structured events: `ToolCallStart` (with `id`, `name`, `arguments`), `ToolResult` (with `id`, `result`), and `StepBegin` (with `tool_name`).
- **kalosm local Provider not yet integrated**: Skeleton file planned; real implementation blocked until agri-paper delivers 7B model benchmark data.
- **Discord/Telegram channels disabled by default**: Blocked by upstream `rustls-webpki` CVEs in `serenity 0.12.5`. Re-enable when upstream publishes a fix.
- ~~Skill system not yet wired into Agent loop~~ **Fixed** (2026-04-20) — `Agent` now holds `skill_registry` and `active_skill`. `build_system_prompt()` injects skill context; `filter_tools_value()` enforces tool whitelists. TUI commands `/skill list` and `/skill use <id>` are live.
- **Gateway HTTP Chat Completions is stateless by default**: While WebSocket already has full session support, the HTTP `/v1/chat/completions` endpoint now supports optional `session_id` persistence (load history + save turn). If no `session_id` is provided, a new one is auto-generated and returned in the response.

## Code Style

- Rust edition 2021, `tokio` full, `ratatui` 0.24, `axum` 0.7.
- Prefer minimal changes; keep diffs small.
- When modifying `agent/mod.rs` or `llm/mod.rs`, run the full test suite before committing.
- When modifying `AgentController` or `Op`, check all callers in `clarity-tui`, `clarity-gateway`, and integration tests.
