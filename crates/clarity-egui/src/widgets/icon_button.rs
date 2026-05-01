use crate::theme::Theme;

/// Icon button with customizable fill and corner radius.
/// Replaces manual `Button::new(RichText::new(icon).font(...)).fill(...).corner_radius(...)` constructions.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    fill: egui::Color32,
    radius: egui::CornerRadius,
    theme: &Theme,
) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(icon).font(theme.font_icon(size)))
            .fill(fill)
            .corner_radius(radius),
    )
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

/// Convenience: icon button with text color and full radius (primary action style, e.g. Send).
pub fn icon_button_primary(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    fill: egui::Color32,
    theme: &Theme,
) -> egui::Response {
    icon_button(
        ui,
        icon,
        size,
        fill,
        egui::CornerRadius::same(theme.radius_full as u8),
        theme,
    )
}
