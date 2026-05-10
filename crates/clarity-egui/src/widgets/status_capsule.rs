use crate::theme::Theme;

/// Status capsule widget — a rounded pill showing a colored dot + label.
///
/// Replaces inline `Frame::new()` + `ui.painter().circle_filled()` constructions
/// in the title-bar status indicators.
pub fn status_capsule(
    ui: &mut egui::Ui,
    dot_color: egui::Color32,
    label: &str,
    label_color: egui::Color32,
    is_clickable: bool,
    theme: &Theme,
) -> egui::Response {
    let inner = egui::Frame::new()
        .fill(theme.bg_elevated)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .size(theme.text_sm)
                        .color(dot_color),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(label)
                        .size(theme.text_xs)
                        .color(label_color),
                );
            });
        });

    if is_clickable {
        ui.interact(inner.response.rect, inner.response.id, egui::Sense::click())
    } else {
        inner.response
    }
}
