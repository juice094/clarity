# Clarity — Personal AI Standard Runtime

> An opinionated, multi-entry AI runtime: plan → execute → monitor → remember.

Clarity is not another chat client. It is a **personal AI standard runtime** that orchestrates LLMs, tools, and sub-agents across multiple entry points — system-tray monitor, web IDE, desktop GUI, and full TUI — with persistent memory, structured planning, and parallel execution.

**All UI layers are Rust-native** (Dioxus for Desktop/Web, ratatui for TUI) — no JavaScript frontend, no Electron, no webview tax.

```
┌─────────────────────────────────────────────────────────────┐
│                        CLARITY RUNTIME                       │
├─────────────┬─────────────────┬─────────────────────────────┤
│   claw      │     window      │           cli               │
│  (托盘)      │   (Web IDE)     │      (TUI 终端)            │
│             │                 │                             │
│ • 实时通知   │ • 文件浏览器    │ • 完整交互式会话            │
│ • 任务徽章   │ • Monaco 编辑器 │ • /plan /parallel /task     │
│ • 系统托盘   │ • SSE 流式对话  │ • 快捷键 + 弹窗审批         │
└──────┬──────┴────────┬────────┴────────────┬────────────────┘
       │               │                     │
       └───────────────┴─────────────────────┘
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

| Feature | Description |
|---------|-------------|
| **Plan Mode** | LLM generates a structured execution plan first; runs steps in batch without per-tool approval. |
| **Parallel Subagents** | Split work across multiple specialized agents (coder, explore, plan) and execute concurrently. |
| **Background Tasks** | Long-running agent tasks survive TUI/Web closure; monitored in real-time by the system-tray app. |
| **MCP Ecosystem** | Stdio, HTTP, and SSE transports for Model Context Protocol servers. |
| **Persistent Memory** | BM25 + vector hybrid search across conversation history. |
| **Skills** | Markdown+YAML skill files that inject context and whitelist tools into the system prompt. |
| **Agent Teams** | Dynamically create collaborative teams of sub-agents with shared mailbox coordination. |
| **Push Notifications** | Multi-channel alerts (filesystem + webhook) for task completions and important events. |
| **Daemon Runtime** | Cross-platform PID lockfile + graceful shutdown for long-running background processes. |
| **AutoDream** | Nightly memory consolidation scheduler — automatically compiles and archives memories. |
| **Lazy Master** | Heavy components (LLM, MemoryStore, SkillRegistry) are initialized on first `run()`, not at startup. |
| **Three Entries** | claw (tray), window (browser), cli (ratatui terminal). Use the right tool for the job. |
| **Rust-Native UI** | Web + Desktop GUI powered by [Dioxus](https://dioxuslabs.com/) — pure Rust, no JavaScript. |

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

Supported providers: `kimi`, `kimi-code`, `openai`, `anthropic`, `deepseek`.

### 3. Run

```bash
# Start the Gateway (serves Web UI on http://127.0.0.1:18800)
clarity-gateway

# In another terminal, start the TUI
clarity-tui

# Optional: start the tray monitor
clarity-claw
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
```

## License

MIT
