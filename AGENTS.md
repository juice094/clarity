# Agent Guidance for Project Clarity

## Quick Reference

```bash
cd C:\Users\<user>\Desktop\clarity
cargo test --workspace --lib          # 334+ tests
cargo clippy --workspace --lib --bins --tests  # zero warnings
cargo run -p clarity-tui               # run TUI (needs API key)
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

## Recent Major Changes (2026-04-09)

1. **Stream-first LLM architecture**: `Agent::run_streaming()` now calls `llm.stream()` first and only falls back to `complete()`. This eliminates the double-request penalty.
2. **Prompt cache key**: `OpenAiCompatibleLlm` injects `prompt_cache_key` into request bodies. `KimiLlm` and `KimiCodeLlm` support `set_prompt_cache_key()`.
3. **Shared HTTP client**: Connection pool, 10s connect timeout, 300s request timeout via `reqwest`.
4. **TUI command registry + mouse wheel**: `clarity-tui` now has a `CommandRegistry` (`/model`, `/help`, `/stop`) and captures mouse scroll events.
5. **TUI dark theme overhaul**: `ChatPane`, `InputPane`, `GeneratingIndicator`, `StatusBar`, and `CommandBar` all use a unified dark-blue color palette.
6. **Kimi Code endpoint fixed**: Default base URL is now `https://api.kimi.com/coding/v1` (was incorrectly pointing at `api.moonshot.cn`).
7. **Tool calls in `complete()`**: `OpenAiCompatibleLlm::complete()` now correctly parses `choices[].message.tool_calls` instead of returning an empty vector.

## Known Issues

- **Personality system produces verbose `<mood>` XML metadata**: The current default personality generates poetic, existential `<mibe>` blocks that waste tokens and obscure tool-calling behavior. A plan to add minimal/engineering personality modes is in progress.
- **MCP client is skeletal**: `mcp.json` loading works but dynamic server registration is not fully wired.

## Code Style

- Rust edition 2021, `tokio` full, `ratatui` 0.24, `axum` 0.7.
- Prefer minimal changes; keep diffs small.
- When modifying `agent/mod.rs` or `llm/mod.rs`, run the full test suite before committing.
