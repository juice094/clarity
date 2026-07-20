//! Circular avatar widget with a single-letter label.
//!
//! Replaces the inline painter anti-pattern in `components/chat/avatar.rs`
//! with an idiomatic `Frame` + `ui.label()` construction. The circular shape
//! is achieved via a fully rounded frame background; the label is centered
//! through egui's standard layout engine.
//!
//! ## Rule 4 exemption
//! `allocate_exact_size` lives **inside this widget**, which is the canonical
//! "custom widget in widgets/" exemption per `EGUI_LAYOUT.md` §RULE 4. Callers
//! only see the idiomatic `Response` return.

use crate::theme::Theme;

/// Render a circular avatar with a single-letter label.
///
/// # Arguments
/// - `label`: single character shown in the center of the avatar
/// - `theme`: current theme (used for surface + text colors)
/// - `bg`: optional background color; defaults to `theme.surface_strong`
/// - `fg`: optional label color; defaults to `theme.text_strong`
///
/// # Returns
/// `Response` for hover detection.
pub fn avatar(
    ui: &mut egui::Ui,
    label: &str,
    theme: &Theme,
    bg: Option<egui::Color32>,
    fg: Option<egui::Color32>,
) -> egui::Response {
    const SIZE: f32 = 28.0;
    let bg = bg.unwrap_or(theme.surface_strong);
    let fg = fg.unwrap_or(theme.text_strong);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SIZE, SIZE), egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return response;
    }

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::new()
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(255))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.with_layout(
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            egui::RichText::new(label)
                                .font(egui::FontId::proportional(theme.text_sm))
                                .color(fg),
                        );
                    },
                );
            });
    });

    response
}

/// Render a circular avatar with an explicit font size for the label.
///
/// The widget size is derived from the font size so callers can create compact
/// avatars (e.g. in the Bot bar) without changing the default 28 px avatar.
pub fn avatar_sized(
    ui: &mut egui::Ui,
    label: &str,
    theme: &Theme,
    font_size: f32,
    bg: Option<egui::Color32>,
    fg: egui::Color32,
) -> egui::Response {
    let bg = bg.unwrap_or(theme.surface_strong);
    let size = (font_size * 1.6).ceil().max(20.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return response;
    }

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::new()
            .fill(bg)
            // Fully rounded corners on a square produce a circle.
            .corner_radius(egui::CornerRadius::same(255))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.with_layout(
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            egui::RichText::new(label)
                                .font(egui::FontId::proportional(font_size))
                                .color(fg),
                        );
                    },
                );
            });
    });

    response
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn run_in_frame<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f_opt = Some(f);
        let mut output = None;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(f) = f_opt.take() {
                    output = Some(f(ui));
                }
            });
        });
        // SAFE: egui::CentralPanel::show always invokes its closure at least
        // once during the `ctx.run()` call. The `f_opt.take()` ensures the
        // function is called at most once, so output is always populated.
        output.expect("CentralPanel should always run its closure")
    }

    #[test]
    fn avatar_allocates_expected_size() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| avatar(ui, "A", &theme, None, None));
        assert!(
            (resp.rect.width() - 28.0).abs() < 0.5,
            "expected ~28px width, got {}",
            resp.rect.width()
        );
        assert!(
            (resp.rect.height() - 28.0).abs() < 0.5,
            "expected ~28px height, got {}",
            resp.rect.height()
        );
    }

    #[test]
    fn avatar_returns_hover_response() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| avatar(ui, "A", &theme, None, None));
        // Response must carry Sense::hover() — call does not panic and returns bool.
        let _ignored: bool = resp.hovered();
    }

    #[test]
    fn avatar_handles_empty_label() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| avatar(ui, "", &theme, None, None));
        assert!(resp.rect.width() > 0.0);
    }

    #[test]
    fn avatar_handles_long_label_without_panic() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| avatar(ui, "Long", &theme, None, None));
        assert!(resp.rect.width() > 0.0);
    }
}
