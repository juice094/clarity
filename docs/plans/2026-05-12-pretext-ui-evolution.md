# Pretext UI Evolution Plan — Grid, Lines, State, and Claude-Borrowed Composition

> **Date**: 2026-05-12 (revised after Kimi K2.6 strategic review and 6-axis design audit)
> **Status**: Phase 0 ✅ · Phase 0.5 ✅ (audit + 5 P0 blockers, S1 done) · Phase 1-3 planned
> **Owner**: juice094 + Clarity Agent
> **Theory anchor**: `docs/architecture/pretext-ui-theory.md`
> **Audit anchor**: `docs/audits/2026-05-12-ui-design-audit.md`
> **Skill**: `~/.config/agents/skills/egui-layout-canons/SKILL.md`

---

## 0. Background and Motivation

The TitleBar regression session (2026-05-11 → 2026-05-12) required **four
consecutive fixes** on a single 36×36 chrome button. Five distinct death traps
were discovered, all rooted in the same source: command-mode cursor flow has
no systematic invariants for composite chrome widgets.

This plan formalizes a four-pronged evolution backed by the
`pretext-ui-theory.md` six-dimension advantage matrix:

1. **Grid for Chrome** (Phase 1) — Replace imperative
   `ui.horizontal_centered` / `estimated_right_w` / `ui.put` patterns with
   declarative constraint grids (StripBuilder).
2. **State Machine** (Phase 1.5) — Collapse the 33 legacy boolean flags into
   typed enum states. Necessary for engineering-dimension gains.
3. **Lines for Content** (Phase 2) — Replace pixel-continuous markdown streams
   with semantic discrete line sequences. Unlocks AI introspection.
4. **TUI Parity + Claude Composition** (Phase 3) — Achieve renderer parity and
   adopt Claude's slash-command / artifact / project-context affordances while
   keeping egui pixel decoration distinct.

Total effort: ~49h across 9-10 focused sessions.

---

## 1. Phase 0 — TitleBar Crisis Closure ✅

**Commits** (this session):
- `f6f8b93c` feat(core+egui): cross-platform UI state machine + CommandPalette
- `8ebe2947` feat(theme): layout & breakpoint tokens
- `14760ff6` fix(egui-titlebar): drop horizontal_centered + Pattern A
- `2a25a76f` refactor(egui-widgets): interactive_row → UiBuilder::sense()
- `db9195c5` docs(egui-layout): Production-Verified Traps appendix
- `fbacc5e3` docs(plans): Pretext UI evolution roadmap
- `4a4a6e1b` docs(audits): Phase 0.5 UI design audit

**External**:
- `~/.config/agents/skills/egui-layout-canons/SKILL.md` created
- `~/AGENTS.md` updated with intent routing for the skill

**Deliverables**: All 6 user-reported TitleBar functions verified working;
window controls precisely centered; ViewState state machine introduced;
CommandPalette accessible via Ctrl+Shift+P; 5 traps codified, 3 patterns shipped.

---

## 2. Phase 0.5 — Foundation Hardening ✅ (S1, ~4h, 1 session)

**Status**: Completed 2026-05-12. Commits `2df427e7` → `6eb23dae` (5 atomic commits).

> **Goal**: Land the 5 P0 blockers from the design audit before any new
> architectural work begins.
> **Gate**: Phase 1 cannot start until all 5 items merge. ✅ Gate cleared.

| ID | Item | Effort | Acceptance |
|----|------|--------|-----------|
| P0.5.F.1 | Add 8 chrome dimension tokens to `theme.rs` | 30min | `theme.window_default_w/h`, `window_min_w`, `window_edge_zone`, `size_sidebar_collapsed`, `size_tab_h`, `modal_offset_y`, `palette_w`, `palette_max_h` all exist; replace literal call-sites |
| P0.5.C.1 | Unify `ShortcutAction` ↔ `CommandItem` via `CommandId` | 1h | Shortcuts emit `CommandId(String)`; both shortcut layer and palette route through same `CommandRouter` |
| P0.5.C.2 | Wire `CommandPalette::execute()` to real `CommandRouter` | 1h | Clicking a palette command actually mutates app state (no more `tracing::info!` stub) |
| P0.5.E.1 | Focus ring on 5 widgets | 1h | `sidebar_card`, `provider_row`, `theme_card`, `status_capsule`, `tab_button` all draw `theme.focus_ring` when `response.has_focus()` |
| P0.5.E.3 | `.on_hover_text()` on TitleBar buttons | 15min | All 4 window-control buttons + sidebar toggle + dashboard toggle have tooltips |

**User-visible delta**: tooltips appear, focus ring visible while tabbing,
command palette actually executes commands.

**Commits planned**: 5 atomic commits, one per item.

**Skill update**: Add focus-ring section to `egui-layout-canons/SKILL.md`.

---

## 3. Phase 1 — Grid for Chrome ✅ (S2, ~3.8h actual vs ~5h budget, 1 session)

> **Completed 2026-05-12** — see `docs/plans/2026-05-12-S2-session-archive.md`.
> Phase scope expanded mid-session to include icon font migration
> (originally Phase 0.5 epilogue): ADR-010 superseded ADR-009 to adopt
> `lucide-icons` over `egui-phosphor`. 5 commits total. Tests 66/66.

> **Goal (achieved)**: Eliminated `estimated_right_w: 450/280` hardcoded
> heuristic from `render_titlebar`. Three-zone declarative StripBuilder
> layout now drives chrome.

### 3.1 Scope

| Region | File | Current | Target |
|--------|------|---------|--------|
| TitleBar | `main.rs::render_titlebar` | `ui.horizontal` + `estimated_right_w` (450/280) | `StripBuilder` LTR with `Size::exact` / `Size::remainder` |
| StatusBar | TBD (add minimal one if absent) | none | StripBuilder with progress + indicator slots |
| ToolBar | `panels/chat/header.rs::render_header` | `ui.horizontal` with manual spacing | StripBuilder |

### 3.2 Tasks

- [ ] **P1.0** PoC: replicate TitleBar layout with `StripBuilder` in
      `crates/clarity-egui/examples/strip_titlebar.rs` (1h)
- [ ] **P1.1** Add `egui_extras = "0.31"` to `crates/clarity-egui/Cargo.toml`
      with explicit features (`["all_loaders"]` likely not needed) (15min)
- [ ] **P1.2** Refactor `render_titlebar` to use `StripBuilder` (1.5h)
- [ ] **P1.3** Delete `estimated_right_w` heuristic and CENTER zone
      `allocate_ui_with_layout` workaround (15min)
- [ ] **P1.4** Refactor `render_status_bar` if exists; else add minimal one
      with StripBuilder (1h)
- [ ] **P1.5** Add RULE 6 to `EGUI_LAYOUT.md`: chrome regions must use
      StripBuilder (15min)
- [ ] **P1.6** Update `egui-layout-canons` SKILL.md with "When to Use Grid vs
      Cursor" decision tree (45min)

### 3.3 Acceptance Criteria

- TitleBar uses zero `estimated_*` constants
- All 6 functional tests pass at three window widths (600 / 900 / 1400 px)
- No `ui.horizontal_centered` remains in chrome rendering
- `cargo build` produces no new warnings
- TitleBar visual output is pixel-equivalent to Phase 0 baseline

### 3.4 Risk and Rollback

- **Risk**: StripBuilder API may differ from PoC assumptions
- **Mitigation**: P1.0 PoC must succeed before P1.1 commits
- **Rollback**: Keep Phase 0 TitleBar implementation behind feature flag
  `legacy-titlebar` for one release cycle

---

## 4. Phase 1.5 — State Machine Migration ⏳ (~6h, 1 session)

> **Goal**: Collapse the 33 legacy boolean flags into typed enum states.
> Engineering-dimension investment with long-horizon payoff.
> **Why now**: Phase 2 keyboard navigation needs predictable parent state;
> the boolean bridge layer in `main.rs:802` becomes unsustainable.

### 4.1 Scope

Inventory from `docs/audits/2026-05-12-ui-design-audit.md` §B.2:

| Category | Count | Target form |
|----------|-------|-------------|
| `*_panel_open` (8 booleans) | 8 | `SidePanel` enum already exists; migrate |
| `*_modal_open` (6 booleans) | 6 | `ModalType` enum already exists; migrate |
| `*_expanded/collapsed` (9 booleans) | 9 | Per-panel `PanelExpansion` struct |
| Workflow flags (`is_loading`, `compacting`, `stopping`) | 4 | `TurnState` enum |
| Style flags (`agent_turn_style`, `agent_turn_glass`, `mcp_changed`) | 3 | Keep as-is (theme-level concerns) |
| Misc | 3 | Case-by-case |

### 4.2 Tasks

- [ ] **P1.5.1** Inventory all 33 booleans into a doc table, classify
      keep / migrate / delete (1h)
- [ ] **P1.5.2** Define `PanelExpansion`, `TurnState` enums in `clarity-core` (1h)
- [ ] **P1.5.3** Migrate `*_panel_open` to `ViewState.left` / `.right` (1.5h)
- [ ] **P1.5.4** Migrate `*_modal_open` to `ViewState.modal` (1.5h)
- [ ] **P1.5.5** Migrate workflow flags to `TurnState` (30min)
- [ ] **P1.5.6** Remove compatibility bridge in `main.rs:802-810` (15min)
- [ ] **P1.5.7** Add state transition table to
      `docs/architecture/view-state-transitions.md` (15min)

### 4.3 Acceptance Criteria

- `grep -c "_open: bool\|_collapsed: bool" crates/clarity-egui/` drops from 33 to <5
- Compatibility bridge `// Sync legacy boolean flags with ViewState` removed
- All existing UI behavior preserved (smoke test: open every panel/modal once)
- Illegal states unreachable by construction (e.g., two modals open
  simultaneously fails to compile)

### 4.4 Risk and Rollback

- **Risk**: 8 panels touched; latent state coupling may surface
- **Mitigation**: Migrate one category per commit; smoke-test after each
- **Rollback**: Per-commit revert; bridge layer can be reintroduced in 1 commit

---

## 5. Phase 2 — Lines for Content ⏳ (3 sessions, ~18h)

> **Goal**: Replace `Message::parsed: Vec<RenderBlock>` with
> `Message::lines: Vec<RenderLine>`. Unlocks AI introspection (Dimension 3),
> virtual scrolling, and keyboard navigation.

### 5.1 Phase 2A — RenderLine Foundation (~6h)

- [ ] **P2A.1** Define `RenderLine` enum in
      `clarity-core/src/ui/render_line.rs` (1h)
- [ ] **P2A.2** Write `markdown_to_lines(md: &str) -> Vec<RenderLine>`
      converter using `pulldown-cmark` (3h)
- [ ] **P2A.3** Unit tests: basic markdown (headings, lists, code, tables)
      to lines (1.5h)
- [ ] **P2A.4** Document line model semantics in
      `docs/architecture/render-line.md` (30min)

### 5.2 Phase 2B — Renderer + Streaming (~6h)

- [ ] **P2B.1** Write `render_lines(ui, &[RenderLine], theme)` in
      `clarity-egui/src/ui/line_renderer.rs` (2h)
- [ ] **P2B.2** Implement exact-pixel virtual scrolling:
      `scroll_offset / line_height` (1.5h)
- [ ] **P2B.3** Implement keyboard navigation: `j` / `k` / `g` / `G` / `Enter` /
      `Esc` (2h)
- [ ] **P2B.4** Implement streaming append: per-line buffer flush on `\n`
      (30min)

### 5.3 Phase 2C — ChatArea Migration (~6h)

- [ ] **P2C.1** Add feature flag `line-mode` to toggle ChatArea rendering
      (15min)
- [ ] **P2C.2** Add `Message::lines: Vec<RenderLine>` field, populated in
      `prepare()` alongside (initially) `parsed: Vec<RenderBlock>` (2h)
- [ ] **P2C.3** ChatArea renders from `lines` when flag enabled (2h)
- [ ] **P2C.4** Migrate Sidebar and Workspace to line-rows (1.5h)
- [ ] **P2C.5** `BlockSlot` fallback wired for tables / images (15min)

### 5.4 Data Model

```rust
// In clarity-core/src/ui/render_line.rs (new)
pub enum RenderLine {
    Text { spans: Vec<Span>, role: LineRole, indent: u8 },
    CodeLine { lang: SmolStr, content: SmolStr, line_no: Option<u32> },
    ToolCallHeader { name: SmolStr, status: ToolStatus, expanded: bool },
    ToolCallArg { key: SmolStr, value: SmolStr },
    Divider,
    Empty,
    BlockSlot { block_id: BlockId, line_count: u8 },
}

pub enum LineRole {
    UserMessage,
    AgentMessage,
    SystemMessage,
    ErrorMessage,
    Heading(u8),
    Quote,
    ListItem(u8),
}
```

### 5.5 Acceptance Criteria

- ChatArea scrolling is pixel-perfect (no jitter, no jumps) at 10K messages
- `j` / `k` navigation works in ChatArea, Sidebar, Workspace
- Streaming text appends without re-parsing full message
- `Message::to_text()` returns deterministic text for AI introspection
- Existing markdown tests still pass with `line-mode` enabled
- 60fps maintained with 10K messages, 1MB total markdown

### 5.6 Risk and Rollback

- **Risk**: Tables, nested lists, HTML degrade in line form
- **Mitigation**: `BlockSlot` provides escape hatch to existing block renderers
- **Risk**: 18h is the largest single phase
- **Mitigation**: Split across 3 sessions (2A / 2B / 2C); each commits
  independently; feature flag keeps old path alive for 2 release cycles

---

## 6. Phase 3 — TUI Parity + Claude Composition ⏳ (3 sessions, ~16h)

> **Goal**: Achieve TUI/GUI feature parity and adopt Claude-inspired
> affordances while keeping egui pixel decoration distinct.

### 6.1 Phase 3A — TUI Parity (~6h)

- [ ] **P3A.1** Wire `clarity-tui` to `ViewState` + `RenderLine` (3h)
- [ ] **P3A.2** ANSI rendering for `RenderLine` with box-drawing chrome (2h)
- [ ] **P3A.3** Snapshot tests: same fixture renders to GUI + TUI; assert
      text content matches (1h)

### 6.2 Phase 3B — Claude-Inspired Composition (~6h)

This is the deep Claude borrow. Each item maps a Claude affordance to our
Pretext primitives while keeping egui's pixel-decoration advantages.

- [ ] **P3B.1** **Slash commands** in input panel (1h)
      - Detect `/` prefix in `InputPanel`
      - Open contextual dropdown showing matching `CommandItem`s from
        `CommandRouter`
      - Both GUI (dropdown) and TUI (autocomplete) render same source
- [ ] **P3B.2** **Artifacts panel** (2h)
      - Extend `Workspace` SidePanel with an `Artifact` type
      - Detect long code blocks in chat → "Open as artifact" affordance
      - Persistent across messages, with version history
      - GUI gets syntax highlighting; TUI gets monospace
- [ ] **P3B.3** **Project context display** in TitleBar CENTER zone (1h)
      - Show: `<workspace>/<branch> · <dirty count> staged`
      - Updates on filesystem watch events
      - TUI shows same in dedicated status line
- [ ] **P3B.4** **Memory affordances** per message (1h)
      - Pin/unpin via right-click (GUI) or `m` key (TUI)
      - Persisted via existing snapshot system
      - Visual indicator: pin icon (GUI) / `[pinned]` prefix (TUI)
- [ ] **P3B.5** **Streaming cursor** in agent messages (1h)
      - Word-by-word append with trailing cursor character
      - Smooth in GUI (animated opacity), discrete in TUI

### 6.3 Phase 3C — Documentation and Closure (~4h)

- [ ] **P3C.1** Write `docs/architecture/ui-axis.md` — per-panel
      grid-vs-cursor classification (1h)
- [ ] **P3C.2** Update `egui-layout-canons` SKILL with the full theory
      reference (1h)
- [ ] **P3C.3** CHANGELOG entries for all phases (30min)
- [ ] **P3C.4** Performance benchmark: GUI 60fps with 10K messages;
      TUI same data without lag (1h)
- [ ] **P3C.5** Cross-platform regression suite as CI gate (30min)

### 6.4 Acceptance Criteria

- TUI achieves feature parity with GUI for all 5 main views
- Slash commands work in both renderers
- Artifacts persist across sessions; restoration tested
- Project context updates within 100ms of git state change
- Streaming feels smooth in GUI, discrete in TUI (both at >60Hz refresh)
- `docs/architecture/` directory contains 4 design notes:
  `pretext-ui-theory.md`, `view-state-transitions.md`, `render-line.md`,
  `ui-axis.md`

### 6.5 Risk and Rollback

- **Risk**: TUI rendering layer is underbuilt; parity may surface core bugs
- **Mitigation**: Phase 3A is gated by Phase 2 stability; if Phase 2 has
  unresolved issues, Phase 3A waits
- **Risk**: Artifacts panel is a meaningful new feature, not a refactor
- **Mitigation**: Ship behind feature flag `artifacts`; default off

---

## 7. Cross-Phase Concerns

### 7.1 Backward Compatibility Matrix

| Phase | User-visible change | Compatibility strategy |
|-------|--------------------|-----------------------|
| 0.5 | Tooltips + focus ring + working palette | None needed (additive) |
| 1 | None (visual identical) | Keep Phase 0 path behind `legacy-titlebar` flag |
| 1.5 | None (refactor) | Per-category migration, atomic commits |
| 2 | New scrolling, j/k nav | `line-mode` feature flag; old `RenderBlock` path remains 2 releases |
| 3A | TUI feature parity | TUI was already alpha; no compat needed |
| 3B | New affordances (slash, artifacts, memory, project, cursor) | Each behind feature flag during alpha |

### 7.2 Performance Targets

- TitleBar repaint: < 100µs per frame (Phase 1)
- ChatArea: 60fps with 10K lines, 1MB markdown (Phase 2)
- Keyboard navigation: < 16ms response (Phase 2)
- TUI render: 60Hz refresh, < 50ms full redraw (Phase 3)

### 7.3 Documentation Cadence

Each phase produces:
- One commit to `docs/plans/` updating status
- One section in `~/.config/agents/skills/egui-layout-canons/SKILL.md`
- One rule extension in `crates/clarity-egui/EGUI_LAYOUT.md` (if applicable)
- One ADR-style note in `docs/architecture/` for new architectural decisions

---

## 8. Session-Level Schedule

| Session | Phase | Hours | Deliverable |
|---------|-------|-------|-------------|
| S1 | 0.5 | 4h | 5 P0 commits + audit closure |
| S2 | 1 | 5h | StripBuilder TitleBar + RULE 6 in EGUI_LAYOUT.md |
| S3 | 1.5 | 6h | 33 booleans collapsed; bridge layer removed |
| S4 | 2A | 6h | RenderLine + markdown_to_lines + tests |
| S5 | 2B | 6h | Renderer + virtual scroll + j/k navigation |
| S6 | 2C | 6h | ChatArea + Sidebar + Workspace migration |
| S7 | 3A | 6h | TUI wired to ViewState + RenderLine; snapshot tests |
| S8 | 3B | 6h | 5 Claude-inspired affordances |
| S9 | 3C | 4h | Architecture docs + benchmarks + closure |
| **Total** | | **49h** | 9 sessions |

---

## 9. Open Questions

- [ ] `line_height`: single global token or per-role (Text vs Code)?
- [ ] Word-wrap within a `Text` line exceeding viewport: soft-wrap virtual
      sub-lines, or hard-wrap to multiple `Text` lines?
- [ ] Image rendering: inline `BlockSlot` or floating layer over lines?
- [ ] Search across lines: linear scan up to 10K lines is fine; beyond that?
- [ ] Artifacts panel: file-system-backed (persistent path) or in-memory only?

These resolve during Phase 2A as we build the actual data model.

---

## 10. Decision Log

| Date | Decision | Source |
|------|----------|--------|
| 2026-05-12 | Adopt `egui_extras::StripBuilder` over self-implementation | Kimi review + audit §F |
| 2026-05-12 | Phase 0.5 P0 blockers must merge before Phase 1 | Audit §G |
| 2026-05-12 | Phase 1.5 (state machine) is included, not skipped | User decision: engineering-dimension gains counted |
| 2026-05-12 | TUI and GUI share `clarity-core::ui::RenderLine` | Theory §2 Dimension 1 |
| 2026-05-12 | Keep `RenderBlock` as fallback during Phase 2 | Theory §5 (BlockSlot graceful degradation) |
| 2026-05-12 | Phase 3B borrows Claude affordances; egui keeps pixel decoration distinct | User direction + theory §4 |

---

## 11. References

- `docs/architecture/pretext-ui-theory.md` — strategic rationale (6-dim matrix)
- `docs/audits/2026-05-12-ui-design-audit.md` — Phase 0.5 audit
- `crates/clarity-egui/EGUI_LAYOUT.md` — layout rules including 5 traps
- `~/.config/agents/skills/egui-layout-canons/SKILL.md` — skill protocol
- This session commits: `f6f8b93c` → `4a4a6e1b`
- Kimi K2.6 conversation: `C:\Users\22414\Desktop\UI层次与全局渲染.md`
- egui_extras: https://docs.rs/egui_extras
- ratatui Layout (Cassowary inspiration): https://docs.rs/ratatui/latest/ratatui/layout/
