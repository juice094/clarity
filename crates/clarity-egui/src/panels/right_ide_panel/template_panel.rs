//! Template / preset injection panel placeholder.

use crate::App;

/// Render the template panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            crate::theme::ICON_LAYOUT_TEMPLATE,
            app.t("Templates")
        ))
        .size(theme.text_sm)
        .color(theme.text_dim),
    );
}
