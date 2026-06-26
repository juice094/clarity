---
id: clarity-worktree
name: Clarity Project Worktree
description: OKF knowledge bundle describing the Clarity crate topology, responsibilities, and dependency graph.
version: 0.3.4-rc
date: 2026-06-25
source_repo: https://github.com/juice094/clarity
title: Clarity Project Worktree
type: index
okf_version: '0.1'
timestamp: '2026-06-26T11:28:50Z'
---

# Clarity Project Worktree

This OKF bundle describes the 23 crate directories in the Clarity workspace:
22 active workspace members + 1 archived (`clarity-tauri`), plus the
`tests/integration` integration-test crate.

## Layers

  archived:
    - clarity-tauri
  contract:
    - clarity-contract
    - clarity-wire
  experimental:
    - clarity-slint
  infrastructure:
    - clarity-channels
    - clarity-llm
    - clarity-mcp
    - clarity-memory
    - clarity-openclaw
    - clarity-rollout
    - clarity-secrets
    - clarity-subagents
    - clarity-telemetry
    - clarity-thread-store
    - clarity-tools
  kernel:
    - clarity-core
  presentation:
    - clarity-claw
    - clarity-egui
    - clarity-gateway
    - clarity-headless
    - clarity-mobile-core
    - clarity-tui
  utility:
    - clarity-anthropic-proxy

## Concepts

- [`clarity-contract`](concepts/clarity-contract.md) — Shared trait/type contract with zero internal dependencies.
- [`clarity-wire`](concepts/clarity-wire.md) — UI ↔ Agent event bus using SPMC channels.
- [`clarity-memory`](concepts/clarity-memory.md) — Hybrid memory: SQLite + BM25 + vector search.
- [`clarity-mcp`](concepts/clarity-mcp.md) — MCP client with stdio / SSE / HTTP / WebSocket transports.
- [`clarity-llm`](concepts/clarity-llm.md) — LLM provider abstraction + Candle GGUF local inference.
- [`clarity-tools`](concepts/clarity-tools.md) — Built-in tool library.
- [`clarity-channels`](concepts/clarity-channels.md) — External communication channel abstraction.
- [`clarity-secrets`](concepts/clarity-secrets.md) — Encrypted secret storage using ChaCha20-Poly1305.
- [`clarity-openclaw`](concepts/clarity-openclaw.md) — OpenClaw/KimiClaw Gateway WebSocket client and device identity.
- [`clarity-subagents`](concepts/clarity-subagents.md) — Sub-agent executor and parallel scheduler.
- [`clarity-rollout`](concepts/clarity-rollout.md) — JSONL rollout persistence for thread event logs.
- [`clarity-thread-store`](concepts/clarity-thread-store.md) — Thread persistence abstraction.
- [`clarity-telemetry`](concepts/clarity-telemetry.md) — Unified telemetry: WideEvent, metrics, traces, config audit.
- [`clarity-core`](concepts/clarity-core.md) — Agent kernel: ReAct/Plan loop, Approval, Skill, MCP integration.
- [`clarity-gateway`](concepts/clarity-gateway.md) — Axum HTTP/WebSocket server and Web IDE.
- [`clarity-egui`](concepts/clarity-egui.md) — Primary desktop GUI (eframe/egui, pure Rust).
- [`clarity-tui`](concepts/clarity-tui.md) — Terminal UI using ratatui.
- [`clarity-claw`](concepts/clarity-claw.md) — System-tray background monitor.
- [`clarity-headless`](concepts/clarity-headless.md) — Headless CLI for scripts and CI.
- [`clarity-mobile-core`](concepts/clarity-mobile-core.md) — Mobile FFI core for Android/iOS.
- [`clarity-slint`](concepts/clarity-slint.md) — Experimental Slint desktop GUI.
- [`clarity-anthropic-proxy`](concepts/clarity-anthropic-proxy.md) — Anthropic Messages API → DeepSeek proxy utility.
- [`clarity-tauri`](concepts/clarity-tauri.md) — Archived Tauri desktop frontend.

## Key Invariants

1. `clarity-core` has zero dependencies on any frontend or network crate.
2. `clarity-contract` has zero internal dependencies.
3. Frontend crates never import each other; cross-frontend state/events go
   through `clarity-wire`.
4. `clarity-slint` depends only on `clarity-contract` + `clarity-wire`; it does
   not consume `clarity-core`.

## Test Cases

- [TC-LLM-001: OpenAI-compatible provider completes a chat request](test-cases/TC-LLM-001.md)
- [TC-LLM-002: Model registry loads models.toml and resolves an alias](test-cases/TC-LLM-002.md)
- [TC-LLM-003: resolve_key_ref handles env, file, and literal references](test-cases/TC-LLM-003.md)
- [TC-LLM-004: ReliableProvider retries primary and falls back on failure](test-cases/TC-LLM-004.md)
- [TC-LLM-005: Local GGUF model discovery respects CLARITY_LOCAL_MODEL_PATH](test-cases/TC-LLM-005.md)
- [TC-LLM-006: RouterLlmProvider routes by hint](test-cases/TC-LLM-006.md)
- [TC-LLM-007: model_listing fallback derives from canonical registry defaults](test-cases/TC-LLM-007.md)
- [TC-LLM-008: Anthropic provider uses prompt-guided tool calling](test-cases/TC-LLM-008.md)

## Templates

- [Test Case Template](templates/test-case.md)

## Living Sources of Truth

- Operational context & test baselines: [`AGENTS.md`](../../AGENTS.md)
- Code-accurate architecture: [`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md)
- Module-level topology: [`docs/architecture/map-topology.md`](../../architecture/map-topology.md)
