---
title: UI Design Audit — Phase 0.5
category: Audit
date: 2026-05-12
tags: [audit, ui]
---

# UI Design Audit — Phase 0.5

> **Date**: 2026-05-12
> **Scope**: `crates/clarity-egui` + `crates/clarity-core/src/ui`
> **Trigger**: Pre-Phase-1 design-level evaluation (must precede Grid for Chrome migration)
> **HEAD**: `fbacc5e3` (pushed to origin/main)
> **Owner**: juice094 + Clarity Agent

---

## Executive Summary

The TitleBar crisis is **functionally** closed (4/4 chrome buttons work, 5 traps codified,
canonical `Pattern A` shipped). Phase 0.5 reviews whether the **design surface** is ready
for Phase 1 (StripBuilder) without further regressions. Verdict:

| Axis | Verdict | Phase 1 Action |
|------|---------|----------------|
| F. Token system | 🟡 **Token gaps** — 12+ hardcoded values left | Block: tokenize before StripBuilder |
| B. Information arch | 🟠 **Compat layer present** — 33 legacy bool flags shadow ViewState | Defer to Phase 1.5 |
| C. Interaction | 🟡 **Shortcuts ≠ palette** — registries diverge by 6 commands | Block: unify before Phase 2 |
| D. Responsive | 🟢 **Functional** — 3/4 breakpoints used, `breakpoint_wide` dead code | Cleanup only |
| E. Accessibility | 🔴 **Focus blind spots** — 7 widgets bypass focus ring via raw allocate | Block: fix before Phase 2 |
| A. Visual | 🟢 **Trap-free TitleBar** — 2 residual `horizontal_centered` calls in non-chrome | Audit + canonize |

**Phase 1 Gate**: F + C + E must reach 🟢 before adopting `egui_extras::StripBuilder`.
Phase 1 plan (`2026-05-12-pretext-ui-evolution.md`) is amended in §6.

---

## A. Visual Code-Level Review (post-TitleBar fix)

Reviewed via static scan — no screenshots possible in CLI architecture.

### A.1 Anti-pattern density

```
ui.allocate_exact_size : 10 occurrences (10 files)
ui.put                  : 0 active uses (all in window_control.rs comments)
horizontal_centered     : 2 active uses (provider_row.rs:85, sidebar.rs:172)
ui.interact(rect,…)     : 0 (per RULE 3, confirmed)
Button::new("")         : 0 (ghost button extinct, per RULE 1)
```

### A.2 Verdict per occurrence

| File | Line | Pattern | Verdict |
|------|------|---------|---------|
| `widgets/window_control.rs` | 33 | `allocate_space` | ✅ Pattern A canonical |
| `widgets/sidebar_card.rs` | 23 | `allocate_exact_size + Sense::click` | ✅ Encapsulated widget (RULE 4 exception) |
| `widgets/provider_row.rs` | 49 | `allocate_exact_size + Sense::click` | ✅ Encapsulated widget |
| `widgets/theme_card.rs` | 49 | `allocate_exact_size + Sense::click` | ✅ Encapsulated widget |
| `widgets/status_capsule.rs` | 46 | `allocate_exact_size + Sense::click` | ✅ Encapsulated widget |
| `widgets/toggle.rs` | 11 | `allocate_exact_size + Sense::click` | ✅ Encapsulated widget |
| `main.rs:249` | drag filler | `allocate_exact_size + Sense::click_and_drag` | ✅ Pattern B canonical |
| `components/chat/avatar.rs` | (decorative) | `allocate_exact_size` | ✅ Pure spacer |
| `panels/sidebar.rs` | (multiple) | `allocate_exact_size` | ⚠️ Review needed for row widgets |
| `panels/workspace.rs` | (file tree) | `allocate_exact_size` | ⚠️ Review needed; Phase 2 will replace with line-rows |
| `panels/dashboard.rs` | (cards) | `allocate_exact_size` | ⚠️ Review needed |
| `components/settings/about_tab.rs` | (panel) | `allocate_exact_size` | ⚠️ Review needed |
| `provider_row.rs:85` | inner | `horizontal_centered` | ⚠️ Trap 4 risk — measuring pass double execution |
| `panels/sidebar.rs:172` | inner | `horizontal_centered` | ⚠️ Trap 4 risk |

**Action**: Three ⚠️ `horizontal_centered` callsites (none in chrome) need
single-pass verification before Phase 2 begins.

---

## B. Information Architecture

### B.1 ViewState present, but shadowed

`crates/clarity-core/src/ui/view_state.rs` defines the canonical state machine:

```
AppView    : Chat | Settings | Dashboard | Gantt | TaskBoard    (5 variants)
SidePanel  : Sidebar | Workspace | Team | Task                  (4 variants)
ModalType  : Approval | Snapshot | Login | TaskCreate | TeamCreate  (5 variants)
```

This is the **right** model. However `main.rs:802-810` contains a **compatibility
layer** that syncs ViewState back into 3 legacy booleans every frame:

```rust
self.settings_store.settings_open    = view_state.main == AppView::Settings;
self.ui_store.dashboard_panel_open   = view_state.main == AppView::Dashboard;
self.ui_store.gantt_panel_open       = view_state.main == AppView::Gantt;
```

This is **technically correct** but creates a **two-source-of-truth** risk: legacy
panel renderers read the booleans; the state machine writes them. If any panel
writes back to the boolean (e.g. close button setting `settings_open = false`),
the ViewState becomes stale until the next sync.

### B.2 Legacy boolean inventory (`crates/clarity-egui/src/stores/mod.rs`)

Counted **33 bool flags** representing UI state that should live in ViewState:

| Category | Count | Examples |
|----------|-------|----------|
| `*_panel_open` | 8 | `task_panel_open`, `team_panel_open`, `dashboard_panel_open`, `gantt_panel_open`, `skill_panel_open`, `mcp_panel_open`, `preview_drawer_open`, `kimi_code_login_open` |
| `*_modal_open` | 6 | `task_create_modal_open`, `task_view_modal_open`, `team_create_modal_open`, `cron_create_modal_open`, `subagent_view_modal_open`, `snapshot_modal_open` |
| `*_collapsed`/`*_expanded` | 9 | `sidebar_collapsed`, `cron_expanded`, `web_tabs_expanded`, `web_tabs_add_visible`, `thinking_log_expanded`, `thinking_log_show_all`, `tools_expanded`, `subagents_expanded`, `workspace_plan_expanded` |
| Workflow flags | 4 | `is_loading`, `compacting`, `stopping`, `stick_to_bottom` |
| Style flags | 3 | `agent_turn_style`, `agent_turn_glass`, `mcp_changed` |
| Misc | 3 | `settings_open`, `show_add_provider`, `focus_input_requested`, `workspace_plan_manually_collapsed`, `restoring`, `downloading_auto` |

### B.3 Findings

- ✅ ViewState **defined** and **shared** between GUI/TUI (correct architecture)
- 🟡 Migration **partial** — only 3 of 8 main views routed through `view_state.main`
- 🟠 33 boolean flags persist; for Phase 2 (Lines for Content) this becomes noise
      because line-rows need predictable parent state, not 33-dim bitmap
- 🔴 **No formal state transition table** — ad-hoc toggles in main.rs lines 635-686
      mean illegal states are reachable (e.g. modal open + sidebar collapsed
      + dashboard open simultaneously)

### B.4 Recommendation

Defer full migration to **Phase 1.5** (between Grid and Lines). Rationale:
breaking up 33 booleans now requires touching 8 panels — too much surface for
the current refactor budget. Phase 1 only needs ViewState for chrome routing,
which is already done.

**However**, Phase 1 SHOULD:
- [ ] Lock ViewState as **write-once-per-frame** at top of `update()`
- [ ] Mark the boolean sync layer with `// FIXME: legacy bridge, remove in Phase 1.5`
- [ ] Add a debug assert: `view_state` must match boolean snapshot at end of frame

---

## C. Interaction Audit (shortcuts ↔ command palette ↔ menus)

### C.1 Three registries currently diverge

#### Registry 1: `ShortcutAction` (`shortcuts/mod.rs`) — 9 variants

```
NewSession, StopGeneration, SendMessage, CloseModal,
ToggleSkillPanel, ToggleTeamPanel, FocusInput,
ToggleCommandPalette, ToggleDashboardPanel
```

#### Registry 2: `built_in` (`commands.rs`) — 8 commands

```
new-session, stop-generation, toggle-sidebar, focus-input,
open-settings, toggle-skill-panel, toggle-team-panel, toggle-dashboard
```

#### Registry 3: TitleBar buttons (`main.rs`) — 4 chrome buttons

```
sidebar-toggle, settings, dashboard-toggle, minimize/maximize/close
```

### C.2 Gap analysis

| Command | Shortcut | Palette | TitleBar |
|---------|----------|---------|----------|
| `new-session` | ✅ Ctrl+N | ✅ | ❌ |
| `stop-generation` | ✅ Ctrl+C | ✅ | ❌ |
| `send-message` | ✅ Ctrl+Enter | ❌ | ❌ |
| `close-modal` | ✅ Esc | ❌ | ❌ |
| `focus-input` | ✅ Ctrl+K | ✅ | ❌ |
| `toggle-command-palette` | ✅ Ctrl+Shift+P | ❌ | ❌ |
| `toggle-sidebar` | ❌ | ✅ Ctrl+B | ✅ |
| `toggle-skill-panel` | ✅ Ctrl+. | ✅ | ❌ |
| `toggle-team-panel` | ✅ Ctrl+Shift+T | ✅ | ❌ |
| `toggle-dashboard` | ✅ Ctrl+Shift+D | ✅ | ✅ |
| `open-settings` | ❌ (Esc closes only) | ✅ | ✅ |

**Gaps**: 6 mismatches between shortcuts and palette; TitleBar exposes only 2/11
commands as visible buttons.

### C.3 Findings

- 🟡 Three sources of truth (`ShortcutAction` enum, `built_in` fn, `render_titlebar` calls)
- 🔴 `CommandPalette::execute()` is a **stub** (`tracing::info!` only) — clicking a
      command does **nothing functional**. The palette is decoration.
- 🟢 Both registries share `CommandItem` type from `clarity-core::ui`
- 🟠 No `toggle-sidebar` shortcut despite TitleBar showing the affordance

### C.4 Recommendation (Phase 1 blocker)

Unify before Phase 2 keyboard navigation work:

- [ ] **P0.5.C.1** Replace `ShortcutAction` enum with `CommandId(String)` keyed off
      `built_in::all()` IDs; shortcuts become `(KeyCombo, CommandId)` pairs
- [ ] **P0.5.C.2** Wire `CommandPalette::execute()` to a real `CommandRouter` that
      mutates `App` (move out of stub)
- [ ] **P0.5.C.3** Add missing commands: `send-message`, `close-modal`,
      `toggle-command-palette`, `toggle-workspace`, `switch-view-chat/settings/…`
- [ ] **P0.5.C.4** Add `Ctrl+B` shortcut for `toggle-sidebar` (currently palette-only)

---

## D. Responsive Audit

### D.1 Breakpoint definitions (`theme.rs`)

```
breakpoint_compact :  768.0  (collapse sidebar, hide status labels)
breakpoint_medium  : 1100.0  (auto-close dashboard/team/task)
breakpoint_wide    : 1400.0  (currently unused — dead constant)
content_min_width  :  480.0  (min chat column width)
```

### D.2 Usage sites in `main.rs`

| Line | Constant | Behavior |
|------|----------|----------|
| 196  | `breakpoint_compact` | TitleBar status labels show/hide |
| 756  | `breakpoint_compact` | Sidebar auto-collapse on shrink |
| 751  | `breakpoint_medium`  | Auto-close dashboard/team/task panels |
| 787  | `content_min_width`  | Chat column squeeze guard |

### D.3 Findings

- ✅ Three of four breakpoints actively used
- 🟡 `breakpoint_wide` (1400) **defined but unused** — either implement or remove
- 🟢 Responsive guard triggers only on **shrink** events (`last_width >= bp &&
      current_width < bp`) — correct hysteresis to avoid flicker
- 🟠 No **expand-side** behavior — once user manually collapses sidebar at narrow,
      growing the window does not re-open it. Acceptable UX but undocumented.

### D.4 Recommendation

- [ ] **P0.5.D.1** Delete `breakpoint_wide` token or document its intended trigger
- [ ] **P0.5.D.2** Add comment in `theme.rs` explaining hysteresis (shrink-only)

---

## E. Accessibility Audit

### E.1 Focus ring coverage

`egui` focus ring requires the response carries the right `Sense` and is
registered through the layout engine. Findings:

| Widget | Sense source | Focus ring |
|--------|--------------|------------|
| `window_control_button` | `UiBuilder::sense(Sense::click)` | ✅ Inherited (Pattern A) |
| `interactive_row` | `UiBuilder::sense(Sense::click)` | ✅ |
| `toggle` | `allocate_exact_size(Sense::click)` | 🟡 Drawn manually via `Stroke::new(1.5, theme.focus_ring)` (toggle.rs:56) |
| `sidebar_card` | `allocate_exact_size(Sense::click)` | 🔴 No `has_focus()` check — focus invisible |
| `provider_row` | `allocate_exact_size(Sense::click)` | 🔴 No focus visual |
| `theme_card` | `allocate_exact_size(Sense::click)` | 🔴 No focus visual |
| `status_capsule` | `allocate_exact_size(Sense::click \| hover)` | 🔴 No focus visual |
| `tab_button` | `response.interact(Sense::click)` | 🟡 RULE 3 violation candidate; check focus |

### E.2 Tab order

`egui` derives Tab order from widget registration sequence. The TitleBar's
new horizontal layout means tabs flow: `sidebar-toggle → brand-label →
session-tabs → drag-filler → status-capsules → settings → minimize → maximize →
close`. **Drag-filler accepts focus but does nothing** — confusing.

### E.3 `on_hover_text` coverage

```
crates/clarity-egui/src/main.rs:361   conn_resp.on_hover_text("Agent connection status");
crates/clarity-egui/src/main.rs:378   .on_hover_text("Click to start/stop Gateway");
```

Only **2 hover tooltips** in the entire chrome. Window controls (close/minimize/
maximize/settings) lack tooltips entirely.

### E.4 Findings

- 🔴 **5 widgets bypass focus ring** — sidebar_card, provider_row, theme_card,
      status_capsule, tab_button
- 🟠 **Drag filler is keyboard-trap-light** — accepts Tab but offers no action
- 🟠 **TitleBar chrome buttons lack tooltips** — only 2/N have `on_hover_text`
- 🟢 Modal blocking is correct: `is_modal_open()` checks 6 conditions, returns
      early from shortcut collection

### E.5 Recommendation (Phase 2 blocker)

Phase 2's keyboard navigation cannot ship without these fixes:

- [ ] **P0.5.E.1** Add `response.has_focus()` → draw focus ring on 5 widgets above
- [ ] **P0.5.E.2** Make drag-filler `Sense::click_and_drag` only (no focus)
- [ ] **P0.5.E.3** Add `.on_hover_text()` to all TitleBar chrome buttons
- [ ] **P0.5.E.4** Add unit test: every widget in `widgets/` carries focus visual

---

## F. Token Audit

### F.1 Tokens correctly used

```
theme.size_titlebar       :  6 uses
theme.size_panel_right    :  3 uses
theme.size_sidebar        :  1 use
theme.breakpoint_compact  :  3 uses
theme.breakpoint_medium   :  1 use
theme.content_min_width   :  1 use
theme.space_*             : 26 uses
theme.text_*              : 14 uses
theme.radius_*            : 12 uses
```

### F.2 Hardcoded values remaining in chrome

| Location | Value | Reason | Action |
|----------|-------|--------|--------|
| `main.rs:201` | `450.0` / `280.0` (estimated_right_w) | Reserve for RTL right zone | Phase 1: StripBuilder replaces this |
| `main.rs:228` | `40.0` (center_w min) | Drag-filler floor | Phase 1: token `theme.min_drag_w` |
| `main.rs:247` | `20.0` (drag_w min) | Drag-filler floor | Phase 1: token `theme.min_drag_w` |
| `main.rs:438` | `10.0` (edge) | Resize-zone padding | Token `theme.window_edge_zone` |
| `main.rs:?` | `36.0` (sidebar collapsed) | Collapsed width | Token `theme.size_sidebar_collapsed` |
| `main.rs:init` | `900.0/700.0/600.0` | Window initial size | Token `theme.window_default_w/h/min_w` |
| `command_palette.rs:53` | `40.0` (offset_y) | Center-top distance | Token `theme.modal_offset_y` |
| `command_palette.rs:62` | `520.0` (palette width) | Modal width | Token `theme.palette_w` |
| `command_palette.rs:79` | `320.0` (max_height) | Scroll height | Token `theme.palette_max_h` |
| `tab_button.rs` | `28.0` / `18.0` | Tab height + close | Token `theme.size_tab_h` |
| `theme_card.rs` | `200.0 × 64.0` | Card dimensions | Token `theme.size_theme_card_*` |
| `sidebar_card.rs` | `56.0` | Card height | Token `theme.size_sidebar_card_h` |
| `main.rs:216` | `8.0` (sidebar gap) | Inter-widget gap | Already `theme.space_8`; convert |
| `main.rs:240,245,249` | `8.0` (multiple) | Inter-widget gaps | Convert to `theme.space_8` |

### F.3 Findings

- 🟡 **12+ chrome hardcoded values** remain — most are sizing/positioning
- 🟢 Token system **structurally correct** — 4 dimension + 4 breakpoint tokens
      already present in theme.rs
- 🟠 Some values (`estimated_right_w 450/280`) are **trap-relics** that
      Phase 1 will delete entirely, not tokenize

### F.4 Recommendation (Phase 1 blocker)

Pre-Phase-1 token expansion:

- [ ] **P0.5.F.1** Add 8 tokens to `theme.rs`:
      `window_default_w/h`, `window_min_w`, `window_edge_zone`,
      `size_sidebar_collapsed`, `size_tab_h`, `modal_offset_y`,
      `palette_w`, `palette_max_h`
- [ ] **P0.5.F.2** Convert remaining `8.0` literals to `theme.space_8`
- [ ] **P0.5.F.3** Delete `estimated_right_w` constant entirely in Phase 1
      (replaced by `StripBuilder::size(Size::exact(...))` declarations)

---

## G. Priority Matrix

### G.1 P0 Blockers (must fix before Phase 1 starts)

| ID | Item | Effort | Section |
|----|------|--------|---------|
| **P0.5.F.1** | Add 8 chrome dimension tokens | 30min | F |
| **P0.5.C.1** | Unify `ShortcutAction` ↔ `CommandItem` via shared `CommandId` | 1h | C |
| **P0.5.C.2** | Wire `CommandPalette::execute()` to real `CommandRouter` | 1h | C |
| **P0.5.E.1** | Focus ring on 5 widgets | 1h | E |
| **P0.5.E.3** | `.on_hover_text()` on TitleBar buttons | 15min | E |

**Total**: ~4h, one focused session before Phase 1.1 (`egui_extras` adoption).

### G.2 P1 Should-fix (during Phase 1)

| ID | Item | Section |
|----|------|---------|
| P0.5.B.1 | Mark legacy boolean sync as `// FIXME: Phase 1.5` | B |
| P0.5.C.3 | Add missing commands to palette | C |
| P0.5.C.4 | `Ctrl+B` shortcut for sidebar toggle | C |
| P0.5.D.1 | Resolve `breakpoint_wide` (use or delete) | D |
| P0.5.D.2 | Document hysteresis comment | D |
| P0.5.E.2 | Make drag filler non-focusable | E |
| P0.5.F.2 | Convert literal `8.0` to `theme.space_8` | F |
| Trap-4 audit | Verify `provider_row.rs:85` and `sidebar.rs:172` | A |

### G.3 P2 Phase 2 dependencies

| ID | Item | Phase |
|----|------|-------|
| P0.5.B.2 | Migrate 33 booleans to ViewState | 1.5 |
| P0.5.E.4 | Widget focus unit tests | 2 |
| Sidebar/Workspace `allocate_exact_size` review | Phase 2 line-rows | 2 |

---

## 6. Phase 1 Plan Amendment

Original Phase 1 (`2026-05-12-pretext-ui-evolution.md` §2.3) defined 7 tasks.
This audit adds **5 P0.5 prerequisites** before P1.1:

```
P0.5.F.1  Add 8 chrome dimension tokens
P0.5.C.1  Unify ShortcutAction ↔ CommandItem
P0.5.C.2  Wire CommandPalette::execute()
P0.5.E.1  Focus ring on 5 widgets
P0.5.E.3  .on_hover_text() on TitleBar buttons
─────────  ← Gate: Phase 1 starts here
P1.1      Add egui_extras = "0.31"
P1.2      Refactor render_titlebar with StripBuilder
P1.3      Remove estimated_right_w
…
```

### 6.1 Why these are blockers

- **F.1 (tokens)**: StripBuilder declarations like `Size::exact(theme.size_*)`
  require the token to exist — adding tokens after StripBuilder migration would
  mean re-touching the same code twice.
- **C.1 / C.2 (command unification)**: Phase 2 keyboard navigation (j/k/g/G/Enter)
  routes through `CommandRouter`. If the router is a stub when Phase 2 starts,
  every navigation key becomes a special case.
- **E.1 (focus ring)**: Phase 2 introduces `j`/`k` row navigation. Without
  visible focus, users cannot tell which line is selected.
- **E.3 (tooltips)**: Cheap win, prevents users from rediscovering the icons
  every session.

### 6.2 Revised effort estimate

```
Phase 0.5  : 4h (this audit + 5 fixes)        ← 1 session
Phase 1    : 5h (StripBuilder migration)      ← 1 session
Phase 1.5  : 6h (ViewState full migration)    ← 1 session
Phase 2    : 18h (Lines for Content)          ← 2-3 sessions
Phase 3    : 12h (Unification)                ← 1-2 sessions
─────────
Total      : ~45h, 6-8 focused sessions
```

Original estimate was 35h; the +10h is the Phase 0.5 + Phase 1.5 cost, which
saves at least 15h of regression debugging based on this session's data point
(4 consecutive fixes on a single button).

---

## 7. Verdict

Phase 0.5 audit **PASSES** with **5 P0 blockers** identified. Recommended path:

1. **Now**: commit this audit as `docs(audits/...)` to lock the findings.
2. **Next session (4h)**: execute the 5 P0 fixes as **one atomic commit per
   section** (F → C → E → audit re-test).
3. **Following session (5h)**: Phase 1 StripBuilder migration.
4. **Defer**: Phase 1.5 boolean-to-ViewState migration after Phase 1 stabilizes.

The skill `egui-layout-canons` already captures the failure modes; the audit
ensures Phase 1 enters with a cleaner baseline so we don't burn cycles on
preventable rework.

---

## References

- `docs/plans/2026-05-12-pretext-ui-evolution.md` — Phase plan (amended in §6 above)
- `crates/clarity-egui/EGUI_LAYOUT.md` — Layout rules (Appendix: 5 traps)
- `~/.config/agents/skills/egui-layout-canons/SKILL.md` — Skill protocol
- `crates/clarity-core/src/ui/view_state.rs` — Canonical state machine
- `crates/clarity-egui/src/shortcuts/mod.rs` — Shortcut registry
- `crates/clarity-egui/src/widgets/command_palette.rs` — Command palette stub

