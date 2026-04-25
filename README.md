# Clarity — Personal AI Standard Runtime

> An opinionated, multi-entry AI runtime: plan → execute → monitor → remember.

Clarity is not another chat client. It is a **personal AI standard runtime** that orchestrates LLMs, tools, and sub-agents across multiple entry points — system-tray monitor, web IDE, desktop GUI, and full TUI — with persistent memory, structured planning, and parallel execution.

**Rust drives the core** (Agent/Memory/Tools), **Tauri 2 drives the GUI** (Desktop + Mobile + Web), **ratatui drives the TUI** — native performance across all platforms.

```
┌─────────────────────────────────────────────────────────────────────┐
│                         CLARITY RUNTIME                              │
├──────────┬──────────┬──────────────┬────────────────────────────────┤
│  mobile  │ desktop  │    web       │           cli                  │
│  (APP)   │  (GUI)   │  (Browser)   │      (TUI 终端)               │
│          │          │              │                                │
│• iOS/安卓│• 多会话   │• 即开即用    │• 完整交互式会话                │
│• 推送通知│• 本地Agent│• REST + WS   │• /plan /parallel /task         │
│• 生物识别│• 系统集成 │• 多设备访问  │• 快捷键 + 弹窗审批             │
└─────┬────┴────┬─────┴──────┬───────┴──────────┬─────────────────────┘
      │         │            │                  │
      └─────────┴────────────┴──────────────────┘
                        │
          ┌─────────────┴─────────────┐
          │      clarity-core         │
          │  • Agent (ReAct / Plan)   │
          │  • ToolRegistry (built-in + MCP)
          │  • BackgroundTaskManager  │
          │  • Memory (BM25 + vector) │
          │  • Subagent (parallel)    │
          └───────────────────────────┘
```

## Features

| Feature | Description | Status |
|---------|-------------|--------|
| **Plan Mode** | LLM generates a structured execution plan first; runs steps in batch without per-tool approval. | ✅ |
| **Parallel Subagents** | Split work across multiple specialized agents and execute concurrently. | ✅ |
| **Background Tasks** | Long-running agent tasks survive TUI/Web closure; monitored in real-time. | ✅ |
| **MCP Ecosystem** | Stdio, HTTP, and SSE transports for Model Context Protocol servers. | ✅ |
| **Persistent Memory** | BM25 + vector hybrid search across conversation history. | ✅ |
| **Skills** | Markdown+YAML skill files that inject context and whitelist tools. | ✅ |
| **Agent Teams** | Collaborative teams of sub-agents with shared mailbox coordination. | ✅ |
| **Push Notifications** | Multi-channel alerts (Slack/Discord/钉钉/飞书/Telegram/Webhook). | ✅ |
| **Daemon Runtime** | Cross-platform PID lockfile + graceful shutdown. | ✅ |
| **AutoDream** | Nightly memory consolidation scheduler. | ✅ |
| **Lazy Master** | Heavy components initialized on first `run()`, not at startup. | ✅ |
| **Four Entries** | mobile, desktop, web, cli — use the right tool for the job. | 🔄 Desktop WIP |
| **GUI Desktop** | Tauri 2 + React 18 — chat, sessions, tasks, settings, file browser. | 🔄 Sprint 1-2 |
| **Session Management** | Multi-session sidebar with create/switch/delete/rename. | ✅ |
| **Task Panel** | Real-time background task list with cancel action. | ✅ |
| **Settings Panel** | Model/provider/approval-mode/theme config with JSON persistence. | ✅ |
| **Theme System** | Dark / Light / Auto with CSS variables + system theme listener. | ✅ |
| **Approval System** | Interactive / Yolo / Plan — runtime hot-swap via GUI. | 🔄 Subagent-E |
| **File Browser** | Browse working directory tree, click to insert `@path` references. | 🔄 Subagent-F |

## Clarity vs cc-haha (Claude Code Haha)

Both projects fork from the same Claude Code leaked source, but diverge significantly:

| Dimension | **Clarity** | **cc-haha** |
|-----------|-------------|-------------|
| **Core Language** | Rust (memory-safe, zero-cost) | TypeScript (Bun runtime) |
| **TUI** | ratatui (Rust terminal UI) | Ink (React in terminal) |
| **Desktop Architecture** | Tauri 2 → native Rust core (single process) | Tauri 2 frontend ↔ Bun server (dual process) |
| **HTTP Gateway** | Axum (built-in, `cargo run -p clarity-gateway`) | Bun server (separate `SERVER_PORT=3456` launch) |
| **LLM Providers** | OpenAI, Anthropic, DeepSeek, Ollama, Kimi | Anthropic-compatible (OpenRouter, MiniMax, etc.) |
| **Memory Storage** | SQLite + BM25 + cosine vector hybrid | File-based (unspecified backend) |
| **Background Tasks** | Worker pool + Cron scheduler + system tray | Cron tasks |
| **Notifications** | Multi-channel webhook (5+ platforms) | Telegram / 飞书 / Discord IM adapters |
| **Computer Use** | ❌ Not yet | ✅ Screenshot / mouse / keyboard control |
| **Approval Modes** | ✅ Interactive / Yolo / Plan (runtime switchable) | Permission controls |
| **Theme System** | ✅ Dark / Light / Auto | ❌ Not yet |
| **Session Isolation** | ✅ Sidebar with full CRUD | Multi-tab |
| **Headless Mode** | ❌ Not yet | ✅ `--print` for scripts/CI |
| **LSP Support** | ❌ Not yet | ✅ |
| **Diff View** | ❌ Not yet | ✅ In desktop |
| **Test Coverage** | 474+ Rust unit tests | Vitest (desktop only) |

**Clarity's differentiation**: Rust-native core for performance and safety, richer built-in toolset (web search, todo, plan, notify), SQLite-backed hybrid memory retrieval, and a unified single-process desktop architecture.

**cc-haha's differentiation**: Computer Use desktop control, more mature desktop UI (diff view, code editing), deeper IM integrations, and comprehensive VitePress documentation.

## Quick Start

### 1. Install

```bash
# TUI (full interactive experience)
cargo install --path crates/clarity-tui

# Gateway (Web IDE + API server)
cargo install --path crates/clarity-gateway

# claw (system-tray monitor)
cargo install --path crates/clarity-claw
```

### 2. Configure API Key

```bash
# Kimi Code (recommended for coding tasks)
mkdir -p .clarity
cat > .clarity/user_config.json << 'EOF'
{
  "provider": "kimi-code",
  "api_key": "sk-kimi-..."
}
EOF
```

Supported providers: `kimi`, `kimi-code`, `openai`, `anthropic`, `deepseek`, `ollama`.

### 3. Run

```bash
# Start the Gateway (serves Web UI on http://127.0.0.1:18800)
clarity-gateway

# In another terminal, start the TUI
clarity-tui

# Optional: start the tray monitor
clarity-claw

# Desktop GUI (Tauri 2 — requires Node.js/npm)
cd crates/clarity-tauri/frontend && npm install && npm run build
cd ../..
cargo tauri dev
```

## TUI Commands

```
/plan <query>          Generate a structured execution plan
/execute               Execute the pending plan
/parallel <type>:<prompt> [| ...]   Run subagents in parallel
/task list             List background tasks
/task spawn <name> <prompt>         Spawn a background task
/skill list            List available skills
/skill use <id>        Activate a skill
/model <name>          Switch LLM model
/help                  Show all commands
```

## Architecture

```
crates/
├── clarity-core      # Agent loop, tools, memory, MCP, subagents
├── clarity-memory    # BM25 + vector hybrid search, chunking
├── clarity-gateway   # Axum HTTP server, Web UI, session store
├── clarity-tauri     # Tauri 2 Desktop + Mobile GUI (React frontend)
├── clarity-tui       # ratatui terminal interface
├── clarity-claw      # System-tray background monitor
└── clarity-wire      # UI↔Agent event bus
```

## Development

```bash
# Run all tests
cargo test --workspace --lib          # 474+ tests

# Run clippy
cargo clippy --workspace --lib --bins --tests

# Run Gateway for local development
cargo run -p clarity-gateway

# Run TUI
cargo run -p clarity-tui

# Run Desktop GUI
cd crates/clarity-tauri/frontend && npm run build
cargo tauri dev
```

## Roadmap

- [x] Core Agent (ReAct, streaming, tool registry)
- [x] TUI (ratatui full interface)
- [x] Gateway (Axum HTTP + session store)
- [x] Memory system (BM25 + vector hybrid)
- [x] Background tasks + Cron
- [x] MCP ecosystem (stdio/sse/http)
- [x] Subagents + Teams
- [x] GUI Desktop — Chat panel + streaming
- [x] GUI Desktop — Session sidebar
- [x] GUI Desktop — Task panel
- [x] GUI Desktop — Settings panel
- [x] GUI Desktop — Theme system (Dark/Light/Auto)
- [ ] GUI Desktop — Approval system runtime sync (Subagent-E 🔄)
- [ ] GUI Desktop — File browser panel (Subagent-F 🔄)
- [ ] GUI Desktop — Session persistence (SQLite backend)
- [ ] GUI Desktop — Diff view for file edits
- [ ] Computer Use (screenshot / mouse / keyboard)
- [ ] Headless mode (`--print`)
- [ ] LSP integration
- [ ] Mobile app (iOS/Android via Tauri 2)

## License

MIT
