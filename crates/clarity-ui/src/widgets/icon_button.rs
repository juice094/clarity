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
    icon_button_with_color(ui, icon, size, fill, theme.text, radius, theme)
}

/// Icon button with explicit icon glyph colour.
///
/// Use when the button must communicate state through colour
/// (e.g. a disabled/empty send icon vs. an active accent one).
pub fn icon_button_with_color(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    fill: egui::Color32,
    color: egui::Color32,
    radius: egui::CornerRadius,
    theme: &Theme,
) -> egui::Response {
    let response = ui.add(
        egui::Button::new(
            egui::RichText::new(icon)
                .font(theme.font_icon(size))
                .color(color),
        )
        .fill(fill)
        .corner_radius(radius),
    );
    if response.has_focus() {
        crate::design_system::paint_focus_ring(ui, response.rect, radius);
    }
    response
}

/// Convenience: icon button with transparent fill and small radius (toolbar style).
///
/// The background fades in on hover with a 150 ms ease-out transition for a
/// polished micro-interaction.
pub fn icon_button_toolbar(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    theme: &Theme,
) -> egui::Response {
    icon_button_toolbar_colored(ui, icon, size, theme.text, theme)
}

/// Toolbar icon button with an explicit glyph colour.
///
/// Useful for actions that should be dimmed by default (e.g. message-row
/// actions that only appear when the row is hovered).
pub fn icon_button_toolbar_colored(
    ui: &mut egui::Ui,
    icon: &str,
    size: f32,
    color: egui::Color32,
    theme: &Theme,
) -> egui::Response {
    let padding = theme.space_8;
    let desired_size = egui::vec2(size + padding * 2.0, size + padding * 2.0);
    let (_id, rect) = ui.allocate_space(desired_size);

    // Drive the hover animation from the pointer position so we can set the
    // button fill before the widget is placed.
    let id = ui.auto_id_with(("icon_button_toolbar", icon, size.to_bits()));
    let pointer_over = ui
        .ctx()
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| rect.contains(p));
    let hover = theme.animate_value(
        ui.ctx(),
        id.with("hover"),
        if pointer_over { 1.0 } else { 0.0 },
        crate::animation::AnimationSpeed::Fast,
    );

    let fill = lerp_color(egui::Color32::TRANSPARENT, theme.bg_hover, hover);
    let btn = egui::Button::new(
        egui::RichText::new(icon)
            .font(theme.font_icon(size))
            .color(color),
    )
    .fill(fill)
    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
    let response = ui.put(rect, btn);

    if response.has_focus() {
        crate::design_system::paint_focus_ring(
            ui,
            rect,
            egui::CornerRadius::same(theme.radius_sm as u8),
        );
    }

    response
}

/// Linearly interpolate between two premultiplied RGBA colours.
fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgba_premultiplied(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
    )
}

// Note: icon_button_primary removed — use icon_button directly with desired radius.
// If a full-radius (circular) primary button is needed again, restore from git history.
