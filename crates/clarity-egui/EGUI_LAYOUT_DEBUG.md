# egui Layout Debug Method — Red/Green/Blue/Yellow Borders

> **Version:** 1.0  
> **Scope:** `crates/clarity-egui/src/**/*.rs`  
> **Status:** Recommended for every layout/debugging session.  
> **Shortcut:** `Ctrl+Shift+L` toggles the overlay globally.  

This document turns the ad-hoc "paint coloured rectangles until you understand egui" technique into a reproducible, team-shared debugging protocol.

---

## 1. Why This Exists

egui's layout state is mostly implicit:

- `max_rect` — where the current `Ui` *could* place children.
- `available_rect_before_wrap()` — remaining space in the current layout pass.
- `clip_rect()` — the region that is actually visible; anything outside is clipped.
- `min_rect` — the tight bounding box of what has already been placed.

When a widget exists in code but does not appear on screen, the usual suspects are:

1. The widget was placed **outside `clip_rect`** and clipped.
2. The widget was placed **outside `max_rect`** and egui refused to allocate it.
3. The widget rendered with **zero size** or **transparent content**.
4. The widget was **over-painted** by a later widget occupying the same rect.

The coloured-border method makes these states visible in one screenshot.

---

## 2. Colour Semantics (Mandatory)

| Colour  | Meaning                                                                 | API                                |
|---------|-------------------------------------------------------------------------|------------------------------------|
| **Green**  | `ui.available_rect_before_wrap()` / `max_rect` — theoretical allocation boundary. | `debug_overlay::rect(ui, r, DebugColor::Green)` |
| **Blue**   | `ui.clip_rect()` — actual visible boundary.                              | `debug_overlay::rect(ui, r, DebugColor::Blue)` |
| **Red**    | Interaction anchor / widget placement point / far-right target.          | `debug_overlay::marker(ui, pos, DebugColor::Red)` |
| **Yellow** | Warning: zero-size rect, rect partially or fully outside clip, failed allocation. | `debug_overlay::rect(ui, r, DebugColor::Yellow)` |

**Never change these meanings.** Consistency lets any team member read a screenshot instantly.

---

## 3. How to Enable

### Runtime toggle

- **Keyboard:** `Ctrl+Shift+L`
- **Settings:** Settings → Interface → "Show green/blue/red/yellow layout diagnostics"
- **Command Palette:** "Toggle Layout Debug"

The state is persisted in `gui-settings.json` under `debug_layout_overlay`.

### Per-panel instrumentation

```rust
use crate::ui::debug_overlay;

fn render_something(app: &mut App, ui: &mut egui::Ui) {
    if debug_overlay::is_enabled(ui.ctx()) {
        debug_overlay::show_layout_state(ui, "my-panel");
    }
    // ... normal rendering
}
```

`show_layout_state` draws the blue clip rect, the green available rect, and prints both dimensions in yellow at the top-left.

### Ad-hoc markers

```rust
if debug_overlay::is_enabled(ui.ctx()) {
    let r = ui.available_rect_before_wrap();
    debug_overlay::rect(ui, r, debug_overlay::DebugColor::Green);
    debug_overlay::marker(ui, r.right_top(), debug_overlay::DebugColor::Red);
    debug_overlay::label(ui, r.min, "header", debug_overlay::DebugColor::Yellow);
}
```

---

## 4. Standard 5-Step Diagnostic Flow

When a widget is missing or misaligned:

1. **Enable the overlay** (`Ctrl+Shift+L`).
2. **Find the panel** in the screenshot and read the yellow dimension label.
3. **Compare green vs blue:**
   - If green extends past blue on the right/bottom, that zone is theoretically allocatable but clipped.
   - If green is much smaller than blue, something earlier in the layout consumed space unexpectedly.
4. **Place a red marker** at the exact coordinate where you think the widget should be.
   - If the marker is missing, the coordinate is outside `clip_rect`.
   - If the marker is present but the widget is not, the widget itself has a rendering problem (size 0, wrong font, transparent fill, etc.).
5. **Check `show_layout_state` in the parent Ui** to see whether the problem is in the current `Ui` or inherited from an ancestor (`with_centered_content`, `SidePanel`, `CentralPanel`).

---

## 5. Case Study: Chat Header Right-Rail Toggles

**Symptom:** The context-switch + drawer-expand icons in `panels/chat/header.rs` compiled but never appeared.

**Initial hypothesis:** Spacer too small, or Lucide icon not rendering.

**Debug process:**

1. Painted a green rect around `ui.available_rect_before_wrap()` — right edge at x≈880.
2. Painted a blue rect around `ui.clip_rect()` — right edge at x≈900.
3. Placed a red marker at `avail.max - 24` — **not visible**.
4. Placed a red marker at `avail.min` — **visible**.
5. Even a plain `ui.label("R")` after `ui.add_space(300.0)` did **not** appear.

**Conclusion:** The problem was not the icon, the spacer, or the widget. The `with_centered_content` sub-Ui interacts with egui's clip/max rect such that widgets pushed to the far right of the content area are clipped or not allocated. The fix requires restructuring where the header places its right group (e.g. giving the right-rail toggles their own `SidePanel`-style zone, or stopping the content-centering from clipping the header row).

**TODO:** `panels/chat/header.rs` contains a marker for this case. Resolve it after the layout debug tool lands.

---

## 6. Common Traps

### Trap 1 — `ui.add_space(large)` pushes widgets past `clip_rect`

`add_space` advances the cursor unconditionally. If the remaining width is 200 and you `add_space(300)`, the cursor ends up 100 px past `max_rect`; the next widget may be silently clipped.

**Fix:** Compute spacer as `max(0.0, available - reserved)`, or use `StripBuilder` for chrome-style zones.

### Trap 2 — `right_to_left` layout does not start at the right edge of `max_rect`

`ui.with_layout(Layout::right_to_left(...))` only reverses child order; it does not relocate the starting cursor. To right-align inside a zone you must first allocate that zone (e.g. with `StripBuilder` or `allocate_ui_with_layout`).

### Trap 3 — `with_centered_content` clip interaction

`with_centered_content` creates a child `Ui` whose `max_rect` is centred inside the parent. If that centred rect is close to the parent's right edge, egui may clip far-right children even though `available_rect_before_wrap()` reports enough width.

**Fix:** Leave generous right margin, or avoid placing right-aligned controls inside a centred content `Ui`.

### Trap 4 — Painter draws are also clipped

`ui.painter().rect_filled(...)` is **not** exempt from `clip_rect`. If a debug marker does not appear, the coordinate is outside the current `Ui`'s clip rect — exactly the information you need.

---

## 7. API Reference

Defined in `crates/clarity-egui/src/ui/debug_overlay.rs`.

```rust
pub enum DebugColor { Green, Blue, Red, Yellow }

pub fn sync_enabled(ctx: &egui::Context, enabled: bool);
pub fn is_enabled(ctx: &egui::Context) -> bool;
pub fn rect(ui: &mut egui::Ui, rect: Rect, color: DebugColor);
pub fn marker(ui: &mut egui::Ui, pos: Pos2, color: DebugColor);
pub fn cross(ui: &mut egui::Ui, pos: Pos2, color: DebugColor);
pub fn label(ui: &mut egui::Ui, pos: Pos2, text: impl Into<String>, color: DebugColor);
pub fn show_layout_state(ui: &mut egui::Ui, name: &str);
```

All functions are zero-cost when the overlay is disabled.

---

## 8. When *Not* to Use

- Do not leave `debug_overlay::rect(...)` calls in production code paths that run every frame while disabled — the check is cheap but still a branch.
- Do not use debug overlay colours for actual UI theming.
- Do not use `debug_overlay::label` for user-facing text; it bypasses text layout, RTL, and accessibility.

---

## 9. Integration Checklist

When adding a new top-level panel or chrome zone:

- [ ] Add `if debug_overlay::is_enabled(ui.ctx()) { debug_overlay::show_layout_state(ui, "name"); }` at the panel entry.
- [ ] Choose a short, unique `name` (e.g. `left-rail-icon`, `right-rail`, `chat-content`).
- [ ] Verify the overlay still compiles with `cargo clippy -p clarity-egui -- -D warnings`.
