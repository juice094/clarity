//! Project knowledge base panel placeholder.

use crate::App;

/// Render the knowledge base panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            crate::theme::ICON_BOOK_OPEN,
            app.t("Knowledge base")
        ))
        .size(theme.text_sm)
        .color(theme.text_dim),
    );
}
