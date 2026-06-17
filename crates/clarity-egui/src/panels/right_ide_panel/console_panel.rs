//! Console / task log panel placeholder.

use crate::App;

/// Render the console panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            crate::theme::ICON_TERMINAL,
            app.t("Console / task log")
        ))
        .size(theme.text_sm)
        .color(theme.text_dim),
    );
}
