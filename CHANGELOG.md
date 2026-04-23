# Changelog

## [0.1.1] — 2026-04-23

### Security

- **Directory Traversal Fix** — `resolve_path()` in `clarity-core` now validates that resolved paths stay within the working directory, preventing `..` escapes and absolute-path traversal.
- **Gateway Path Sanitization** — `sanitize_path()` in `clarity-gateway` restricts all file API access to the current working directory prefix after `canonicalize()`.

### Added

- **Crate Documentation** — README + `AGENTS.md` for all 6 crates (`clarity-core`, `clarity-gateway`, `clarity-memory`, `clarity-wire`, `clarity-tui`, `clarity-claw`).
- **Gateway Handler Tests** — 8 mock integration tests covering `health_check`, `file_tree`, `file_read`, `admin_tools`, `admin_get_approval_mode`, `admin_set_approval_mode`.
- **Claw & TUI Lib Split** — Binary crates now expose testable `lib.rs` modules with 6 tests each (tooltip formatting, gateway URL resolution, command parsing).
- **CI Hardening** — `cargo audit` (security scan) and `cargo tarpaulin` (coverage report) jobs added to GitHub Actions.
- **Hybrid Store Tests** — 5 new unit tests for `HybridStore` cache behavior; 2 previously-ignored integration tests re-enabled.

### Fixed

- **Clippy Clean** — Zero warnings across the entire workspace (`-D warnings`).
- **File Cleanup** — Removed untracked artifacts (`hello.rs`, `test_output.txt`, `subagent_context/`).

---

## [0.1.0] — 2026-04-21

### Added

- **Plan Mode** — LLM generates a structured JSON plan (`Plan` / `PlanStep`) before execution. `Agent::run()` in Plan mode bypasses the standard ReAct loop and executes steps in batch. TUI commands `/plan` and `/execute`.
- **Parallel Subagents** — `Agent::run_parallel()` executes multiple `RunSpec` concurrently via `BackgroundTaskManager`. TUI `/parallel` command, Gateway `POST /v1/parallel`, and Web UI parallel execution panel.
- **Background Tasks** — `BackgroundTaskManager` with agent scheduler loop, priority queue, and persistent store. Gateway `POST/GET/DELETE /v1/tasks`. claw system-tray app monitors `.clarity/tasks/` in real-time via `notify` + OS notifications.
- **MCP SSE Transport** — Full MCP-over-SSE protocol implementation with endpoint discovery, relative URL resolution, and reconnect loop. `McpManager::from_config()` now honors `transport: "stdio" | "http" | "sse"` from `mcp.json`.
- **Web UI Task Panel** — Click the task badge to open a task list modal with refresh, cancel, and detail view.
- **TUI Parallel Results** — Polished boxed layout with aligned columns for success/failure/aggregation.
- **E2E Validation** — Verified end-to-end with kimi-code: chat completions, background tasks, and parallel subagents all pass.

### Fixed

- Plan mode no longer triggers per-tool interactive approval in `execution.rs` (plan-level vetting happens in `run()`).
- `cancel_on_error` in parallel execution now actually cancels remaining tasks via `task_manager.cancel()`.

---

## [0.0.9] — 2026-04-20

### Added

- **Skill System** — Markdown+YAML orchestration layer. `SkillLoader` parses frontmatter + body. `SkillRegistry` with keyword search. `Agent` injects skill context into system prompt and enforces tool whitelists. TUI `/skill list` and `/skill use <id>`.
- **BM25 + Hybrid Search** — `SqliteStore::search_similar` uses FTS5 recall + in-memory BM25 re-ranking. Replaces O(n) full-table scan.
- **RAG Chunking** — `Chunker` with configurable size, overlap, separator, and source tracking.
- **Gateway Session Store** — SQLite-backed session persistence (CRUD, message append, request counting, expiration cleanup).
- **SSE Parsing Extraction** — Dedicated `SseParser` in `llm/sse.rs` for OpenAI-style streaming deltas, reasoning_content, and tool call deltas.

### Fixed

- Cyclic dependency decoupling: `ToolCall`/`FunctionCall` → `types.rs`; `Message`/`LlmProvider` → `llm/api.rs`.
- `run()` / `run_with_messages_sync()` duplication eliminated via `Agent::run_sync_loop()`.
- Gateway SSE now forwards `tool_calls` deltas as structured events (`ToolCallStart`, `ToolResult`, `StepBegin`).

---

## [0.0.8] — 2026-04-17

### Added

- **Security Hardening** — `validate_mcp_command()` rejects shell metacharacters, relative paths, and non-existent absolute paths before spawning MCP stdio servers. Override via `CLARITY_MCP_ALLOWLIST`.
- **MCP Auto-loading** — Gateway startup loads `~/.config/clarity/mcp.json` automatically.
- **Stream-first LLM Architecture** — `Agent::run_streaming()` calls `llm.stream()` first, falls back to `complete()` only when streaming is unavailable.

### Fixed

- Tool-calling pipeline in Gateway chat: `get_tool_descriptions()` parses `ToolRegistry::get_tool_schemas()` correctly. `OpenAiCompatibleLlm` forwards `tool_calls` and `tool_call_id`. Kimi `thinking` mode disabled to prevent 400 on multi-round tool calls.
- `cargo audit` reports 0 vulnerabilities after updating `rustls-webpki`, `rand`, and 40+ transitive deps.
