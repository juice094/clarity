# clarity-headless

Headless CLI runner for Clarity agents: execute tasks from stdin or JSON payloads without launching a GUI. Ideal for CI pipelines, cron jobs, and shell scripting.

## Why use this instead of...

- **Claude Code (CLI)** — Claude Code is closed-source and cloud-dependent; clarity-headless is fully local-first with no API key required for local models.
- **Aider** — Aider is tightly coupled to git and code editing; clarity-headless is a general-purpose agent executor that can run any Clarity skill or plan.

## Usage

```bash
cargo run -p clarity-headless -- --help
```
