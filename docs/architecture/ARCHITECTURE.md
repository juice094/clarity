---
title: Clarity Architecture — Master Index
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# Clarity Architecture — Master Index

> **Status**: Living document | **Last revised**: 2026-05-15 (post-S7 Phase 3A)
> **Audience**: New contributors, AI agents, technical reviewers
> **Scope**: Cross-crate architecture, design contracts, decision rationales

This document is the **single entry point** to Clarity's architecture. Specific
subsystems each have their own deep-dive note linked below.

---

## 1. Project Layout

Clarity is a Rust workspace with the following crates (top-down dependency):

```
clarity-egui  (binary)  ──┐
clarity-tui   (binary)  ──┤
clarity-gateway         ──┼─► clarity-core (the "kernel")
clarity-headless (bin)  ──┤        │
clarity-claw  (binary)  ──┘        ├─► clarity-memory
                                   ├─► clarity-llm
                                   ├─► clarity-tools
                                   ├─► clarity-mcp
                                   ├─► clarity-subagents
                                   ├─► clarity-wire   (protocol layer)
                                   └─► clarity-contract (37 LOC type bridge)
```

- **`clarity-core`** is the agent kernel: state, planning, tool dispatch, LLM
  abstraction, approval, snapshot/restore, BTM, cron, subagents.
- **`clarity-egui` / `clarity-tui` / `clarity-gateway`** are alternative
  frontends that all consume `core::ui::ViewState` and `core::ui::RenderLine`.
- **`clarity-contract`** is a minimal type bridge to avoid circular deps.
- **`clarity-memory`** owns BM25 + cosine index + session persistence.

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
| 008 | Brain / hands / session decoupling | Accepted |
| 009 | Icon font strategy | Accepted |
| 010 | Lucide icons adoption | Accepted |
| 011 | Workspace architecture (3-tier) | Accepted |
| 012 | `RenderLine` enum design (13 variants) | Accepted |
| 013 | Keyboard shortcuts (ClaudeCode-inspired) | Accepted |
| 014 | Side panel tab consolidation (Tab D) | Accepted |
| 015 | `EndpointDescriptor` abstraction (Persona/Site/Frontend unified) | Accepted |

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
cargo test --workspace --lib              # All unit tests
cargo clippy --workspace -- -D warnings   # Zero warnings policy
cargo fmt --all -- --check                # Style check
cargo audit                               # Security scan
```

### 4.3 Release workflow

`v*` tag push triggers `.github/workflows/release.yml`:
- Windows: signed `clarity-egui.exe` + `clarity-headless.exe`
- Linux: `clarity-egui` + `clarity-headless`
- Both attached to GitHub Release

Latest: v0.3.3 (2026-05-15).

---

## 5. Roadmap & Plans

📄 Master schedule: [`../plans/2026-05-14-master-schedule.md`](../plans/2026-05-14-master-schedule.md)
📄 BACKLOG: [`../plans/BACKLOG.md`](../plans/BACKLOG.md)
📄 Pretext UI evolution: [`../plans/2026-05-12-pretext-ui-evolution.md`](../plans/2026-05-12-pretext-ui-evolution.md)
📄 Tri-role kernel extraction: [`../plans/2026-05-11-trirole-kernel-architecture-extraction.md`](../plans/2026-05-11-trirole-kernel-architecture-extraction.md)

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

See [`../plans/2026-05-14-master-schedule.md#9-hard-veto-边界不可逾越`](../plans/2026-05-14-master-schedule.md) for rationale.

---

*This document is maintained by AI agents; humans may edit directly. On every
session boot, first reconcile against the master schedule's "current phase
anchor" table.*
