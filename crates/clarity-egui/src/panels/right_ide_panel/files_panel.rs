//! File explorer panel placeholder.

use crate::App;

/// Render the files panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            crate::theme::ICON_FOLDER_OPEN,
            app.t("Files / workspace")
        ))
        .size(theme.text_sm)
        .color(theme.text_dim),
    );
}
