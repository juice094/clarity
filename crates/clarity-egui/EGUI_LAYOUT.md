# clarity-egui Layout Specification

> **Version:** 1.1
> **Scope:** All `.rs` files in `crates/clarity-egui/src`
> **Status:** Mandatory — P0 compliance required for all PRs
> **Changelog v1.1 (2026-05-12 / S2.P1.5+P1.7):** added RULE 6 (chrome uses StripBuilder) and RULE 7 (icons are glyphs).

This document exists because the codebase previously mixed three incompatible layout systems (responsive egui, `allocate_exact_size`, and raw `painter`), producing 47+ hardcoded coordinate offsets, broken theme propagation, and unmaintainable UI code. These rules are non-negotiable.

---

## 1. The 7 Iron Rules

### RULE 1: Ghost Button Prohibition

**Forbidden:** Creating a `Button::new("")` (or any transparent Button) solely to obtain a `Rect`, then using `painter` to draw all visual content and interaction feedback on top of it.

**Rationale:** The Button provides hit-testing, but every visual property (fill, stroke, text, hover state, focus ring) is reimplemented manually with hardcoded coordinates. This bypasses egui's layout engine entirely.

**Exception:** `theme.ghost_button()` is legal **only** when the button carries its own `WidgetText` and no `painter` overlay follows it.

---

### RULE 2: Painter Usage Boundaries — Decoration Only

**Allowed:**
- Divider lines (`painter.line_segment`)
- Decorative shapes (avatars, charts, game HUD)
- Custom cursors or drag previews
- Canvas annotations

**Forbidden in UI panels:**
- `painter.text()` for labels, headings, or button captions
- `painter.rect_filled()` for widget backgrounds, hover states, or focus rings
- `painter.circle_filled()` for status dots inside layout-driven components

**Rationale:** `painter` does not advance `ui.cursor()`, does not participate in clip rects, and ignores theme spacing. All UI text must use `ui.label()`, `ui.heading()`, or `RichText`. All UI backgrounds must use `Frame::fill()` or `Button::fill()`.

---

### RULE 3: Raw Rect + `ui.interact` Prohibition

**Forbidden:** Manually constructing a `Rect` from `ui.cursor().min` + hardcoded size, then calling `ui.interact(rect, id, Sense::click())` to drive behavior.

**Rationale:** This discards egui's built-in focus ring, keyboard navigation, `on_hover_text`, and state management (hover/pressed/active).

**Use instead:**
- `ui.button()` / `ui.selectable_value()` / `ui.checkbox()`
- `SelectableLabel` for list rows
- `Frame::show(ui, |ui| { ... }).response` for custom containers

---

### RULE 4: `allocate_exact_size` Boundaries — Spacers Only

**Allowed:**
- Fixed-size spacers and separators
- Drag handles with known geometry
- Decorative graphics (e.g., chart canvases)
- Custom widgets whose *entire* implementation lives in `widgets/` and has dedicated tests

**Forbidden:**
- Tab buttons
- Text containers
- Interactive rows or cards
- Any widget that contains text or child widgets
- **Never** use `allocate_exact_size` for a widget that is immediately followed by `ui.put`
  of an interactive `Button`/`Label` on the same rect — this creates overlapping interact
  regions and causes the outer Response to become permanently blind to input.

**Rationale:** `allocate_exact_size` reserves space but does not lay out children. If you need to place text or nested widgets inside the reserved area, use `Frame` + inner layout or a built-in widget.

---

### RULE 5: All Layout Constants Through Theme System

**Forbidden:** Hardcoded pixel values `> 8.0` for spacing, sizing, or positioning.

**Required:**
| Category | Token Source |
|----------|-------------|
| Spacing / padding | `theme.space_*` |
| Corner radius | `theme.radius_*` |
| Font size | `theme.text_*` |
| Colors | `theme.bg_*`, `theme.text_*`, `theme.accent_*` |
| Shadows | `theme.shadow_*` |
| Animation duration | `theme.duration_*` |

**Rationale:** If a value is not in `theme.rs`, the theme system is broken. Adding new hardcoded coordinates guarantees theme changes will produce misaligned UI.

---

### RULE 6: Chrome Must Use `StripBuilder` (no `estimated_*` heuristics)

**Forbidden in chrome (titlebar, statusbar, modal frames, sidebar headers):**
- Computing `let estimated_*: f32 = if cond { ... } else { ... }` to reserve space for a sibling zone.
- Calling `ui.available_width() - <magic number>` to size a center zone.
- `ui.allocate_ui_with_layout(vec2(computed_w, h), ...)` followed by `ui.with_layout(RTL, ...)` siblings.

**Required for chrome:**
```rust
use egui_extras::{Size, StripBuilder};

StripBuilder::new(ui)
    .size(Size::exact(theme.titlebar_left_w))            // LEFT
    .size(Size::remainder().at_least(40.0))              // CENTER
    .size(Size::exact(theme.titlebar_right_w_full))      // RIGHT
    .horizontal(|mut strip| {
        strip.cell(|ui| { /* left content */ });
        strip.cell(|ui| { /* center content (tabs + drag filler) */ });
        strip.cell(|ui| { /* right content (RTL layout inside) */ });
    });
```

**Rationale:** Chrome regions are predictable in structure (LEFT exact / CENTER fill / RIGHT exact). Computing zone widths arithmetically from `available_width()` minus heuristics produces traps like `estimated_right_w: 450/280` (S1 audit blocker P0.5.E.4). `StripBuilder` makes the layout declarative, the widths come from theme tokens, and the CENTER zone naturally adapts to window resize.

**Scope:** Applies to TopBottomPanel chrome (`render_titlebar`, future `render_status_bar`), modal frame layouts, sidebar headers. Does **not** apply to content panels (chat history, settings forms) — those use idiomatic `ui.vertical / ui.horizontal` because their structure is content-driven, not declared.

**Token tie-in:** Chrome dimension tokens live in `Theme` (e.g., `titlebar_left_w`, `titlebar_right_w_full`, `titlebar_right_w_compact`, `palette_w`, `palette_max_h`). New chrome regions must add tokens; inline magic numbers are forbidden per RULE 5.

**See also:**
- Canonical implementation: `crates/clarity-egui/src/main.rs::render_titlebar` (post S2.P1.2 refactor).
- PoC reference: `crates/clarity-egui/examples/strip_titlebar.rs`.
- Audit context: `docs/audits/2026-05-12-ui-design-audit.md` §G (P0.5.E.4 estimated_right_w trap).

---

### RULE 7: Icons Are Glyphs — Not Bitmaps, Not SVG Meshes

**Required:** Every icon used in the UI is a Unicode codepoint rendered through the standard font pipeline (`egui::FontFamily::Name("icons")`), identical to text rendering — same `FontId`, same kerning, same `text_color()`, same baseline alignment, same focus-ring story.

**Allowed icon sources** (in priority order):
1. **`lucide_icons::Icon::*`** — type-safe enum (1706 glyphs), preferred for new code (ADR-010).
2. **`crate::theme::ICON_*`** — `&'static str` constants (backward-compatible API surface for the 27 existing call sites; codepoints are the underlying `Icon::*.unicode()` values).
3. **Plain Unicode chars** (e.g., `'\u{2630}'` for hamburger menu) — only for cases not covered by Lucide.

**Forbidden:**
- SVG rasterization at runtime (`resvg` + `egui::ColorImage`) for icon use cases. Reason: bypasses font cache, doubles GPU texture binds, breaks `text_color()` inheritance.
- Per-icon mesh tessellation (`epaint::Mesh` with hand-authored vertices). Reason: violates the icons-are-glyphs principle and produces brittle position math.
- Embedding multiple icon fonts for visual variety. Reason: every additional font is a 500-800 KB binary tax and a parallel codepoint namespace.
- Inline hex codepoints in widget code (`ui.label("\u{e154}")`). Reason: hides intent at the call site; use `theme::ICON_SETTINGS` or `Icon::Settings.unicode()`.

**Rationale:** The Pretext UI thesis (see `docs/architecture/pretext-ui-theory.md`) treats icons and text characters as co-equal `inline glyph` boxes — both occupy an `advance_width`, both inherit `text_color`, both participate in `RichText` layout. Treating an icon as anything other than a glyph re-introduces a parallel layout system, which RULE 1-5 already forbid for text.

**TUI fallback contract (Phase 3 / S7):** Every Lucide codepoint used in chrome or content must have a registered Unicode fallback for ratatui rendering (`IconFallbackTable: IconId -> char`). Lucide's Private Use Area codepoints (`\u{e000}` ~ `\u{f8ff}`) are not portable across terminals; the fallback uses standard Unicode (e.g., `IconId::Settings -> '⚙'` U+2699).

**Sub-pixel quality note:** Lucide's 1.5 px stroke at 12-14 px font sizes (`text_xs` / `text_sm`) may render with anti-aliased gray pixels. If observed in production, mitigate via `egui::pixels_per_point` upscaling rather than switching off the font pipeline.

**See also:**
- ADR-010: Lucide adoption decision and codepoint mapping table.
- `crates/clarity-egui/src/theme.rs`: 27 `ICON_*` constants with Lucide-variant comments.
- `crates/clarity-egui/src/theme.rs::Theme::font_icon`: helper returning `FontId` for icon font.

---

## 2. Decision Tree

```text
Does the element need user interaction (click, hover, focus)?
├─ YES → Does it need to stay selected/highlighted?
│   ├─ YES → SelectableLabel (list rows, nav items, tabs)
│   └─ NO  → Button (actions, toolbars, confirmations)
│
├─ NO → Does it contain nested child widgets or text?
│   ├─ YES → Frame + inner layout (cards, panels, modals)
│   └─ NO  → Is it purely decorative?
│       ├─ YES → painter (lines, dots, shapes, charts)
│       └─ NO  → allocate_exact_size (spacer, drag handle)
```

### Quick Reference

| Goal | Correct Widget |
|------|----------------|
| Clickable action | `Button` |
| Toggle / select row | `SelectableLabel` |
| Container with padding/background | `Frame` |
| Pure decoration | `painter` |
| Fixed-size gap | `ui.allocate_exact_size` (non-interactive) |
| Icon + label combo | `ui.horizontal` with `ui.label(RichText::new(icon).font(...))` |

---

## 3. Anti-pattern vs Correct Pattern

### RULE 1 — Ghost Button

**Anti-pattern:**
```rust
let btn = ui.add(
    Button::new("")
        .fill(TRANSPARENT)
        .min_size(vec2(ui.available_width(), 56.0)),
);
if btn.hovered() {
    ui.painter().rect_filled(btn.rect, theme.radius_md, theme.bg_hover);
}
// 7+ hardcoded coordinate offsets follow...
```

**Correct:**
```rust
ui.horizontal(|ui| {
    ui.add_space(theme.space_12);
    ui.label(RichText::new(icon).font(theme.font_icon(theme.text_base)));
    ui.add_space(theme.space_8);
    ui.vertical(|ui| {
        ui.label(RichText::new(label).strong().size(theme.text_base));
        ui.horizontal(|ui| {
            ui.colored_label(theme.status_online, "●");
            ui.label(RichText::new(subtitle).size(theme.text_xs));
        });
    });
})
.interact(Sense::click())
```

### RULE 2 — Painter in UI

**Anti-pattern:**
```rust
ui.painter().text(
    pos2(rect.min.x + 12.0, rect.min.y + 10.0),
    Align2::LEFT_TOP,
    "Label",
    theme.font(theme.text_base),
    theme.text,
);
```

**Correct:**
```rust
ui.label(RichText::new("Label").size(theme.text_base).color(theme.text));
```

### RULE 3 — Raw Rect + `ui.interact`

**Anti-pattern:**
```rust
let row_rect = Rect::from_min_size(ui.cursor().min, vec2(avail_w, 28.0));
let resp = ui.interact(row_rect, id, Sense::click());
if resp.hovered() {
    ui.painter().rect_filled(row_rect, theme.radius_sm, theme.bg_hover);
}
// manual text painting follows...
```

**Correct:**
```rust
let response = ui.add(
    SelectableLabel::new(is_selected, label)
        .text_style(theme.font(theme.text_sm)),
);
if response.clicked() { is_selected = !is_selected; }
```

### RULE 4 — `allocate_exact_size` for Interactive Widgets

**Anti-pattern:**
```rust
let (tab_rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
// manually draw tab background, text, close button, truncation...
```

**Correct:**
```rust
let tab = ui.add(
    Button::new(&title)
        .min_size(vec2(tab_width, 28.0))
        .fill(if is_active { theme.bg_active } else { TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(theme.radius_sm as u8)),
);
```

Or with `Frame` for composite tabs:
```rust
let response = Frame::new()
    .fill(if is_active { theme.bg_active } else { TRANSPARENT })
    .corner_radius(CornerRadius::same(theme.radius_sm as u8))
    .inner_margin(Margin::symmetric(8, 4))
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&title).color(text_color));
            if ui.small_button("×").clicked() { /* close */ }
        });
    })
    .response;
```

### RULE 5 — Hardcoded Coordinates

**Anti-pattern:**
```rust
let content_left = btn_resp.rect.min.x + 12.0;
let line_y = btn_resp.rect.min.y + 10.0;
let dot_y = line_y + theme.text_base + 4.0;
```

**Correct:**
```rust
ui.add_space(theme.space_12); // replaces +12.0
ui.vertical_centered(|ui| {   // replaces manual vertical centering
    // children auto-align
});
```

---

## 4. Theme Token Reference

All layout constants must resolve to one of these tokens. If a design requires a value not listed here, add it to `theme.rs` first.

### Spacing (8 px baseline grid)
| Token | Value | Usage |
|-------|-------|-------|
| `theme.space_4` | 4.0 | Tight internal padding, icon gaps |
| `theme.space_8` | 8.0 | Default widget gap, inner margin |
| `theme.space_12` | 12.0 | Card inner padding, list item indent |
| `theme.space_16` | 16.0 | Panel padding, section gaps |
| `theme.space_20` | 20.0 | Medium block separators |
| `theme.space_24` | 24.0 | Large block separators |
| `theme.space_40` | 40.0 | Section-level spacing, empty states |

### Corner Radius
| Token | Dark / OLED | Light | Usage |
|-------|-------------|-------|-------|
| `theme.radius_sm` | 8.0 | 6.0 | Buttons, inputs, small chips |
| `theme.radius_md` | 16.0 | 10.0 | Cards, panels |
| `theme.radius_lg` | 28.0 | 12.0 | Chat bubbles, large containers |
| `theme.radius_xl` | 36.0 | 36.0 | Modals, dialogs |
| `theme.radius_full` | 999.0 | 9999.0 | Pills, avatars |

### Typography
| Token | Value | Usage |
|-------|-------|-------|
| `theme.text_xs` | 9.0 | Captions, metadata, badges |
| `theme.text_sm` | 11.0 | Secondary labels, sidebar items |
| `theme.text_base` | 12.0 | Body text, buttons |
| `theme.text_md` | 13.0 | Emphasized body |
| `theme.text_lg` | 15.0 | Section headings |
| `theme.text_xl` | 18.0 | Dialog titles |
| `theme.text_2xl` | 24.0 | Hero / empty state headings |
| `theme.font_scale` | 1.0 | Global typography multiplier |

### Semantic Frames (helpers in `theme.rs`)
| Helper | Purpose |
|--------|---------|
| `theme.bubble_frame(is_user)` | Chat message background |
| `theme.card_frame()` | Elevated card with shadow |
| `theme.sidebar_frame()` | Side panel background |
| `theme.primary_button(...)` | Accent-filled action button |
| `theme.secondary_button(...)` | Surface-filled button |

### Layout-Related Colors
- `theme.bg`, `theme.bg_accent`, `theme.bg_elevated`, `theme.bg_hover`
- `theme.surface`, `theme.surface_strong`
- `theme.glass`, `theme.glass_strong`
- `theme.border`, `theme.border_strong`, `theme.border_hover`

### Shadows
- `theme.shadow_card` — cards, tiles
- `theme.shadow_panel` — sidebars, top bars
- `theme.shadow_modal` — dialogs, popovers
- `theme.shadow_toast` — notifications

---

## 5. PR Review Checklist

For every PR touching `crates/clarity-egui/src/**/*.rs`, the reviewer must verify:

- [ ] **RULE 1** — No `Button::new("")` + `painter` overlay sequences exist.
- [ ] **RULE 2** — No `painter.text()` or `painter.rect_filled()` used for UI widget content/backgrounds in panel code.
- [ ] **RULE 3** — No `ui.interact(raw_rect, ...)` calls remain; all interaction uses built-in widgets or `Frame::show(...).response`.
- [ ] **RULE 4** — `allocate_exact_size` is used only for spacers, drag handles, or widgets in `widgets/` with tests. Never paired with `ui.put` of an interactive widget on the same rect.
- [ ] **RULE 5** — No hardcoded pixel values `> 8.0` for spacing, sizing, or positioning; all constants route through `theme` tokens.
- [ ] **Decision Tree** — Each new widget category maps to the correct primitive (Button / SelectableLabel / Frame / painter / spacer).
- [ ] **Theme Consistency** — New colors or sizes are added to `theme.rs` if no existing token covers the need.
- [ ] **Keyboard Navigation** — Interactive elements have visible focus rings (verify by tabbing through the UI).

**If any checkbox fails → Request changes immediately.** Layout anti-patterns are P0 blockers.

---

## Appendix: Layout Debugging

When a widget compiles but does not appear, use the dedicated layout debug overlay instead of adding temporary `painter` rectangles to panel code. It visualises `max_rect` (green), `clip_rect` (blue), widget anchors (red), and warnings (yellow) with a single global toggle.

- **File:** `crates/clarity-egui/EGUI_LAYOUT_DEBUG.md`
- **Shortcut:** `Ctrl+Shift+L`
- **API:** `crate::ui::debug_overlay`

See that document for the standard 5-step diagnostic flow and the chat-header right-rail case study.

---

## Appendix: egui 0.31 Quick Reference

- `Frame::new()` replaces deprecated `Frame::group()` for custom containers.
- `CornerRadius::same(n)` replaces `Rounding::same(n)`.
- `Margin::symmetric(x, y)` sets left/right and top/right padding.
- `ui.horizontal(|ui| { ... })` and `ui.vertical(|ui| { ... })` are the primary layout primitives.
- `ui.with_layout(Layout::right_to_left(Align::Center), |ui| { ... })` for toolbar alignment.
- `SelectableLabel::new(selected, text)` is the canonical choice for toggle rows and tabs.

---

## Appendix: Production-Verified Traps (2026-05-12)

Discovered during the TitleBar regression that required **four** consecutive fixes on `window_control_button`. These are now banned by verdict.

### Trap 1 — RTL `min_rect` Never Shrinks
`ui.min_rect().width()` inside `right_to_left` always returns the full available width, not the placed-widget total. `expand_to_include_rect` only contracts `min_rect.min.x`; the `max.x` is pinned to `max_rect.max.x` for the entire RTL traversal.

**Use instead**: `cursor.max.x` delta, or an empirical `estimated_right_w`.

### Trap 2 — `ui.put` Cursor Backtrack
`allocate_space(36×36) + ui.put(Button)` net-advances the cursor by only ~14 px (content width), not 36 px. `ui.put` calls `advance_cursor_after_rect(child.min_rect())` using the Button's tight content rect, not the reserved 36 px. In RTL successive buttons overlap.

**Use instead**: Pattern A — `allocate_space` + `new_child(Sense::click)` + `Frame::inner_margin` + `Label`. No `ui.put`.

### Trap 3 — Double Interact Registration
`allocate_exact_size(Sense::click)` + `ui.put(Button)` on same rect → Button's later-registered interact swallows all events. Outer Response permanently blind.

**Use instead**: One Sense per rect.

### Trap 4 — `horizontal_centered` Double Execution
The closure runs twice (measuring pass + render pass). Every `ctx.input`, `state.mutate`, auto-id consumption fires twice. For three-zone layouts (LEFT / CENTER / RIGHT) the measuring pass also poisons the cursor.

**Use instead**: Plain `ui.horizontal` + `estimated_right_w` reserve. See `render_titlebar` in `main.rs`.

### Trap 5 — `new_child` Layout Decoupling
`new_child` ignores parent layout direction. Inside an RTL parent, the child defaults to LTR. A 14 px icon paints at `child.max_rect.min.x` (the *left* of the 36 px area), causing visible offset.

**Use instead**: `Frame::inner_margin(Margin::symmetric(11, 11))` for precise centering. Layout-direction-independent.

---

## Canonical Implementations

| Pattern | File | Use Case |
|---------|------|----------|
| A (Chrome Button) | `widgets/window_control.rs` | Fixed-size icon button with custom hover |
| B (Drag Handle) | `main.rs` drag filler in CENTER zone | Window drag, splitter |
| C (Spacer) | `ui.add_space(theme.space_*)` | Layout gap |

See `~/.config/agents/skills/egui-layout-canons/SKILL.md` for the full skill protocol.
