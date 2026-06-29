//! Theme preview card widget — used in `components/settings/interface_tab.rs`
//! to render Dark / Light theme picker cards.
//!
//! Replaces the inline anti-pattern at `interface_tab.rs:203-256` which used
//! `allocate_exact_size + Sense::click` + 3 `painter.rect_*` calls (card bg,
//! active ring, idle border). All 3 painter calls have been replaced with
//! `Frame::fill + Frame::stroke`.
//!
//! ## Layout
//! ```text
//! ┌─────────────────────────┐
//! │       Dark              │  ← name (bold)
//! │  Deep black canvas      │  ← desc (60% alpha)
//! └─────────────────────────┘  ← border or accent ring
//!     width × height
//! ```
//!
//! ## Rule 4 exemption
//! `allocate_exact_size + Sense::click` is encapsulated **inside this widget**,
//! the canonical "custom widget in widgets/" exemption per `EGUI_LAYOUT.md`
//! §RULE 4. Callers only see the idiomatic `Response` return.

use crate::theme::Theme;

/// Render a theme preview card with the theme's real background + text colors.
///
/// # Arguments
/// - `width / height`: card size (callers compute based on layout)
/// - `card_bg / card_text`: the *previewed* theme's colors (Dark::bg etc.)
/// - `name`: theme display name shown in bold center
/// - `desc`: short description shown in 60%-alpha text below name
/// - `is_active`: drives the border — accent (2px) when active, theme.border (1px) otherwise
///
/// # Returns
/// `Response` for click detection.
#[allow(clippy::too_many_arguments)]
pub fn theme_card(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    card_bg: egui::Color32,
    card_text: egui::Color32,
    name: &str,
    desc: &str,
    is_active: bool,
    theme: &Theme,
) -> egui::Response {
    // Reserve space + register click hit-test (RULE 4 exemption for widgets/).
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    if !ui.is_rect_visible(rect) {
        return response;
    }

    // Border: 2px accent when active, 1px theme.border otherwise.
    // Frame::stroke replaces the two painter.rect_stroke calls (active + idle).
    let stroke = if is_active {
        egui::Stroke::new(2.0, theme.accent)
    } else {
        egui::Stroke::new(1.0, theme.border)
    };

    // Render the card via Frame (no painter for backgrounds).
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::new()
            .fill(card_bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .stroke(stroke)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(name)
                            .font(theme.font(theme.text_base))
                            .color(card_text)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(desc)
                            .font(theme.font(theme.text_xs))
                            .color(card_text.gamma_multiply(0.6)),
                    );
                });
            });
    });

    // Focus ring (shared helper).
    if response.has_focus() {
        crate::design_system::paint_focus_ring(
            ui,
            rect,
            egui::CornerRadius::same(theme.radius_md as u8),
        );
    }

    // Theme description tooltip on hover.
    crate::design_system::tooltip(ui, &response, format!("{} — {}", name, desc));

    response
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    /// Run a closure inside a fresh egui frame.
    ///
    /// `Context::run` expects `FnMut`, so we wrap our `FnOnce` in an `Option`
    /// and use `take()` so the closure can be moved exactly once.
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
        // SAFE: egui::CentralPanel::show always invokes its closure at least
        // once during the `ctx.run()` call. The `f_opt.take()` ensures the
        // function is called at most once, so output is always populated.
        output.expect("CentralPanel should always run its closure")
    }

    #[test]
    fn theme_card_allocates_requested_size() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            theme_card(
                ui,
                200.0,
                64.0,
                Theme::dark().bg,
                Theme::dark().text,
                "Dark",
                "Deep black canvas",
                false,
                &theme,
            )
        });
        assert!(
            (resp.rect.width() - 200.0).abs() < 0.5,
            "expected 200px width, got {}",
            resp.rect.width()
        );
        assert!(
            (resp.rect.height() - 64.0).abs() < 0.5,
            "expected 64px height, got {}",
            resp.rect.height()
        );
    }

    #[test]
    fn theme_card_active_state_preserves_size() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            theme_card(
                ui,
                200.0,
                64.0,
                Theme::light().bg,
                Theme::light().text,
                "Light",
                "Cool off-white",
                true,
                &theme,
            )
        });
        assert!(
            (resp.rect.width() - 200.0).abs() < 0.5,
            "active card should preserve width, got {}",
            resp.rect.width()
        );
    }

    #[test]
    fn theme_card_handles_empty_desc() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            theme_card(
                ui,
                150.0,
                50.0,
                Theme::dark().bg,
                Theme::dark().text,
                "Test",
                "",
                false,
                &theme,
            )
        });
        assert!(resp.rect.height() > 0.0);
    }

    #[test]
    fn theme_card_handles_long_name_without_panic() {
        let theme = Theme::default();
        let long = "A".repeat(80);
        let resp = run_in_frame(|ui| {
            theme_card(
                ui,
                200.0,
                64.0,
                Theme::dark().bg,
                Theme::dark().text,
                &long,
                "desc",
                false,
                &theme,
            )
        });
        assert!(resp.rect.height() > 0.0);
    }

    #[test]
    fn theme_card_response_is_clickable() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            theme_card(
                ui,
                100.0,
                40.0,
                Theme::dark().bg,
                Theme::dark().text,
                "T",
                "d",
                false,
                &theme,
            )
        });
        // Response must carry Sense::click() — call doesn't panic and returns bool.
        let _ignored: bool = resp.clicked();
    }
}
