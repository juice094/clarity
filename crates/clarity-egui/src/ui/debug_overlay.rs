//! Layout diagnostic overlay — the "red/green/blue/yellow border" method.
//!
//! S6 Phase C3: a reusable, runtime-toggleable debug layer that visualises
//! egui's implicit layout state (`available_rect`, `clip_rect`, cursor,
//! placement points).  It is intentionally not subject to `EGUI_LAYOUT.md`
//! RULE 2 because its whole purpose is to paint diagnostic geometry, not to
//! implement UI widgets.
//!
//! ## Colour semantics (mandatory — do not invent new meanings)
//!
//! | Colour | Meaning |
//! |--------|---------|
//! | Green  | `ui.available_rect_before_wrap()` / `max_rect`: the boundary the current Ui is allowed to allocate into. |
//! | Blue   | `ui.clip_rect()`: the actual visible/drawable boundary. Anything outside is clipped. |
//! | Yellow | Warning state (zero-size rect, rect outside clip rect, failed allocation). |
//!
//! ## Quick start
//!
//! ```rust
//! if crate::ui::debug_overlay::is_enabled(ui.ctx()) {
//!     crate::ui::debug_overlay::show_layout_state(ui, "chat-header");
//! }
//! ```
//!
//! Toggle at runtime with `Ctrl+Shift+L` or the Settings → Interface switch.

use egui::{Color32, Id, Pos2, Rect, Stroke, StrokeKind};

/// Stable egui memory key for the overlay toggle.
const ENABLED_KEY: &str = "clarity_debug_layout_overlay";

/// Diagnostic colours and their semantic meanings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugColor {
    /// Content/available boundary.
    Green,
    /// Clip boundary.
    Blue,
    /// Warning / anomaly.
    Yellow,
}

impl DebugColor {
    fn to_color(self) -> Color32 {
        match self {
            DebugColor::Green => Color32::from_rgb(0, 255, 0),
            DebugColor::Blue => Color32::from_rgb(0, 160, 255),
            DebugColor::Yellow => Color32::from_rgb(255, 220, 0),
        }
    }
}

/// Sync the in-memory toggle from the authoritative `ViewState`.
///
/// Call once per frame near the top of `App::update()`.
pub fn sync_enabled(ctx: &egui::Context, enabled: bool) {
    ctx.data_mut(|d| d.insert_temp(Id::new(ENABLED_KEY), enabled));
}

/// Read the current overlay state from egui memory.
pub fn is_enabled(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(Id::new(ENABLED_KEY)).unwrap_or(false))
}

/// Stroke a rectangle with a diagnostic colour.
pub fn rect(ui: &mut egui::Ui, rect: Rect, color: DebugColor) {
    if !is_enabled(ui.ctx()) {
        return;
    }
    let stroke = Stroke::new(1.0, color.to_color());
    ui.painter()
        .rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
}

/// Draw a short text label at a fixed position.
///
/// Uses the painter directly because this is diagnostic output, not a widget.
/// Avoid for production UI — this function exists only for overlay text such
/// as rect dimensions or coordinate readouts.
pub fn label(ui: &mut egui::Ui, pos: Pos2, text: impl Into<String>, color: DebugColor) {
    if !is_enabled(ui.ctx()) {
        return;
    }
    let text = text.into();
    ui.painter().text(
        pos,
        egui::Align2::LEFT_TOP,
        text,
        egui::FontId::proportional(10.0),
        color.to_color(),
    );
}

/// Draw the canonical green + blue diagnostic frame for the current Ui.
///
/// Also prints the available/clip rect extents in the top-left corner.
pub fn show_layout_state(ui: &mut egui::Ui, name: &str) {
    if !is_enabled(ui.ctx()) {
        return;
    }
    let avail = ui.available_rect_before_wrap();
    let clip = ui.clip_rect();

    // Blue first: clip boundary.
    rect(ui, clip, DebugColor::Blue);
    // Green second: available boundary (often coincides with or is inside clip).
    rect(ui, avail, DebugColor::Green);

    // Warn when the available rect exceeds the clip rect.
    if avail.max.x > clip.max.x || avail.max.y > clip.max.y {
        rect(ui, avail.intersect(clip), DebugColor::Yellow);
    }

    label(
        ui,
        avail.min + egui::vec2(2.0, 2.0),
        format!(
            "{} avail={:.0}x{:.0} clip={:.0}x{:.0}",
            name,
            avail.width(),
            avail.height(),
            clip.width(),
            clip.height()
        ),
        DebugColor::Yellow,
    );
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn run_in_frame<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f_opt = Some(f);
        let mut output = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(f) = f_opt.take() {
                    output = Some(f(ui));
                }
            });
        });
        output.expect("CentralPanel should always run its closure")
    }

    #[test]
    fn toggle_roundtrips_through_context_memory() {
        let ctx = egui::Context::default();
        assert!(!is_enabled(&ctx));
        sync_enabled(&ctx, true);
        assert!(is_enabled(&ctx));
        sync_enabled(&ctx, false);
        assert!(!is_enabled(&ctx));
    }

    #[test]
    fn debug_color_maps_to_distinct_colors() {
        let colors: Vec<_> = [DebugColor::Green, DebugColor::Blue, DebugColor::Yellow]
            .into_iter()
            .map(|c| c.to_color())
            .collect();
        // All colours must be pairwise distinct.
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j]);
            }
        }
    }

    #[test]
    fn disabled_overlay_does_not_panic() {
        run_in_frame(|ui| {
            let avail = ui.available_rect_before_wrap();
            rect(ui, avail, DebugColor::Green);
            label(ui, avail.min, "test", DebugColor::Yellow);
            show_layout_state(ui, "test");
        });
    }

    #[test]
    fn enabled_overlay_does_not_panic() {
        run_in_frame(|ui| {
            sync_enabled(ui.ctx(), true);
            let avail = ui.available_rect_before_wrap();
            rect(ui, avail, DebugColor::Green);
            label(ui, avail.min, "test", DebugColor::Yellow);
            show_layout_state(ui, "test");
        });
    }
}
