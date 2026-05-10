use crate::theme::Theme;

/// Window-control icon button (close / maximize / minimize / settings).
///
/// Replaces the anti-pattern of an invisible ghost button + manual painter overlay.
/// Hover state is resolved *before* constructing the real [`egui::Button`], so the
/// fill and icon color are set directly on the widget with **zero** painter calls.
pub fn window_control_button(
    ui: &mut egui::Ui,
    icon: &str,
    theme: &Theme,
    hover_fill: egui::Color32,
    hover_icon_color: egui::Color32,
    normal_icon_color: egui::Color32,
) -> egui::Response {
    let desired_size = egui::vec2(36.0, 36.0);
    let response = ui.allocate_response(desired_size, egui::Sense::click());
    let hovered = response.hovered();
    let fill = if hovered { hover_fill } else { egui::Color32::TRANSPARENT };
    let color = if hovered { hover_icon_color } else { normal_icon_color };

    ui.put(
        response.rect,
        egui::Button::new(
            egui::RichText::new(icon)
                .font(theme.font_icon(14.0))
                .color(color),
        )
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .min_size(desired_size),
    );

    response
}
