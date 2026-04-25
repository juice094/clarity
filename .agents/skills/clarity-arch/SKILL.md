---
name: clarity-arch
description: Rust Agent 运行时 Clarity 的架构规范与开发工作流。Use when working with clarity-core, clarity-tui, clarity-gateway, clarity-memory, clarity-wire, or claw crates. Covers build commands, test strategies, MCP transport configs, Plan Mode, parallel subagents, and security hardening.
---

# Clarity Architecture Skill

## Quick Commands

```bash
# Test
cargo test --workspace --lib          # 350+ tests
cargo clippy --workspace --lib --bins --tests  # zero warnings

# Run
cargo run -p clarity-tui               # TUI entry
cargo run -p clarity-gateway           # Gateway entry (0.0.0.0:3000)
```

## Crate Map

| Crate | Responsibility |
|-------|---------------|
| `clarity-core` | Agent loop, ReAct/Plan, approval, compaction, MCP client |
| `clarity-tui` | Terminal UI, ratatui, CommandBar, Wire adapter |
| `clarity-gateway` | HTTP/WebSocket API, session store, admin endpoints |
| `clarity-memory` | SQLite store, BM25+FTS5 hybrid search, chunking |
| `clarity-wire` | Event bus, SSE streaming, message types |
| `claw` | System tray resident, OS notifications |

## MCP Transport

- **stdio**: default for local tools
- **HTTP**: `McpManager::from_config()` respects `mcp.json` transport field
- **SSE**: full implementation with endpoint discovery + reconnect loop
- E2E: Gateway startup auto-registers 22 MCP tools

## Security

- `validate_mcp_command()` rejects shell metacharacters, `..`, relative paths before spawning stdio server
- Override via `CLARITY_MCP_ALLOWLIST` env var

## Coupling Warnings (Remaining)

1. `clarity-core` ↔ `clarity-gateway`: `AgentController` / `Op` enum extensions ripple back into core
2. `AppState` bloat: `tool_registry` redundant (accessible via `agent.registry()`)
3. `std::sync::RwLock` in `Agent.inner` intentionally kept for sync TUI/Gateway callers

## Known Blockers

- **kalosm local Provider**: skeleton only; blocked until agri-paper delivers 7B model benchmark data
- **Discord/Telegram**: disabled by default due to `rustls-webpki` CVE in `serenity 0.12.5`
