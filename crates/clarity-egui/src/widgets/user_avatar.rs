//! User avatar row — circular avatar + name + optional subtitle/badge.

use crate::design_system::{self, TextStyle};
use crate::theme::Theme;

/// Render a user identity row: circular initial avatar, display name, and
/// an optional subtitle (e.g. the active model name).
///
/// # Arguments
/// - `name`: display name shown next to the avatar
/// - `subtitle`: optional secondary text (e.g. model name)
/// - `theme`: current theme
///
/// # Returns
/// `Response` of the overall row for hover/click detection.
pub fn user_avatar_row(
    ui: &mut egui::Ui,
    name: &str,
    subtitle: Option<&str>,
    theme: &Theme,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_8;

        let initial = name.chars().next().unwrap_or('U').to_string();
        let _ = crate::widgets::avatar::avatar(
            ui,
            &initial,
            theme,
            Some(theme.accent),
            Some(egui::Color32::WHITE),
        );

        ui.vertical(|ui| {
            design_system::text(ui, name, TextStyle::CaptionStrong);
            if let Some(sub) = subtitle {
                if !sub.is_empty() {
                    design_system::text(ui, sub, TextStyle::Small);
                }
            }
        });
    })
    .response
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
    fn user_avatar_row_allocates_space() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| user_avatar_row(ui, "User", Some("kimi-k2"), &theme));
        assert!(resp.rect.width() > 0.0);
        assert!(resp.rect.height() > 0.0);
    }

    #[test]
    fn user_avatar_row_omits_empty_subtitle() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| user_avatar_row(ui, "User", Some(""), &theme));
        assert!(resp.rect.width() > 0.0);
    }

    #[test]
    fn user_avatar_row_fallback_initial() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| user_avatar_row(ui, "", None, &theme));
        assert!(resp.rect.width() > 0.0);
    }
}
