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

## Recent Major Changes (2026-04-15)

1. **Fixed tool-calling pipeline in Gateway chat**:
   - `get_skill_definitions()` now correctly parses `ToolRegistry::get_tool_schemas()` array format, so the system prompt's `# 技能` section is properly populated.
   - `OpenAiCompatibleLlm` (`complete` + `stream`) now correctly forwards `tool_calls` and `tool_call_id` fields in API messages, fixing multi-round tool execution.
   - Added `Op::ConversationTurn(Vec<Message>)` to `AgentController`; Gateway `/v1/chat/completions` now forwards the full message history instead of discarding everything except the last user message.

2. **MCP auto-loading is live**: Gateway startup automatically loads `~/.config/clarity/mcp.json` (or env/local fallbacks) and registers MCP tools into the agent's `ToolRegistry`.

3. **Personality system integrated**: `Direct` engineering mode is the default. It injects concise tool-calling instructions via `SystemPromptBuilder` and eliminates the previous verbose `<mood>` XML leakage.

4. **Stream-first LLM architecture**: `Agent::run_streaming()` calls `llm.stream()` first and only falls back to `complete()`. This eliminates the double-request penalty.

5. **Prompt cache key**: `OpenAiCompatibleLlm` injects `prompt_cache_key` into request bodies. `KimiLlm` and `KimiCodeLlm` support `set_prompt_cache_key()`.

6. **Shared HTTP client**: Connection pool, 10s connect timeout, 300s request timeout via `reqwest`.

## Architecture Notes & Coupling Warnings

> **⚠️ The project currently has tight coupling in a few areas.** When making changes, be aware of these boundaries:
>
> 1. **`clarity-core` ↔ `clarity-gateway`**: `AgentController` lives in `core`, but its `Op` enum (`Op::ConversationTurn`) had to be extended to support Gateway's OpenAI-compatible message history. This means Gateway-driven requirements can ripple back into core agent abstractions.
> 2. **`Agent::run_streaming` vs `run_streaming_with_messages`**: We now have two public entry points (`run_streaming` for TUI/simple use, `run_streaming_with_messages` for pre-built history). Consider extracting a pure "agent loop" trait in future refactors to avoid duplicating compaction / wire / memory logic.
> 3. **`OpenAiCompatibleLlm` monolith**: Both `stream()` and `complete()` share request formatting, but the SSE parsing for tool calls is inline. A dedicated `SseToolCallAssembler` would make the LLM layer more testable.
> 4. **`AppState` bloat**: `AppState` currently carries `agent`, `session_manager`, `tool_registry`, and `task_manager`. The `tool_registry` field is actually redundant because `agent.registry()` already holds it (kept for the admin API convenience).
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
- **Gateway SSE does not forward `tool_calls` deltas to the client**: The current design treats the agent as a black box; only the final text answer is streamed. If you need OpenAI-compatible `tool_calls` visible in the frontend, the SSE formatter in `handlers.rs` will need to emit `delta.tool_calls` chunks.

## Code Style

- Rust edition 2021, `tokio` full, `ratatui` 0.24, `axum` 0.7.
- Prefer minimal changes; keep diffs small.
- When modifying `agent/mod.rs` or `llm/mod.rs`, run the full test suite before committing.
- When modifying `AgentController` or `Op`, check all callers in `clarity-tui`, `clarity-gateway`, and integration tests.
