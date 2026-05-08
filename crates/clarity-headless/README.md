# clarity-headless

Headless CLI runner for Clarity agents: execute tasks from stdin or JSON payloads without launching a GUI. Ideal for CI pipelines, cron jobs, and shell scripting.

## Why use this instead of...

- **Claude Code (CLI)** — Claude Code is closed-source and cloud-dependent; clarity-headless is fully local-first with no API key required for local models.
- **Aider** — Aider is tightly coupled to git and code editing; clarity-headless is a general-purpose agent executor that can run any Clarity skill or plan.

## Usage

```bash
cargo run -p clarity-headless -- --help
```

## 边界与稳定性

- **Stability tier**: Experimental
  - Experimental: API may change before v0.4.0
- **MSRV**: 1.78.0
- **反向依赖禁止** (No reverse dependencies):
  - 可依赖 clarity-core
- **Library/binary classification**:
  - Binary: application entry point, not a library
