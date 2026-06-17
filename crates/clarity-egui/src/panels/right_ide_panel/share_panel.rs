//! Share / export panel placeholder.

use crate::App;

/// Render the share panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            crate::theme::ICON_SHARE,
            app.t("Share conversation")
        ))
        .size(theme.text_sm)
        .color(theme.text_dim),
    );
}
