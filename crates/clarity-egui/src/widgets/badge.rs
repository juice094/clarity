use crate::theme::Theme;

/// Small pill badge (e.g. "openai", "ollama" in provider cards).
/// Replaces raw painter rect_filled + text constructions.
#[allow(dead_code)]
pub fn badge(ui: &mut egui::Ui, text: &str, theme: &Theme) -> egui::Response {
    egui::Frame::new()
        .fill(theme.bg_hover)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .font(egui::FontId::new(theme.text_xs, egui::FontFamily::Monospace))
                    .color(theme.text_dim),
            );
        })
        .response
}
