# Pretext UI Evolution Plan — Grid for Chrome, Lines for Content

> **Date**: 2026-05-12
> **Status**: Phase 0 ✅ completed, Phase 0.5 audit ✅, P0 blockers ⏳, Phase 1-3 planned
> **Owner**: juice094 + Clarity Agent
> **Skill**: `~/.config/agents/skills/egui-layout-canons/SKILL.md`
> **Audit**: `docs/audits/2026-05-12-ui-design-audit.md` (5 P0 blockers, 4h)

---

## 0. Background & Motivation

The TitleBar regression session (2026-05-11 → 2026-05-12) required **four
consecutive fixes** on a single 36×36 chrome button. Five distinct death traps
were discovered, all rooted in the same source: **command-mode cursor flow has
no systematic invariants for composite chrome widgets**.

This plan formalizes a two-pronged evolution:

1. **Grid for Chrome** — Replace imperative `ui.horizontal_centered` /
   `estimated_right_w` / `ui.put` patterns in TitleBar / StatusBar / ToolBar
   with declarative constraint grids (TUI mindset on egui).
2. **Lines for Content** — Replace pixel-continuous markdown streams with
   semantic discrete line sequences (Pretext mindset extended to streaming).

The unifying insight: **GUI is a TUI with pixel decoration**. Both axes derive
from the same information architecture; only the renderer differs.

---

## 1. Phase 0 — TitleBar Crisis Closure ✅

**Commits** (this session):
- `f6f8b93c` feat(core+egui): cross-platform UI state machine + CommandPalette
- `8ebe2947` feat(theme): layout & breakpoint tokens
- `14760ff6` fix(egui-titlebar): drop horizontal_centered + Pattern A
- `2a25a76f` refactor(egui-widgets): interactive_row → UiBuilder::sense()
- `db9195c5` docs(egui-layout): Production-Verified Traps appendix

**External**:
- `~/.config/agents/skills/egui-layout-canons/SKILL.md` created
- `~/AGENTS.md` updated with intent routing for the skill

**Deliverables**:
- All 6 user-reported TitleBar functions verified working
- Window controls precisely centered (Frame::inner_margin)
- ViewState state machine eliminates boolean flag hell
- CommandPalette accessible via Ctrl+Shift+P

**Lessons codified**: 5 death traps, 3 canonical patterns, file-based
diagnostic protocol, PR checklist.

---

## 1.5. Phase 0.5 — Design Audit Gate ✅ (audit) / ⏳ (fixes)

> **Output**: `docs/audits/2026-05-12-ui-design-audit.md`
> **Verdict**: PASSES with 5 P0 blockers identified
> **Effort**: 4h for blockers, 1 session

Five prerequisites must land **before** Phase 1.1 (`egui_extras` adoption):

| ID | Item | Section | Effort |
|----|------|---------|--------|
| P0.5.F.1 | Add 8 chrome dimension tokens to `theme.rs` | F | 30min |
| P0.5.C.1 | Unify `ShortcutAction` ↔ `CommandItem` via shared `CommandId` | C | 1h |
| P0.5.C.2 | Wire `CommandPalette::execute()` to real `CommandRouter` | C | 1h |
| P0.5.E.1 | Add focus ring to 5 widgets that bypass it | E | 1h |
| P0.5.E.3 | `.on_hover_text()` on every TitleBar chrome button | E | 15min |

**Rationale**: tokens are required for `Size::exact(theme.*)` declarations in
StripBuilder; the command router becomes the routing target for Phase 2 keyboard
navigation; focus ring is a Phase 2 prerequisite for `j/k` row selection.

See audit §G for priority matrix and §6 for revised effort estimate (45h total).

---

## 2. Phase 1 — Grid for Chrome (StripBuilder)

> **Goal**: Eliminate every `estimated_*_w` hardcoded value in chrome regions
> by adopting declarative constraint-driven layout.
> **Gate**: Phase 0.5 P0 blockers (5 items) must all merge before P1.1.

### 2.1 Scope

| Region | File | Current | Target |
|--------|------|---------|--------|
| TitleBar | `main.rs::render_titlebar` | `ui.horizontal` + `estimated_right_w` (450/280) | `StripBuilder` LTR with `Size::exact`/`Size::remainder` |
| StatusBar | `main.rs::render_status_bar` (TBD) | TBD | StripBuilder with progress + indicators slots |
| ToolBar | `panels/chat/header.rs::render_header` | `ui.horizontal` with manual spacing | StripBuilder |

### 2.2 Dependency Decision

**Option A**: Add `egui_extras = "0.31"` (official ~80KB)
- Pros: official, maintained, well-tested
- Cons: external dependency

**Option B**: Self-implement `widgets/grid.rs` (~50 LOC)
- Pros: zero dependency, full control
- Cons: maintenance burden, edge cases

**Recommendation**: **Option A**. The cost-benefit clearly favors official
package given the trap-resolution value.

### 2.3 Tasks

- [ ] **P1.1** Add `egui_extras = "0.31"` to `crates/clarity-egui/Cargo.toml`
- [ ] **P1.2** Refactor `render_titlebar` to use `StripBuilder`
- [ ] **P1.3** Remove `estimated_right_w` heuristic; sizes now declarative
- [ ] **P1.4** Refactor `render_status_bar` (if exists) to StripBuilder
- [ ] **P1.5** Add `RULE 6` to `EGUI_LAYOUT.md`: chrome regions must use
      StripBuilder
- [ ] **P1.6** Update `egui-layout-canons` SKILL.md with "When to Use Grid vs
      Cursor" decision tree
- [ ] **P1.7** Write unit tests for grid solver (if Option B chosen)

### 2.4 Acceptance Criteria

- TitleBar uses zero `estimated_*` constants
- TitleBar passes all 6 functional tests on three window widths (600/900/1400)
- No `ui.horizontal_centered` remains in chrome rendering
- StatusBar (when implemented) follows same pattern
- `cargo build` produces no new warnings

### 2.5 Risk & Rollback

- **Risk**: `StripBuilder` API may differ from our assumptions
- **Mitigation**: read `egui_extras` source first (PoC TitleBar in scratch
  before committing)
- **Rollback**: keep current `ui.horizontal + estimated_right_w` as fallback
  for one release cycle, gate StripBuilder behind a feature flag if needed

### 2.6 Estimated Effort

- Investigation + PoC: 2h
- TitleBar migration: 1h
- StatusBar migration: 1h
- Tests + docs: 1h
- **Total**: ~5h, can be one focused session

---

## 3. Phase 2 — Lines for Content (Pretext Streaming)

> **Goal**: Replace `Message::parsed: Vec<RenderBlock>` with
> `Message::lines: Vec<RenderLine>`, giving exact-pixel virtual scrolling and
> first-class keyboard navigation.

### 3.1 Scope

| Region | File | Current | Target |
|--------|------|---------|--------|
| ChatArea | `panels/chat/messages.rs` | `RenderBlock` + `estimate_height()` | `Vec<RenderLine>` + exact pixel scrolling |
| Sidebar (session list) | `panels/sidebar.rs::render_sessions` | `clickable_row` | line-rows with j/k navigation |
| Workspace (file tree) | `ui/file_browser.rs` | recursive rows | flat line list with indent |
| MCP/Skill panels | various | mixed | line-rows |

### 3.2 Data Model

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

### 3.3 Tasks

- [ ] **P2.1** Define `RenderLine` enum in `clarity-core/src/ui/render_line.rs`
- [ ] **P2.2** Write `markdown_to_lines(md: &str) -> Vec<RenderLine>` converter
      using existing `pulldown-cmark`
- [ ] **P2.3** Add `Message::lines: Vec<RenderLine>` field, populated in
      `prepare()` alongside (initially) `parsed: Vec<RenderBlock>`
- [ ] **P2.4** Write `render_lines(ui, &[RenderLine], theme)` in
      `clarity-egui/src/ui/line_renderer.rs`
- [ ] **P2.5** Add feature flag `line-mode` to toggle ChatArea rendering
- [ ] **P2.6** Implement exact-pixel virtual scrolling: `scroll_offset / line_height`
- [ ] **P2.7** Implement keyboard navigation: `j`/`k`/`g`/`G`/`Enter`/`/`
- [ ] **P2.8** Implement streaming append: per-line buffer flush on `\n`
- [ ] **P2.9** Migrate Sidebar, Workspace to line-rows
- [ ] **P2.10** TUI: implement `render_lines_tui` in `clarity-tui` sharing
      `RenderLine` enum; ANSI rendering instead of pixel

### 3.4 Acceptance Criteria

- ChatArea scrolling is pixel-perfect (no jitter, no jumps)
- Virtual list handles 10K messages without lag
- `j`/`k` navigation works in ChatArea, Sidebar, Workspace
- Streaming text appends without re-parsing full message
- TUI and GUI share the same `RenderLine` source
- Existing markdown tests still pass

### 3.5 Risk & Rollback

- **Risk**: Tables, images, complex nested lists may degrade
- **Mitigation**: `BlockSlot` provides fallback to existing block renderers
  for non-line content
- **Risk**: Major refactor surface area
- **Mitigation**: Feature-flag the transition; old `RenderBlock` path remains
  available for 2 release cycles

### 3.6 Estimated Effort

- Data model + converter: 4h
- Renderer: 4h
- Virtual scroll + keyboard: 3h
- Streaming integration: 2h
- TUI alignment: 2h
- Tests + migration: 3h
- **Total**: ~18h, 2-3 focused sessions

---

## 4. Phase 3 — Full Unification

> **Goal**: Chrome regions use grids, content regions use lines, both share
> the same information architecture. GUI and TUI are two skins of one core.

### 4.1 Tasks

- [ ] **P3.1** Audit every Panel for grid vs cursor classification, document
      in `docs/architecture/ui-axis.md`
- [ ] **P3.2** Ensure TUI `clarity-tui` mirrors every GUI panel via the same
      `ViewState` + `RenderLine` core
- [ ] **P3.3** Performance benchmark: GUI 60fps with 10K messages, TUI same
      data without lag
- [ ] **P3.4** Cross-platform regression suite: snapshot tests for both
      renderers reading the same `clarity-core` state
- [ ] **P3.5** Document the unified theory in `docs/architecture/pretext-ui.md`

### 4.2 Estimated Effort

- ~12h, 1-2 sessions after Phase 2 stabilizes

---

## 5. Cross-Phase Concerns

### 5.1 Backward Compatibility

- Phase 1: TitleBar visual unchanged for users
- Phase 2: feature flag `line-mode` allows opt-in during transition
- Phase 3: deprecation path for `RenderBlock` over 2 releases

### 5.2 Documentation Updates

Each phase requires:
- `egui-layout-canons/SKILL.md` chapter additions
- `EGUI_LAYOUT.md` rule extensions
- `docs/architecture/` design notes
- `CHANGELOG.md` user-facing entries

### 5.3 Performance Targets

- TitleBar repaint: < 100µs per frame
- ChatArea 60fps with 10K lines, 1MB markdown
- Keyboard navigation: < 16ms response

---

## 6. Decision Log

| Date | Decision | Rationale |
|------|---------|-----------|
| 2026-05-12 | Adopt `egui_extras::StripBuilder` over self-implementation | Official, 80KB acceptable, eliminates maintenance |
| 2026-05-12 | Keep `RenderBlock` as fallback during Phase 2 | Tables/images need block-level rendering |
| 2026-05-12 | TUI and GUI share `clarity-core::ui::RenderLine` | Single source of truth for line semantics |

---

## 7. Open Questions

- [ ] Should `line_height` be a single global token or per-role (Text vs Code)?
- [ ] How to handle word-wrap within a `Text` line that exceeds viewport width?
      (Soft-wrap virtual sub-lines? Hard-wrap to multiple `Text` lines?)
- [ ] Image/media rendering: inline `BlockSlot` or floating layer?
- [ ] Search across lines: linear scan acceptable up to 10K lines, then?

---

## 8. References

- This session's commits: `f6f8b93c` → `db9195c5`
- Skill: `~/.config/agents/skills/egui-layout-canons/SKILL.md`
- Layout rules: `crates/clarity-egui/EGUI_LAYOUT.md`
- Earlier Pretext analysis: `docs/references/pretext-deep-analysis.md`
- egui_extras: https://docs.rs/egui_extras
- ratatui Layout (inspiration): https://docs.rs/ratatui/latest/ratatui/layout/
