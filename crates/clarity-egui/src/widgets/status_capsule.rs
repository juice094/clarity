use crate::theme::Theme;

/// Status capsule — compact pill-shaped indicator with a colored dot + label.
///
/// Used in titlebar for connection status, gateway status, and similar
/// state indicators.
///
/// # Example
/// ```ignore
/// let resp = status_capsule(ui, "Online", theme.status_online, &theme);
/// if resp.hovered() { resp.clone().on_hover_text("Agent connection status"); }
/// ```
pub fn status_capsule(
    ui: &mut egui::Ui,
    label: &str,
    dot_color: egui::Color32,
    theme: &Theme,
) -> egui::Response {
    egui::Frame::new()
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
                        .color(theme.text_muted),
                );
            });
        })
        .response
}
