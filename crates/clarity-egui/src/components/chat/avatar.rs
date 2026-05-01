use crate::theme::Theme;

/// Circular avatar with a single-letter label.
pub fn avatar(ui: &mut egui::Ui, label: &str, theme: &Theme) -> egui::Response {
    let size = 28.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().circle_filled(rect.center(), size / 2.0, theme.surface_strong);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(theme.text_sm),
            theme.text_strong,
        );
    }
    response
}
