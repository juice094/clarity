use crate::theme::Theme;

/// Titlebar window-control button (close / maximize / minimize / settings).
///
/// Renders as a transparent button with a hover fill and Phosphor icon.
///
/// # Phase-2 TODO
/// Current implementation uses `painter` for fill and icon (EGUI_LAYOUT.md
/// RULE 2 violation).  Refactor to `Frame::show` + `ui.label(RichText)`
/// once egui 0.31's `Button` supports custom child layouts.
///
/// # Example
/// ```ignore
/// // Close button (danger hover)
/// let resp = window_control_button(
///     ui, ICON_X, &theme,
///     theme.danger.linear_multiply(0.25),
///     egui::Color32::WHITE,
///     14.0,
/// );
/// if resp.clicked() { ctx.send_viewport_cmd(ViewportCommand::Close); }
///
/// // Settings button (neutral hover, larger icon)
/// let resp = window_control_button(
///     ui, ICON_SETTINGS, &theme,
///     theme.overlay_medium,
///     theme.text,
///     theme.text_base,
/// );
/// ```
pub fn window_control_button(
    ui: &mut egui::Ui,
    icon: &str,
    theme: &Theme,
    hover_fill: egui::Color32,
    hover_text: egui::Color32,
    icon_size: f32,
) -> egui::Response {
    const BTN_SIZE: f32 = 36.0;

    let resp = ui.add_sized(
        egui::vec2(BTN_SIZE, BTN_SIZE),
        egui::Button::new("")
            .fill(egui::Color32::TRANSPARENT)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
    );

    let fill = if resp.hovered() {
        hover_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(
        resp.rect,
        egui::CornerRadius::same(theme.radius_sm as u8),
        fill,
    );

    let text_color = if resp.hovered() {
        hover_text
    } else {
        theme.text_dim
    };
    ui.painter().text(
        resp.rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        theme.font_icon(icon_size),
        text_color,
    );

    resp
}
