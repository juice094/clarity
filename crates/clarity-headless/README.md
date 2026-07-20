# clarity-headless

Headless CLI runner for Clarity agents: execute tasks from stdin or JSON payloads without launching a GUI. Ideal for CI pipelines, cron jobs, and shell scripting.

## Why use this instead of...

- **Claude Code (CLI)** — Claude Code is closed-source and cloud-dependent; clarity-headless is fully local-first with no API key required for local models.
- **Aider** — Aider is tightly coupled to git and code editing; clarity-headless is a general-purpose agent executor that can run any Clarity skill or plan.

## Usage

```bash
cargo run -p clarity-headless -- --help
```

### ACP bridge

Relay Kimi cloud Agent messages to a local backend:

```bash
# Auto-detect local backend (OpenClaw Gateway if ~/.kimi_openclaw/openclaw.json exists)
cargo run -p clarity-headless -- acp-bridge

# Force original Clarity Gateway
cargo run -p clarity-headless -- acp-bridge --local-backend gateway

# Force Kimi Desktop OpenClaw Gateway
cargo run -p clarity-headless -- acp-bridge --local-backend openclaw
```

### OpenClaw device pairing

Pair this machine with a local Kimi Desktop OpenClaw Gateway to obtain full
scopes. The paired token is saved under the platform data directory and reused
by subsequent `acp-bridge` runs.

```bash
cargo run -p clarity-headless -- openclaw-pair --token <admin-token>
```

## 边界与稳定性

- **Stability tier**: Experimental
  - Experimental: API may change before v0.4.0
- **MSRV**: 1.85（跟随 workspace）
- **反向依赖禁止** (No reverse dependencies):
  - 可依赖 clarity-core
- **Library/binary classification**:
  - Binary: application entry point, not a library
