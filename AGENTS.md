# Agent Guidance for Project Clarity

## Quick Reference

```bash
cd C:\Users\22414\dev\third_party\clarity
cargo test --workspace --lib          # 481 tests
cargo clippy --workspace --lib --bins --tests  # zero warnings
cargo run -p clarity-tui               # run TUI (needs API key)
cargo run -p clarity-gateway           # run Gateway (needs API key)

# Desktop GUI (Tauri 2)
cd crates/clarity-tauri/frontend && npm run build
cargo tauri dev
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

## Recent Major Changes (2026-04-20 ~ Sprint 1 GUI)

### 文档整理（2026-04-25）

- 归档根目录 6 个过时 `.md` 文件至 `docs/archive/` 和 `docs/comparisons/`：
  - `IMPLEMENTATION_SUMMARY.md`、`PHASE2_RWLOCK_AUDIT.md`、`PLAN_v0.2.md`
  - `PROJECT_REPORT.md`、`PROJECT_STATUS.md`、`TEST_REPORT.md`
  - `OPENCLAW_GAP_ANALYSIS.md` → `docs/comparisons/`
- 更新 `docs/README.md` 索引：版本升至 v0.2.0，加入 GUI Sprint 1-2 功能列表
- 根目录仅保留 `README.md`、`README.en.md`、`AGENTS.md`、`CHANGELOG.md`

### GUI Desktop — Sprint 1 核心功能交付

1. **Chat Panel + Agent Bridge**:
   - `agent_run_streaming` Tauri Command with SSE-style event emission (`agent:chunk`, `agent:done`, `agent:error`).
   - Real-time streaming output in React chat panel with dot-flashing loading animation.
   - Auto LLM configuration from `OPENAI_API_KEY` env var when unconfigured.

2. **Session Sidebar** (`Sidebar.tsx`):
   - Multi-session management: create, switch, delete, rename.
   - Sessions sorted by `updated_at` desc; inline rename with Enter/Escape/blur.
   - Collapsible sidebar with smooth width transition.
   - Message history isolation per session (frontend memory only; SQLite persistence pending).

3. **Task Panel** (`TaskPanel.tsx`):
   - Polling every 5s via `list_tasks` / `cancel_task` Tauri Commands.
   - Status color mapping: running→accent, pending→secondary, completed→#238636, failed→danger.
   - Cancel button for running/pending tasks.

4. **Settings Panel** (`SettingsPanel.tsx`):
   - Provider-model linkage: switching provider auto-selects first available model.
   - Approval mode selection: Interactive / Yolo / Plan (UI ready; runtime sync pending Subagent-E).
   - Theme selection: Dark / Light / Auto with instant preview via CSS variable switching.
   - JSON file persistence at `%APPDATA%/clarity/settings.json`.
   - Save/Reset/Cancel with 2s toast feedback.

5. **Theme System** (`App.css` + `App.tsx`):
   - CSS variable dual-theme: `:root` (dark default) + `[data-theme="light"]` override.
   - `window.matchMedia("prefers-color-scheme: dark")` listener for Auto mode.
   - `document.documentElement.setAttribute("data-theme", ...)` dynamic application.
   - SettingsPanel Cancel restores DOM theme to last saved value.

6. **Security & Dependencies**:
   - `cargo audit` + `cargo update` upgraded 14 packages (`rustls` 0.23.39, `libc` 0.2.186, etc.).
   - 9 remaining warnings are all Tauri upstream indirect deps (fxhash, gdkx11, instant, unic-*, rand 0.7) — cannot fix locally.

### Previous Major Changes (2026-04-21)

1. **Plan Mode — "先规划、后执行"工作流**:
   - `Agent::plan()` 调用 LLM 生成结构化 JSON 计划（`Plan` / `PlanStep`）。
   - `Agent::execute_plan()` 按步骤批量执行工具，错误隔离，结果聚合为 `PlanResult`。
   - `ApprovalMode::Plan` 接入 `Agent::run()`：检测到 Plan 模式时自动生成并执行计划，绕过标准 ReAct 循环的逐工具审批。
   - TUI `/plan <query>` 生成计划，`/execute` 执行 pending plan。
   - `execution.rs` Plan 分支改为自动通过（计划级审批已在 `run()` 完成）。

2. **并行子代理（Parallel Subagents）**:
   - `Agent::run_parallel()`：一键并发执行多个子代理，基于 `SubagentManager` + `BackgroundTaskManager`。
   - `ParallelConfig` 支持并发数、超时、失败时取消、结果聚合。
   - TUI `/parallel <type>:<prompt> [| <type>:<prompt>...]` 命令，结果用对齐列展示。
   - Gateway `POST /v1/parallel` JSON API，Web UI 并行执行面板（命令面板入口，动态任务表单，结果回显到聊天）。
   - `cancel_on_error` 占位 TODO 已实现：失败时通过 `task_manager.cancel()` 取消剩余任务。

3. **后台任务系统完善 + claw 托盘实时通知**:
   - Gateway `POST/GET/DELETE /v1/tasks` 完整 handlers + 路由。
   - TUI `/task list/status/cancel/spawn` 命令。
   - claw（系统托盘常驻应用）使用 `notify` crate 实时监控 `.clarity/tasks/` 目录变化，触发即时 OS 通知 + 动态 tooltip。
   - Web UI 右上角任务 badge 实时显示运行中任务数，点击弹出任务列表面板（支持刷新、取消、查看详情）。

4. **MCP SSE transport 完整实现**:
   - `SseMcpClient` 重写：完整 MCP-over-SSE 协议（endpoint 发现、相对 URL 解析、重连循环）。
   - `McpManager::from_config()` 修复：不再硬编码 Stdio，尊重 `mcp.json` 中的 `transport` 字段（stdio/http/sse）。
   - E2E 验证：Gateway 启动时自动注册 22 个 MCP 工具（filesystem、web search 等）。

5. **E2E 真实 LLM 联调通过（kimi-code）**:
   - Chat completions：7.8s 正常响应。
   - 后台任务：2.4s 完成，filesystem 工具调用正常。
   - 并行子代理：2 任务并发，8.4s，成功率 100%。

## Comparison Reference: Clarity vs cc-haha (Claude Code Haha)

Both projects fork from the same Claude Code leaked source but diverge significantly:

| Dimension | **Clarity** | **cc-haha** |
|-----------|-------------|-------------|
| Core Language | Rust | TypeScript (Bun) |
| TUI | ratatui | Ink (React) |
| Desktop | Tauri 2 → native Rust core (single process) | Tauri 2 frontend ↔ Bun server (dual process) |
| Gateway | Axum (built-in) | Bun server (separate launch) |
| LLM Providers | OpenAI, Anthropic, DeepSeek, Ollama, Kimi | Anthropic-compatible only |
| Memory | SQLite + BM25 + vector hybrid | File-based |
| Notifications | Multi-channel webhook (5+ platforms) | Telegram / 飞书 / Discord IM adapters |
| Computer Use | ❌ Not yet | ✅ Screenshot/mouse/keyboard |
| Approval Modes | ✅ Interactive/Yolo/Plan (runtime switchable) | Permission controls |
| Theme System | ✅ Dark/Light/Auto | ❌ Not yet |
| Diff View | ❌ Not yet | ✅ In desktop |
| Headless Mode | ❌ Not yet | ✅ `--print` |
| LSP | ❌ Not yet | ✅ |
| Test Coverage | 474+ Rust unit tests | Vitest (desktop only) |

**Strategic gaps to close**: Computer Use, Diff view, Headless mode, LSP integration.

## Previous Major Changes (2026-04-20)

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
> 4. **`std::sync::RwLock` in `Agent.inner`**: Intentionally kept as `std::sync::RwLock<AgentInner>`. `Agent` getters/setters are synchronous and may be called from non-async contexts (TUI event loop, Gateway handlers). All critical sections are short field reads/writes only. `background/` module locks have been migrated to `tokio::sync` (`1141ba9`).
>
> **Recommendation for future refactors**: Extract a `ChatDriver` or `ConversationEngine` trait from `Agent` so that `Gateway` and `TUI` can inject their own message-building strategies without modifying core enums.

## Security Notes

- **MCP stdio command validation is active** (since 2026-04-17). Before spawning any MCP server, Clarity validates the `command` field:
  - Shell metacharacters and `..` sequences are rejected.
  - Relative paths are rejected.
  - Absolute paths must exist and point to a file.
  - Bare names (e.g. `npx`, `uvx`) are allowed and resolved via `PATH`.
  - Override with the `CLARITY_MCP_ALLOWLIST` environment variable (comma-separated absolute paths or prefixes).

## 子代理串并行执行计划

完整的依赖关系编排和 Git 安全机制见 [`docs/execution-plan.md`](./docs/execution-plan.md)。

**当前 Phase**：Phase 1 — Session persistence（串行单轨）

## Active Subagent Tasks

| Subagent | Task | Branch | Status |
|----------|------|--------|--------|
| Subagent-G | Session persistence — GUI 会话 JSON 持久化 | `subagent/session-persist-2026-0425` | 🔄 Running |

**Merge policy**: Subagent completes → main session reviews → `cargo test` + `npm run build` → merge to `main` → push.

## Completed Subagent Tasks (Sprint 1-2)

| Subagent | Task | Commit |
|----------|------|--------|
| Subagent-D | 暗色主题系统化 | `1641572` |
| Subagent-E | 审批系统运行时切换对接 | `1e8b0fe` |
| Subagent-F | 文件浏览器面板 | `cda98d4` |

## Known Issues

- ~~Personality system produces verbose `<mood>` XML metadata~~ **Fixed** by `Direct` mode.
- ~~MCP client is skeletal~~ **Fixed** — stdio/HTTP/SSE transport and dynamic registration are working.
- ~~MCP SSE not implemented~~ **Fixed** (2026-04-21) — `SseMcpClient` supports full MCP-over-SSE protocol with endpoint discovery and reconnect.
- ~~Web UI missing~~ **Fixed** — Gateway serves an embedded Web IDE (`chat.html`) with Monaco Editor and SSE streaming.
- ~~`agent ↔ approval` / `agent ↔ llm` / `agent ↔ compaction` cyclic dependencies~~ **Fixed** (2026-04-20).
- ~~Old Skill system dead code~~ **Fixed** — removed `skill/` module; new `skills/` orchestration layer landed.
- ~~Gateway SSE does not forward `tool_calls` deltas to the client~~ **Fixed** (2026-04-20) — SSE now emits structured events: `ToolCallStart` (with `id`, `name`, `arguments`), `ToolResult` (with `id`, `result`), and `StepBegin` (with `tool_name`).
- **Session data is frontend-memory only**: Refreshing the GUI loses all sessions. Needs integration with `clarity-memory` session_store or `clarity-gateway` session_store.
- **`task.rs` uses Mock data**: `list_tasks` / `cancel_task` are not wired to real `BackgroundTaskManager`.
- **Streaming output session switching bug**: Switching sessions while streaming may append chunks to the wrong message array.
- **kalosm local Provider not yet integrated**: Skeleton file planned; real implementation blocked until agri-paper delivers 7B model benchmark data.
- **Discord/Telegram channels disabled by default**: Blocked by upstream `rustls-webpki` CVEs in `serenity 0.12.5`. Re-enable when upstream publishes a fix.
- ~~Skill system not yet wired into Agent loop~~ **Fixed** (2026-04-20) — `Agent` now holds `skill_registry` and `active_skill`. `build_system_prompt()` injects skill context; `filter_tools_value()` enforces tool whitelists. TUI commands `/skill list` and `/skill use <id>` are live.
- **Gateway HTTP Chat Completions is stateless by default**: While WebSocket already has full session support, the HTTP `/v1/chat/completions` endpoint now supports optional `session_id` persistence (load history + save turn). If no `session_id` is provided, a new one is auto-generated and returned in the response.

## Code Style

- Rust edition 2021, `tokio` full, `ratatui` 0.24, `axum` 0.7.
- Prefer minimal changes; keep diffs small.
- When modifying `agent/mod.rs` or `llm/mod.rs`, run the full test suite before committing.
- When modifying `AgentController` or `Op`, check all callers in `clarity-tui`, `clarity-gateway`, and integration tests.
