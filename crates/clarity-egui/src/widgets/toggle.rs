use crate::theme::Theme;

/// Custom toggle switch widget.
///
/// Track: rounded rect. Thumb: circle sliding left/right.
/// On  → track filled with accent, thumb on right.
/// Off → track filled with surface, thumb on left.
#[allow(dead_code)]
pub fn toggle(ui: &mut egui::Ui, value: &mut bool, theme: &Theme) -> egui::Response {
    let desired_size = egui::vec2(36.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if response.clicked() {
        *value = !*value;
    }
    let track_radius = rect.height() / 2.0;

    // Track fill
    let track_color = if *value { theme.accent } else { theme.surface };
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(track_radius as u8),
        track_color,
    );

    // Track stroke (subtle border)
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(track_radius as u8),
        egui::Stroke::new(1.0_f32, theme.border),
        egui::StrokeKind::Inside,
    );

    // Thumb
    let thumb_margin = 3.0;
    let thumb_radius = track_radius - thumb_margin;
    let thumb_x = if *value {
        rect.max.x - thumb_radius - thumb_margin
    } else {
        rect.min.x + thumb_radius + thumb_margin
    };
    let thumb_center = egui::pos2(thumb_x, rect.center().y);
    let thumb_color = if *value {
        egui::Color32::WHITE
    } else {
        theme.text_dim
    };
    ui.painter()
        .circle_filled(thumb_center, thumb_radius, thumb_color);

    // Focus ring on hover/active
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same((track_radius + 2.0) as u8),
            egui::Stroke::new(1.5_f32, theme.focus_ring),
            egui::StrokeKind::Inside,
        );
    }

    response
}
