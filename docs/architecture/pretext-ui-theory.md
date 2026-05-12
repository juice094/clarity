# Pretext UI Theory — Why Clarity Blurs the TUI/GUI Boundary

> **Date**: 2026-05-12
> **Status**: Accepted as architectural north star
> **Scope**: All future UI work in `clarity-egui`, `clarity-tui`, and `clarity-core/src/ui`
> **Anchors**: `docs/plans/2026-05-12-pretext-ui-evolution.md` (execution plan), `docs/audits/2026-05-12-ui-design-audit.md` (Phase 0.5 audit)

This document captures the **strategic rationale** for the Pretext UI evolution.
Tactical phasing lives in the plan; this document explains *why* we accept the
investment cost.

---

## 1. Thesis

> **Blurring the TUI/GUI boundary is not an aesthetic preference — it is an
> information architecture discipline.**

Traditional GUI ≈ 30% information + 70% decoration.
Traditional TUI ≈ 100% information.
Pretext UI ≈ 90% information + 10% tasteful decoration.

The blurred boundary is a **forcing function**: if a piece of state cannot be
expressed in the TUI, it is almost certainly decoration in the GUI. Removing it
is a maintenance win, not a feature loss.

---

## 2. The Six-Dimension Advantage Matrix

### Dimension 1 — Architecture (highest leverage)

| Benefit | Mechanism | Clarity impact |
|---------|-----------|----------------|
| Single source of truth | `ViewState`/`CommandItem`/`RenderLine` shared across renderers | Business logic ×1, renderers ×N |
| Enumerable state | TUI demands tractable state machine → forces GUI to be tractable too | 33 booleans collapse to ~5 enums |
| Cross-renderer testing | `to_text()` snapshot diff replaces pixel comparison | CI time ↓80%, no flaky tests |
| Graceful degradation | GUI crash → fallback to TUI; remote sessions get first-class support | Error recovery free |

### Dimension 2 — User Experience

| Benefit | Audience |
|---------|----------|
| Keyboard-first (j/k/g/G/Enter) | Long-session users, developers |
| Unified mental model | Local GUI ↔ SSH TUI switch with zero retraining |
| Screen-reader friendly | Visually impaired users become first-class citizens free of charge |
| Low learning curve | Command palette = self-documenting affordance |

### Dimension 3 — AI Capability (Clarity's asymmetric win)

This is the dimension that makes Pretext UI **structurally aligned** with
Clarity's mission as an AI agent framework:

```
Agent reads its own UI = reads text, not OCR'd screenshots
Agent debugs itself    = reads Vec<RenderLine>, no vision model required
State -> prompt        = "Current UI:\n<attached lines>"
```

For a video editor, this dimension is irrelevant. For an AI agent framework,
it is the **core capability** that turns the UI from a black box into an
introspectable system. Without it, the agent can build UI but cannot reason
about its own UI behavior.

### Dimension 4 — Operations

| Scenario | Pretext UI behavior |
|----------|---------------------|
| SSH remote | Native (no X forwarding / VNC) |
| Low-bandwidth / edge | TUI path automatic (ratatui runs on RP2040) |
| Terminal logs | UI state = log entry = UI replay (trinity) |
| Test automation | Same fixtures validate GUI/TUI/CI |

### Dimension 5 — Engineering

| Benefit | Source |
|---------|--------|
| Compile portability | Core crate decoupled from renderer |
| Dead-code detection | One business logic copy -> static analysis trivial |
| Designer/developer collaboration | Designer edits RenderLine semantics; developers implement two renderers |
| Müller-Brockmann grid discipline | "Through structure comes freedom" — eliminates visual entropy |

### Dimension 6 — Philosophical (long horizon)

- **Anti-entropy**: text endures; aesthetic trends do not. `RenderLine` will
  still parse in 10 years; the 2026 rounded-corner fashion will not.
- **Transparency**: system behavior is readable. No black box.
- **Reversibility**: text-first state always round-trips through
  serialization.

---

## 3. Where Pretext UI Wins, Where It Loses

### Where it wins (= Clarity's profile)

- AI agent framework (content is text by nature)
- Developer tooling (audience accepts keyboard-first)
- Long sessions (10K+ messages, virtual scrolling matters)
- Remote/local hybrid usage (cloud agents + local IDE)
- Agent self-introspection requirement (core capability)

### Where it loses (not Clarity)

- Mass-market SaaS with non-developer users
- Design tools (Figma/Photoshop — content is fundamentally visual)
- Games (experience = visual immersion)
- Rich-media-first products (video, animation, drag-drop heavy)

**Conclusion**: Clarity is in the minority of products where all six
dimensions of advantage compound, and where the structural costs of blurring
are absent. This is the load-bearing claim of the strategy.

---

## 4. Borrowed Concepts (and what we keep distinct)

### From Claude (UI philosophy)

| Concept | Adoption form |
|---------|---------------|
| Slash commands (`/help`, `/clear`, `/init`) | `CommandRouter` with slash-prefix detection in Input panel |
| Inline tool-call rendering | `RenderLine::ToolCallHeader` + `ToolCallArg` (Phase 2) |
| Artifacts side panel | Workspace evolution: code blocks can promote to persistent artifact |
| Memory affordances | Per-message pinned/ephemeral indicator |
| Project context in chrome | TitleBar shows `workspace / branch / dirty count` |
| Streaming with cursor | Word-by-word with cursor token |
| Prompt-able UI | Every UI action is a `CommandId`; agent can invoke as if user |

### From TUI ecosystem (engineering rigor)

| Concept | Source | Adoption |
|---------|--------|----------|
| Constraint solver | Cassowary / ratatui kasuari | `egui_extras::StripBuilder` for chrome (Phase 1) |
| Line-based buffer | Helix / Emacs / xi-editor | `RenderLine` enum (Phase 2) |
| Baseline grid | TeX box/glue/penalty | Per-role `line_height` token |
| Box-drawing decoration | Modern TUIs (lazygit, k9s) | TUI renderer uses Unicode boxes (Phase 3) |

### What stays distinct — egui's pixel-decoration advantages

We **keep** these GUI-only affordances. They are the 10% decoration in our
"90% information + 10% decoration" ratio:

- Rounded corners on chrome (`theme.radius_*`)
- Focus rings (`theme.focus_ring`)
- Hover states with smooth color transition
- Native window controls (close/minimize/maximize)
- Window drag from TitleBar
- Drag-to-resize side panels
- File drag-drop into chat
- Image / icon rendering
- Modal dialogs with backdrop
- Mouse-driven affordances (scrub, drag-reorder)

### What stays distinct — TUI's text-purity advantages

- ANSI color and box-drawing instead of pixel decoration
- SSH-native, no X forwarding
- Pipe-able state (`clarity-tui --dump-state | jq`)
- Programmable navigation (any key sequence scriptable)
- Built-in screen-reader compatibility

---

## 5. Information vs Decoration Test

For any UI element, ask: **does this element fail gracefully when reduced to
text?**

| Element | Pretext test | Verdict |
|---------|--------------|---------|
| Message body | `"User: hello"` reads same as bubble | Information |
| Code block | Renders fine without syntax color | Information |
| Tool call | `> search(query=foo) -> 12 results` reads same as expanded card | Information |
| Hover tooltip | "Click to start Gateway" -> footer hint in TUI | Information |
| Rounded corner | Cannot reduce | Decoration (keep in GUI only) |
| Drop shadow | Cannot reduce | Decoration |
| Smooth scroll | Discrete line in TUI | Decoration (GUI bonus) |
| Avatar image | Falls back to `[A]` initials | Decoration (graceful) |

**Rule**: if reducing to text destroys meaning, the element is **decoration**
and lives in the GUI renderer only. If reducing preserves meaning, the element
is **information** and lives in `clarity-core::ui::RenderLine`.

---

## 6. Anti-Patterns to Reject

This is what we will **not** do, despite ecosystem pressure:

- Building UI as "components" that own both data and rendering (React-style coupling)
- Pixel-perfect designs that cannot be expressed as text first
- Renderer-specific state (no `egui::Memory` for business state — only for ephemeral focus / scroll position)
- Streaming text rendering that re-parses the full message every frame
- "Skinning" the TUI to look like GUI (or vice versa). Each renderer is honest about its medium.

---

## 7. References

### Academic foundation (Kimi K2.6 conversation context, 2026-05-12)

- **Sketchpad (Sutherland, 1963)** — constraint satisfaction as UI primitive
- **Cassowary (Borning et al., 1997 UIST)** — incremental linear constraints
- **Grid Systems in Graphic Design (Müller-Brockmann, 1966)** — "freedom through structure"
- **Immediate Mode GUI as State Monad (2024)** — formal proof IMGUI can host declarative sublayers

### Engineering precedent

- iOS/macOS Auto Layout (billions of devices on Cassowary)
- ratatui (Rust Cassowary port, runs on RP2040 microcontroller)
- egui_extras::StripBuilder (official egui constraint primitive)
- Helix editor (line-based buffer with incremental updates)

### Internal references

- `docs/plans/2026-05-12-pretext-ui-evolution.md` — execution plan with 5 phases
- `docs/audits/2026-05-12-ui-design-audit.md` — Phase 0.5 audit, 6-axis review
- `crates/clarity-egui/EGUI_LAYOUT.md` — layout rules including 5 traps
- `~/.config/agents/skills/egui-layout-canons/SKILL.md` — skill protocol

---

## 8. Decision Record

| Date | Decision | Source |
|------|----------|--------|
| 2026-05-12 | Adopt Pretext UI as architectural north star | This document |
| 2026-05-12 | Six-dimension advantage matrix is the value justification | §2 |
| 2026-05-12 | Clarity's profile uniquely fits Pretext UI | §3 |
| 2026-05-12 | Borrow from Claude philosophy; keep egui pixel decoration distinct | §4 |
| 2026-05-12 | Information-vs-decoration test gates new UI work | §5 |
