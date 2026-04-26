# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **T_FTUE** — First-time user experience with launch status detection (`get_launch_status` command) and `OnboardingModal` React component with i18n support (en/zh).
- **T_DYNAMIC_PROMPT** — `SystemPromptBuilder` with declarative `PromptComponent` enum for conditional prompt assembly (approval mode notices, offline notices, template variables).
- **T_APPROVAL (V1)** — Rule-based risk engine (`RuleEngine` with `RiskLevel`: Auto/Low/Medium/High). Integrated into `execute_tool_call` to reduce unnecessary approval popups in Interactive mode.
- **T_SETTINGS** — `reload_llm` Tauri command for provider/model hot-swap without restart.
- **T_COMPACT** — Two-tier compaction: Tier-1 fast local truncation of old assistant text (no LLM call) + Tier-2 LLM summarization.
- **T_PARALLEL** — Concurrent tool call execution via `futures::future::join_all` in both sync and streaming loops.
- **T_RELEASE/T_PACKAGE/T_UPDATE/T_SIGN** — GitHub Actions release workflow (tag-triggered), MSI/NSIS bundling, auto-update check (GitHub API + SemVer), and Windows self-signed certificate signing.

### Changed

- **Approval Module Restructure** — `approval.rs` split into `approval/mod.rs` + `approval/rules.rs`.
- **Interactive Mode Refinement** — Low-risk tools (`file_read`, `web_search`) now auto-approve in Interactive mode; only High/Medium/forced-approval tools prompt.

### Fixed

- **Tauri Build Paths** — `tauri.conf.json` `frontendDist` corrected from `../frontend/dist` to `frontend/dist`; `beforeBuildCommand`/`beforeDevCommand` switched to `npm run build`/`npm run dev` (Tauri CLI executes these in `frontend/` directory on Windows).
- **CI Release Workflow** — Added `working-directory: crates/clarity-tauri` to the Tauri build step so `cargo tauri build` locates `tauri.conf.json` correctly in GitHub Actions.
- **FTUE Settings Reload** — `SettingsPanel` now calls `reload_llm` after `save_settings` succeeds, ensuring the LLM binding is re-created with new provider/model configuration without restart.

## [0.2.1] — 2026-04-25

### Added

- **Local GGUF Inference (Default)** — `LocalGgufProvider` using Candle for zero-dependency native GGUF model loading. Supports Qwen2/Qwen2.5/DeepSeek-R1-Distill-Qwen architectures with auto chat-template detection and streaming via `tokio::mpsc`. Now enabled by default via `clarity-core/default = ["local-llm"]`.
- **Offline Auto-Fallback** — Startup `prewarm_llm` with background network monitoring (TCP probe every 30s). Offline mode automatically falls back to local provider; reconnects to cloud provider when network recovers. Frontend shows `llm:fallback` banner.
- **Settings-Runtime Integration** — `ensure_llm` reads `GuiSettings` provider and `local_model_path` at runtime. Settings cache eliminates per-request disk I/O. `save_settings` validates `network_probe_url` format.
- **Headless CLI Local Provider** — `clarity-headless --provider local` with `CLARITY_LOCAL_MODEL_PATH` and `CLARITY_LOCAL_TOKENIZER_REPO` env vars.
- **Model Path Resolution** — `resolve_local_model_path()` replaces all hardcoded paths; uses `CLARITY_LOCAL_MODEL_PATH` env var or auto-scans `~/models/`.
- **Settings Panel Local Model Config** — `clarity-tauri` Settings Panel now supports selecting `Local (GGUF)` provider with auto-scanned `.gguf` models from `~/models/` and `CLARITY_LOCAL_MODEL_PATH`. Path persisted in `gui-settings.json`.
- **Tokenizer Auto-Detection** — `ensure_llm` auto-detects `tokenizer.json` in model directory; avoids HuggingFace download when local file exists. Files < 1024 bytes are rejected as corrupted.
- **Startup Error Caching** — `AppState.prewarm_error` caches startup LLM load errors for frontend inspection via `get_prewarm_status`.

### Changed

- **Kalosm Deprecation** — Old `KalosmProvider` stubbed to redirect users to `LocalGgufProvider`. `ProtocolType::KalosmLocal` now builds `LocalGgufProvider`.

### Fixed

- **Concurrent LLM Load Race** — `tokio::sync::Mutex<()>` double-checked locking prevents concurrent duplicate model loading in `ensure_llm`.
- **Explicit Provider Failure** — When user explicitly selects a provider and it fails, error is returned directly instead of silently falling back to `auto_arc`.
- **LlmBinding.is_fallback Dead Code** — Removed unused field.

### Security

- **Network Probe URL Validation** — `save_settings` validates probe URL contains valid port (1–65535), rejecting malformed endpoints.

## [0.1.2] — 2026-04-24

### Added

- **Memory Depth Integration** — `clarity-core` now uses `clarity-memory::SharedMemoryTicker` exclusively. Gateway `create_agent()` wires up `PersistentMemoryStore` + `SharedMemoryTicker` with a 5-turn default trigger. `MemoryCompiler` four-level pipeline (today → week → longterm → facts) runs automatically on ticker callback via `LlmProviderBridge`.
- **Slack Channel** — New `SlackChannel` implementing the `Channel` trait. Supports `chat.postMessage` with automatic 4000-char chunking, Slack Events API challenge verification, and HMAC-SHA256 signature validation (`verify_signature`). Enabled via `SLACK_ENABLED` / `SLACK_BOT_TOKEN` / `SLACK_APP_TOKEN` env vars.
- **TOML Config System** — `Config::load()` reads three-layer TOML (defaults → `~/.config/clarity/config.toml` → `.clarity.toml`). `export_to_env()` writes profile credentials to provider-specific env vars only when not already set. Integrated into both Gateway and TUI `create_agent()`.

### Fixed

- **Documentation Drift** — `PROJECT_STATUS.md` and `tools_roadmap.md` corrected 6 falsely-claimed limitations that were already implemented.
- **Send Safety** — `clarity-memory::CompilationFuture` and `CompileCallback` tightened with `+ Send` bounds to compile inside `tokio::spawn` async blocks.

## [0.1.1] — 2026-04-23

### Security

- **Directory Traversal Fix** — `resolve_path()` in `clarity-core` now validates that resolved paths stay within the working directory, preventing `..` escapes and absolute-path traversal.
- **Gateway Path Sanitization** — `sanitize_path()` in `clarity-gateway` restricts all file API access to the current working directory prefix after `canonicalize()`.

### Added

- **Crate Documentation** — README + `AGENTS.md` for all 6 crates.
- **Gateway Handler Tests** — 8 mock integration tests covering `health_check`, `file_tree`, `file_read`, `admin_tools`, `admin_get_approval_mode`, `admin_set_approval_mode`.
- **Claw & TUI Lib Split** — Binary crates now expose testable `lib.rs` modules with 6 tests each.
- **CI Hardening** — `cargo audit` and `cargo tarpaulin` jobs added to GitHub Actions.
- **Hybrid Store Tests** — 5 new unit tests for `HybridStore` cache behavior; 2 previously-ignored integration tests re-enabled.

### Fixed

- **Clippy Clean** — Zero warnings across the entire workspace (`-D warnings`).
- **File Cleanup** — Removed untracked artifacts.

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

## [0.0.8] — 2026-04-17

### Added

- **Security Hardening** — `validate_mcp_command()` rejects shell metacharacters, relative paths, and non-existent absolute paths before spawning MCP stdio servers. Override via `CLARITY_MCP_ALLOWLIST`.
- **MCP Auto-loading** — Gateway startup loads `~/.config/clarity/mcp.json` automatically.
- **Stream-first LLM Architecture** — `Agent::run_streaming()` calls `llm.stream()` first, falls back to `complete()` only when streaming is unavailable.

### Fixed

- Tool-calling pipeline in Gateway chat: `get_tool_descriptions()` parses `ToolRegistry::get_tool_schemas()` correctly. `OpenAiCompatibleLlm` forwards `tool_calls` and `tool_call_id`. Kimi `thinking` mode disabled to prevent 400 on multi-round tool calls.
- `cargo audit` reports 0 vulnerabilities after updating `rustls-webpki`, `rand`, and 40+ transitive deps.
