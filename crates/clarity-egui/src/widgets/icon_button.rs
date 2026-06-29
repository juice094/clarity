use crate::theme::Theme;

/// Icon button with customizable fill and corner radius.
///
/// Paints a keyboard focus ring when the button has focus, so Tab-based
/// navigation is visually tracked.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    fill: egui::Color32,
    radius: egui::CornerRadius,
    theme: &Theme,
) -> egui::Response {
    let response = ui.add(
        egui::Button::new(egui::RichText::new(icon).font(theme.font_icon(size)))
            .fill(fill)
            .corner_radius(radius),
    );
    if response.has_focus() {
        crate::design_system::paint_focus_ring(ui, response.rect, radius);
    }
    response
}

/// Convenience: icon button with transparent fill and small radius (toolbar style).
pub fn icon_button_toolbar(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    theme: &Theme,
) -> egui::Response {
    icon_button(
        ui,
        icon,
        size,
        egui::Color32::TRANSPARENT,
        egui::CornerRadius::same(theme.radius_sm as u8),
        theme,
    )
}

// Note: icon_button_primary removed — use icon_button directly with desired radius.
// If a full-radius (circular) primary button is needed again, restore from git history.
