---
title: ADR-016: Pretext Single-Page / Three-Column Layout Migration
category: ADR
tags: [adr, ui, egui, pretext]
---

# ADR-016: Pretext Single-Page / Three-Column Layout Migration

- Status: Accepted (Phase A implemented)
- Deciders: juice094
- Date: 2026-06-13
- Related: ADR-011 (workspace architecture), ADR-014 (side-panel consolidation)

## Context

S5 completed the `clarity-egui` module hygiene pass: ViewState single-source-of-truth, `panels/` directory reorganisation, design system registration, and a `layout.rs` / `App::render_layout_shell()` staging point. The next architectural target is the Pretext UI evolution described in `docs/planning/plans/2026-05-12-pretext-ui-evolution.md` and the concrete migration plan in `docs/planning/plans/clarity-egui-pretext-layout-migration.md`.

The current GUI has evolved into a "titlebar + left sidebar + main view + floating right panels" model. The concept art for Pretext shows a single-page three-column layout:

```text
[ icon rail | expanded list ] [ main stage ] [ utility rail ]
```

This ADR records the decision to migrate the GUI shell to that layout incrementally, without deleting any existing panels during Phase A.

## Decision

### 1. Layout topology

The shell is redefined as three columns plus shared chrome:

- **Left rail**: always-visible icon rail. Tapping an icon expands an adjacent list panel (`Sessions`, `Workspace`, `Plugins`).
- **Main stage**: the mutually-exclusive central view (`Chat`, `Settings`, `Dashboard`, `Gantt`, `TaskBoard`, `Work`).
- **Right rail**: a collapsible utility rail showing one card at a time (`Status`, `Tools`, `Subagents`, `Memory`).
- **Shared chrome**: custom titlebar, bottom input bar, modals, toasts.

### 2. State representation

The rail state is stored in `clarity_core::ui::ViewState` so that GUI and TUI share the same semantics:

```rust
pub struct ViewState {
    // ... existing fields ...
    pub left_rail: LeftRailSection,
    pub left_rail_expanded: bool,
    pub right_rail: RightRailSection,
    pub right_rail_visible: bool,
}
```

`LeftRailSection` and `RightRailSection` are new typed enums in `clarity-core/src/ui/view_state.rs` and re-exported from `clarity_core::ui`.

Rationale:

- `clarity-core` is the canonical home for cross-frontend view semantics.
- Typed enums prevent the "50 boolean flag" anti-pattern that ADR-014 eliminated for legacy panels.
- Serialization defaults keep old persisted `ViewState` JSON forward-compatible.

### 3. Removal of `UiStore.sidebar_collapsed`

The legacy `sidebar_collapsed: bool` in `UiStore` is removed. Its meaning is subsumed by `view_state.left_rail_expanded`. All toggle sites (titlebar, chat header, keyboard shortcut, sidebar collapse button, responsive breakpoints) now mutate `view_state.left_rail_expanded`.

This enforces a single source of truth for left-rail visibility.

### 4. Responsive collapse policy

`layout.rs::update_and_measure()` applies one-way collapse rules as the window shrinks:

- `< breakpoint_wide`: hide the right utility rail.
- `< breakpoint_medium`: collapse legacy right panels and the left expanded list.
- `< breakpoint_compact`: keep only the icon rail and main stage.
- If content width would drop below `theme.content_min_width`, sacrifice right rail, then legacy panels, then the left expanded list.

The computed `LayoutMetrics` (`left_rail_w`, `left_panel_w`, `right_rail_w`, `content_w`) are exposed for future UI components that need geometry-aware behaviour.

### 5. Phase A scope (implemented)

- Add `LeftRailSection` / `RightRailSection` and `ViewState` rail fields.
- Remove `UiStore.sidebar_collapsed` and migrate callers.
- Rewrite `App::render_layout_shell()` to call `render_left_rail`, `render_main_stage`, `render_right_rail`.
- Implement an icon rail with session / workspace / plugin toggles.
- Add a right-rail scaffold with section toggles and placeholder cards.
- Preserve all existing panels: the old sidebar is still the expanded session list; workspace still renders file previews; modals remain unchanged.

### 6. Phase B/C scope (pending)

- Populate right-rail cards (`StatusCard`, `ToolsCard`, `SubagentCard`, `MemoryCard`) with real content migrated from legacy panels.
- Flatten the left sidebar into a native rail-expanded list (remove nested `SidePanel`).
- Evaluate `pretext-rust` integration for CJK line breaking and rich inline items.

## Consequences

- `clarity-core::ui::ViewState` gains new fields; any code constructing `ViewState` with struct-literal syntax outside the module will need updating. In practice only `clarity-egui` constructs `ViewState`, and it uses `ViewState::new()` plus field assignment.
- `UiStore` loses a field; this is a runtime-state change, not persisted settings, so no migration is needed.
- The GUI now renders a left icon rail by default. This is a visible change, but existing functionality is preserved.
- Future Pretext typography work has a clear insertion point: the main-stage message bubbles and right-rail cards.

## Compliance

Phase A passes:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo test --workspace --lib --exclude clarity-slint
cargo test -p clarity-egui --bin clarity-egui
```

All green. `clarity-core::ui::view_state` tests include round-trip and toggle coverage for the new rail enums.
