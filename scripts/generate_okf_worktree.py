#!/usr/bin/env python3
"""Generate an OKF (Open Knowledge Format) bundle for Clarity crate topology."""

import os
from pathlib import Path

BASE = Path("docs/okf/clarity-worktree")
CONCEPTS = BASE / "concepts"

CRATES = [
    {
        "id": "clarity-contract",
        "name": "clarity-contract",
        "type": "contract",
        "layer": "contract",
        "depends_on": [],
        "consumed_by": [
            "clarity-wire",
            "clarity-memory",
            "clarity-mcp",
            "clarity-llm",
            "clarity-tools",
            "clarity-channels",
            "clarity-secrets",
            "clarity-rollout",
            "clarity-thread-store",
            "clarity-telemetry",
            "clarity-core",
            "clarity-anthropic-proxy",
            "clarity-slint",
            "clarity-mobile-core",
        ],
        "summary": "Shared trait/type contract with zero internal dependencies.",
        "responsibilities": [
            "`LlmProvider` trait",
            "`Tool` trait",
            "`AgentError` unified error type",
            "`FederationMessage`",
            "`ThreadId`",
            "`RolloutItem`",
        ],
        "note": "Everything builds on this crate.",
    },
    {
        "id": "clarity-wire",
        "name": "clarity-wire",
        "type": "wire",
        "layer": "contract",
        "depends_on": ["clarity-contract"],
        "consumed_by": [
            "clarity-core",
            "clarity-egui",
            "clarity-tui",
            "clarity-gateway",
            "clarity-claw",
            "clarity-headless",
            "clarity-mobile-core",
            "clarity-slint",
        ],
        "summary": "UI ↔ Agent event bus using SPMC channels.",
        "responsibilities": [
            "`WireMessage` protocol",
            "`ViewCommand`",
            "`WireBroadcaster`",
        ],
        "note": "Cross-frontend communication must go through this crate.",
    },
    {
        "id": "clarity-memory",
        "name": "clarity-memory",
        "type": "memory",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract"],
        "consumed_by": ["clarity-core", "clarity-gateway", "clarity-mobile-core", "clarity-knowledge"],
        "summary": "Hybrid memory: SQLite + BM25 + vector search.",
        "responsibilities": [
            "BM25 keyword retrieval",
            "Vector/cosine similarity search",
            "Chunking",
            "Four-level compaction/archive",
            "Session persistence",
        ],
        "note": "Features: `sqlite`, `embedding`.",
    },
    {
        "id": "clarity-knowledge",
        "name": "clarity-knowledge",
        "type": "knowledge",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract", "clarity-memory"],
        "consumed_by": ["clarity-core"],
        "summary": "Local knowledge indexing and AI-native interaction with activation dynamics.",
        "responsibilities": [
            "File-system scanning and incremental indexing",
            "Hybrid retrieval (BM25 + vector + graph)",
            "In-memory knowledge graph",
            "Dynamic knowledge field with spreading activation",
            "File-system change detection",
        ],
        "note": "No dependency on Obsidian/Syncthing; works with plain Markdown and wikilinks.",
    },
    {
        "id": "clarity-mcp",
        "name": "clarity-mcp",
        "type": "mcp",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract", "clarity-wire"],
        "consumed_by": ["clarity-llm", "clarity-core"],
        "summary": "MCP client with stdio / SSE / HTTP / WebSocket transports.",
        "responsibilities": [
            "MCP server lifecycle",
            "Command validation / allowlist",
            "Transport abstraction",
        ],
        "note": "Includes a local `clarity-dev` MCP server for build tasks.",
    },
    {
        "id": "clarity-llm",
        "name": "clarity-llm",
        "type": "llm",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract", "clarity-mcp", "clarity-memory", "clarity-secrets"],
        "consumed_by": ["clarity-core", "clarity-mobile-core", "clarity-anthropic-proxy"],
        "summary": "LLM provider abstraction + Candle GGUF local inference.",
        "responsibilities": [
            "Provider registry",
            "`ReliableProvider` retry/failover",
            "`runtime_router` alias routing",
            "Candle GGUF local inference",
            "OAuth device flow auth",
        ],
        "note": "Features: `local-llm`, `local-llm-cuda`.",
    },
    {
        "id": "clarity-tools",
        "name": "clarity-tools",
        "type": "tools",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract", "clarity-memory"],
        "consumed_by": ["clarity-core"],
        "summary": "Built-in tool library.",
        "responsibilities": [
            "File tools",
            "Shell/PowerShell tools",
            "Web search/fetch",
            "Devkit tools",
            "Task/team tools",
        ],
        "note": "Split out from clarity-core to keep core smaller.",
    },
    {
        "id": "clarity-channels",
        "name": "clarity-channels",
        "type": "channels",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract"],
        "consumed_by": ["clarity-core"],
        "summary": "External communication channel abstraction.",
        "responsibilities": [
            "WeChat iLink (`chkit`) implementation",
            "Webhook adapter (enabled by default)",
            "Discord/Slack/Telegram stubs (disabled pending rustls-webpki fix)",
        ],
        "note": "Not a full multi-channel bot matrix.",
    },
    {
        "id": "clarity-secrets",
        "name": "clarity-secrets",
        "type": "secrets",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract"],
        "consumed_by": ["clarity-llm", "clarity-core"],
        "summary": "Encrypted secret storage using ChaCha20-Poly1305.",
        "responsibilities": [
            "`enc2:` key encryption/decryption",
            "Local keyring integration",
        ],
        "note": "Used by `models.toml` per-alias encrypted keys.",
    },
    {
        "id": "clarity-subagents",
        "name": "clarity-subagents",
        "type": "subagents",
        "layer": "infrastructure",
        "depends_on": ["clarity-core"],
        "consumed_by": [],
        "summary": "Sub-agent executor and parallel scheduler.",
        "responsibilities": [
            "`SubAgentManager`",
            "`AgentPool`",
            "Team coordination",
            "Parallel execution",
        ],
        "note": "Consumes clarity-core; not a dependency of core.",
    },
    {
        "id": "clarity-rollout",
        "name": "clarity-rollout",
        "type": "rollout",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract"],
        "consumed_by": ["clarity-thread-store"],
        "summary": "JSONL rollout persistence for thread event logs.",
        "responsibilities": [
            "`RolloutRecorder`",
            "`RolloutItem`",
            "Compaction/replacement history",
            "Event replay",
        ],
        "note": "API design inspired by OpenAI Codex; original Clarity implementation.",
    },
    {
        "id": "clarity-thread-store",
        "name": "clarity-thread-store",
        "type": "thread-store",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract", "clarity-rollout"],
        "consumed_by": ["clarity-core"],
        "summary": "Thread persistence abstraction.",
        "responsibilities": [
            "`ThreadStore` trait",
            "`LocalThreadStore`",
            "`LiveThread`",
            "Thread lifecycle persistence",
        ],
        "note": "Depends on clarity-rollout for JSONL event logs.",
    },
    {
        "id": "clarity-telemetry",
        "name": "clarity-telemetry",
        "type": "telemetry",
        "layer": "infrastructure",
        "depends_on": ["clarity-contract"],
        "consumed_by": ["clarity-gateway"],
        "summary": "Unified telemetry: WideEvent, metrics, traces, config audit.",
        "responsibilities": [
            "`WideEvent`",
            "SQLite/GreptimeDB sinks",
            "Tracing layer",
            "Config audit",
        ],
        "note": "Currently used by clarity-gateway.",
    },
    {
        "id": "clarity-core",
        "name": "clarity-core",
        "type": "core",
        "layer": "kernel",
        "depends_on": [
            "clarity-contract",
            "clarity-wire",
            "clarity-memory",
            "clarity-knowledge",
            "clarity-mcp",
            "clarity-llm",
            "clarity-tools",
            "clarity-channels",
            "clarity-secrets",
            "clarity-thread-store",
        ],
        "consumed_by": [
            "clarity-gateway",
            "clarity-egui",
            "clarity-tui",
            "clarity-claw",
            "clarity-headless",
            "clarity-mobile-core",
            "clarity-subagents",
            "clarity-telemetry",
            "clarity-anthropic-proxy",
        ],
        "summary": "Agent kernel: ReAct/Plan loop, Approval, Skill, MCP integration.",
        "responsibilities": [
            "Agent loop (`Agent`, `AgentController`, `Op`)",
            "ReAct/Plan execution",
            "Streaming event dispatch",
            "Approval runtime (Interactive/Smart/Plan/Yolo)",
            "Skill loading/discovery",
            "MCP integration",
            "Background task management",
            "Thread/session lifecycle",
            "`ViewState` UI state machine",
        ],
        "note": "Must have zero dependencies on frontend or network crates.",
    },
    {
        "id": "clarity-gateway",
        "name": "clarity-gateway",
        "type": "gateway",
        "layer": "presentation",
        "depends_on": ["clarity-core", "clarity-wire", "clarity-memory", "clarity-telemetry"],
        "consumed_by": [],
        "summary": "Axum HTTP/WebSocket server and Web IDE.",
        "responsibilities": [
            "Public API on :18790",
            "Admin + Web UI on :18800",
            "Session store",
            "SSE/WebSocket endpoints",
            "MCP server exposure",
        ],
        "note": "Can be built as bin or lib.",
    },
    {
        "id": "clarity-egui",
        "name": "clarity-egui",
        "type": "egui",
        "layer": "presentation",
        "depends_on": ["clarity-core", "clarity-wire"],
        "consumed_by": [],
        "summary": "Primary desktop GUI (eframe/egui, pure Rust).",
        "responsibilities": [
            "Main desktop window",
            "Three-rail Pretext layout",
            "Message bubbles",
            "Settings UI",
            "Design system",
        ],
        "note": "Zero Web dependencies.",
    },
    {
        "id": "clarity-tui",
        "name": "clarity-tui",
        "type": "tui",
        "layer": "presentation",
        "depends_on": ["clarity-core", "clarity-wire"],
        "consumed_by": [],
        "summary": "Terminal UI using ratatui.",
        "responsibilities": [
            "Terminal interface",
            "Command registry",
            "Keyboard routing",
            "Protocol renderer",
        ],
        "note": "Preferred for remote/SSH use.",
    },
    {
        "id": "clarity-claw",
        "name": "clarity-claw",
        "type": "claw",
        "layer": "presentation",
        "depends_on": ["clarity-contract"],
        "consumed_by": ["clarity-egui"],
        "summary": "Unified client-side Claw node: UI-agnostic library + system-tray binary.",
        "responsibilities": [
            "Gateway WebSocket client",
            "OpenClaw/KimiClaw JSON-RPC compatibility layer",
            "Device discovery / identity / pairing",
            "Role-context sync",
            "Tray icon and OS notifications",
            "Task monitoring",
        ],
        "note": "Merged from former clarity-openclaw; internal Clarity mesh uses Gateway WebSocket, OpenClaw JSON-RPC is external fallback.",
    },
    {
        "id": "clarity-headless",
        "name": "clarity-headless",
        "type": "headless",
        "layer": "presentation",
        "depends_on": ["clarity-core"],
        "consumed_by": [],
        "summary": "Headless CLI for scripts and CI.",
        "responsibilities": [
            "`run` subcommand",
            "`jumpy` subcommand",
            "JSON/Markdown output",
            "Stdin pipe support",
        ],
        "note": "Single binary, no GUI.",
    },
    {
        "id": "clarity-mobile-core",
        "name": "clarity-mobile-core",
        "type": "mobile-core",
        "layer": "presentation",
        "depends_on": ["clarity-core", "clarity-wire", "clarity-memory", "clarity-contract", "clarity-llm"],
        "consumed_by": [],
        "summary": "Mobile FFI core for Android/iOS.",
        "responsibilities": [
            "UniFFI bridge",
            "Runtime/events/config/memory APIs",
            "Kotlin/Swift bindings",
        ],
        "note": "Full Android/iOS UI is still in roadmap; `local-llm` disabled by default for mobile ABI.",
    },
    {
        "id": "clarity-slint",
        "name": "clarity-slint",
        "type": "slint",
        "layer": "experimental",
        "depends_on": ["clarity-contract", "clarity-wire"],
        "consumed_by": [],
        "summary": "Experimental Slint desktop GUI.",
        "responsibilities": [
            "Alternative desktop GUI",
        ],
        "note": "Does NOT consume clarity-core. Excluded from default CI.",
    },
    {
        "id": "clarity-anthropic-proxy",
        "name": "clarity-anthropic-proxy",
        "type": "anthropic-proxy",
        "layer": "utility",
        "depends_on": ["clarity-contract", "clarity-core", "clarity-llm"],
        "consumed_by": [],
        "summary": "Anthropic Messages API → DeepSeek proxy utility.",
        "responsibilities": [
            "Translate Anthropic requests to DeepSeek",
            "Tool/schema conversion",
            "Streaming response adaptation",
        ],
        "note": "Utility binary (`cc-proxy`).",
    },
    {
        "id": "clarity-tauri",
        "name": "clarity-tauri",
        "type": "tauri",
        "layer": "archived",
        "depends_on": [],
        "consumed_by": [],
        "summary": "Archived Tauri desktop frontend.",
        "responsibilities": [],
        "note": "Excluded from workspace. Do not modify.",
    },
]


def write_concept(crate: dict) -> None:
    path = CONCEPTS / f"{crate['id']}.md"
    deps = ", ".join(f'"{d}"' for d in crate["depends_on"]) or '""'
    consumers = ", ".join(f'"{c}"' for c in crate["consumed_by"]) or '""'
    resp_list = "\n".join(f"- {r}" for r in crate["responsibilities"]) or "- (none)"

    content = f"""---
id: {crate['id']}
name: {crate['name']}
type: {crate['type']}
layer: {crate['layer']}
depends_on: [{deps}]
consumed_by: [{consumers}]
---

# {crate['name']}

{crate['summary']}

## Responsibilities

{resp_list}

## Notes

{crate['note']}
"""
    path.write_text(content, encoding="utf-8")
    print(f"Generated {path}")


def write_index() -> None:
    layers = {}
    for crate in CRATES:
        layers.setdefault(crate["layer"], []).append(crate["id"])

    layer_entries = "\n".join(
        f"  {layer}:\n" + "\n".join(f"    - {cid}" for cid in sorted(ids))
        for layer, ids in sorted(layers.items())
    )

    concept_links = "\n".join(
        f"- [`{c['name']}`](concepts/{c['id']}.md) — {c['summary']}"
        for c in CRATES
    )

    index = f"""---
id: clarity-worktree
name: Clarity Project Worktree
description: OKF knowledge bundle describing the Clarity crate topology, responsibilities, and dependency graph.
version: 0.3.5-rc
date: 2026-07-06
source_repo: https://github.com/juice094/clarity
---

# Clarity Project Worktree

This OKF bundle describes the 21 crate directories in the Clarity workspace:
20 active workspace members + 1 archived (`clarity-tauri`), plus the
`tests/integration` integration-test crate.

## Layers

{layer_entries}

## Concepts

{concept_links}

## Key Invariants

1. `clarity-core` has zero dependencies on any frontend or network crate.
2. `clarity-contract` has zero internal dependencies.
3. Frontend crates never import each other; cross-frontend state/events go
   through `clarity-wire`.
4. `clarity-slint` depends only on `clarity-contract` + `clarity-wire`; it does
   not consume `clarity-core`.

## Living Sources of Truth

- Operational context & test baselines: [`AGENTS.md`](../../AGENTS.md)
- Code-accurate architecture: [`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md)
- Module-level topology: [`docs/architecture/map-topology.md`](../../architecture/map-topology.md)
"""
    (BASE / "index.md").write_text(index, encoding="utf-8")
    print(f"Generated {BASE / 'index.md'}")


if __name__ == "__main__":
    CONCEPTS.mkdir(parents=True, exist_ok=True)
    (BASE / "relations").mkdir(parents=True, exist_ok=True)
    for crate in CRATES:
        write_concept(crate)
    write_index()
    print("OKF bundle generated.")
