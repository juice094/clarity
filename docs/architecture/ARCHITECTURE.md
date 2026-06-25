---
title: Clarity Architecture — Master Index
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# Clarity Architecture — Master Index

> **Status**: Living document | **Last revised**: 2026-06-25
> **Audience**: New contributors, AI agents, technical reviewers
> **Scope**: Cross-crate architecture, design contracts, decision rationales

This document is the **index entry point** to Clarity's architecture. For the
code-accurate crate topology and worktree, see [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
and [`../../AGENTS.md`](../../AGENTS.md) §3 / §A. Specific subsystems each have
their own deep-dive note linked below.

---

## 1. Project Layout

Clarity is a Rust workspace with **23 crate directories** (22 active workspace
members + 1 archived `clarity-tauri`). See [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
§2 for the authoritative crate topology. The dependency direction is:

```
clarity-contract (zero internal deps)
    ▲
    ├── {wire, memory, mcp, llm, tools, channels, secrets, openclaw, rollout}
    ├── thread-store (→ rollout)
    │
    ▼
  core ← {gateway, egui, tui, claw, headless, mobile-core}
    ▲
    ├── subagents (consumes core)
    └── telemetry (currently used by gateway)

# Experimental / utility:
clarity-slint ← {contract, wire}
clarity-anthropic-proxy (utility binary)
```

- **`clarity-core`** is the agent kernel: state, planning, tool dispatch, LLM
  abstraction, approval, snapshot/restore, background tasks, subagents.
- **`clarity-egui` / `clarity-tui` / `clarity-gateway` / `clarity-claw` /
  `clarity-headless` / `clarity-mobile-core`** are alternative frontends that
  communicate through `clarity-wire`.
- **`clarity-slint`** is an experimental Slint frontend that depends only on
  `clarity-contract` + `clarity-wire`; it does **not** consume `clarity-core`.
- **`clarity-contract`** is the shared type/trait contract with zero internal
  dependencies.
- **`clarity-memory`** owns BM25 + vector + session persistence.

---

## 2. Cross-Cutting Subsystems

### 2.1 UI State Machine — `ViewState`

A typed enum-based state machine that replaces 50+ boolean flags from the
pre-Pretext era. Single source of truth for view state shared between GUI and
TUI.

📄 Deep dive: [`viewstate-migration.md`](viewstate-migration.md)

Key types: `ViewState`, `AppView`, `SidePanel`, `ModalType`, `TurnState`,
`PanelExpansion`, `FocusScope`, `PanelKind`.

Invariants (enforced by type system + 44 tests):
- At most one modal at a time.
- Right panel is mutually exclusive (ADR-014 Tab D).
- `TurnState` is exhaustive enum — `Loading + Compacting` is unrepresentable.
- Modal focus blocks panel focus overrides.

### 2.2 Render Pipeline — `RenderLine`

13-variant enum that absorbs 30+ markdown/UI line patterns. The atomic
unit of chat rendering, shared between GUI (egui) and TUI (ratatui).

📄 Deep dive: [`renderline-pipeline.md`](renderline-pipeline.md)
📄 Decision record: [`../adr/ADR-012-renderline-enum-design.md`](../adr/ADR-012-renderline-enum-design.md)

Pipeline:
```
markdown string
  → clarity_core::ui::markdown_to_lines()    (pulldown-cmark)
  → Vec<RenderLine>                          (frontend-neutral)
  → render_line_to_ratatui()  | line_renderer::render_lines()
  → ratatui Text              | egui frame
```

Parity contract: both frontends must yield identical plain text when projected
via `render_line_plain_text()`. Enforced by 19 cross-renderer tests
(7 in `clarity-core` + 12 in `clarity-tui`).

### 2.3 Keyboard Routing — `ShortcutRegistry`

Focus-aware key dispatch table per ADR-013. Resolves keystrokes against the
current `FocusScope` and picks the most-specific binding.

📄 Deep dive: [`shortcut-focus-routing.md`](shortcut-focus-routing.md)
📄 Decision record: [`../adr/ADR-013-keyboard-shortcuts-claudecode-inspired.md`](../adr/ADR-013-keyboard-shortcuts-claudecode-inspired.md)

Specificity hierarchy: `Widget(5) > Panel(4) > Modal(3) > App(2) > Os(1)`.

LIFO override semantics: later `register()` calls with identical (key, scope)
replace earlier bindings.

### 2.4 Pretext UI Theory

The macro-level philosophy: every UI element must be reducible to text
(information vs decoration test). Drives the discrete line-based data model.

📄 Deep dive: [`pretext-ui-theory.md`](pretext-ui-theory.md)

### 2.5 UI Axis (grid vs cursor)

Classification of UI affordances by interaction grain (grid-based panels vs
cursor-based text flow).

📄 Deep dive: [`ui-axis.md`](ui-axis.md)

### 2.6 Endpoint Abstraction — `EndpointDescriptor`

A unified contract for **any addressable endpoint** in the Clarity ecosystem:
in-process personas (Kin / Analyst / Programmer), browser-mediated AI sites
(ChatGPT / Claude / Gemini / DeepSeek — OpenTeam-Core), and frontend adapters
(GUI / TUI / Headless).

📄 Decision record: [`../adr/ADR-015-endpoint-descriptor-abstraction.md`](../adr/ADR-015-endpoint-descriptor-abstraction.md)
📄 Source: `crates/clarity-core/src/endpoint.rs`

Why this exists: every switcher widget (Persona switcher in the GUI Top Bar,
Site selector in OpenTeam-Core) needs the same conceptual shape — id + display
metadata + capabilities + dispatch kind. Sharing one descriptor eliminates
triplicated UI code and divergent serde schemas.

Capability flags (`Chat | Coding | Analysis | Browse | Vision | ToolUse | Planning`)
drive UX gating: when the active endpoint lacks `Browse`, the browser panel
auto-disables.

---

## 3. Architecture Decision Records (ADRs)

All major decisions are recorded under `../adr/`. Active ADRs:

| # | Topic | Status |
|---|-------|--------|
| 001 | Tauri → egui migration | Accepted |
| 002 | `std::sync` → `parking_lot` | Accepted |
| 003 | `clarity-contract` extraction | Accepted |
| 004 | `rustls` TLS replacement | Accepted |
| 005 | Subagent / core decoupling | Accepted |
| 006 | Protocol layer convergence | Accepted |
| 007 | Turn ID in `WireMessage` | Accepted |
| 008 | Brain / hands / session decoupling | Accepted |
| 009 | Icon font strategy | Accepted |
| 010 | Lucide icons adoption | Accepted |
| 011 | Workspace architecture (3-tier) | Accepted |
| 012 | `RenderLine` enum design (13 variants) | Accepted |
| 013 | Keyboard shortcuts (ClaudeCode-inspired) | Accepted |
| 014 | Side panel tab consolidation (Tab D) | Accepted |
| 015 | `EndpointDescriptor` abstraction (Persona/Site/Frontend unified) | Accepted |
| 016 | Pretext three-column layout | Accepted |
| 017 | Claw architecture review | Accepted |
| 018 | Session-scoped event routing | Accepted |

---

## 4. Build / Test / Release

### 4.1 Local build

```bash
cargo build --release -p clarity-egui     # GUI binary
cargo build --release -p clarity-tui      # TUI binary
cargo build --release -p clarity-headless # Headless binary
```

### 4.2 Pre-merge checks

```bash
cargo test --workspace --lib --exclude clarity-slint              # Unit tests
cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2
cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2
cargo test -p clarity-integration-tests --lib
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo fmt --all -- --check
cargo audit --deny unsound --deny yanked
```

### 4.3 Release workflow

`v*` tag push triggers `.github/workflows/release.yml`:
- Windows: signed `clarity-egui.exe` + `clarity-headless.exe`
- Linux: `clarity-egui` + `clarity-headless`
- Both attached to GitHub Release

Latest: v0.3.3 (2026-05-15).

---

## 5. Roadmap & Plans

📄 Master schedule: [`../plans/2026-05-14-master-schedule.md`](../planning/plans/2026-05-14-master-schedule.md)
📄 BACKLOG: [`../plans/BACKLOG.md`](../planning/BACKLOG.md)
📄 Pretext UI evolution: [`../plans/2026-05-12-pretext-ui-evolution.md`](../planning/plans/2026-05-12-pretext-ui-evolution.md)
📄 Tri-role kernel extraction: [`../plans/2026-05-11-trirole-kernel-architecture-extraction.md`](../planning/plans/2026-05-11-trirole-kernel-architecture-extraction.md)

---

## 6. Hard Veto Boundaries

These constraints are non-negotiable:

1. **Local LLM first** — every feature must work offline; cloud is optional.
2. **Zero data exfiltration** — API keys never leave the local machine.
3. **No Docker / Electron** — pure Rust binary, no container or browser runtime.
4. **No RAG vector DB** — SQLite + BM25 + cosine index suffice.
5. **Project breadth ≤ 5 core tools** — new features replace old ones.
6. **Rust core stays in-house** — subagents may research, but core code must
   be reviewed by the primary agent.

See [`../plans/2026-05-14-master-schedule.md#9-hard-veto-边界不可逾越`](../planning/plans/2026-05-14-master-schedule.md) for rationale.

---

*This document is maintained by AI agents; humans may edit directly. On every
session boot, first reconcile against the master schedule's "current phase
anchor" table.*
