use crate::theme::Theme;

/// Window-control icon button (close / maximize / minimize / settings).
///
/// **Reference**: Pattern A in `egui-layout-canons` skill.
/// See `crates/clarity-egui/EGUI_LAYOUT.md` Appendix "Production-Verified Traps".
///
/// Architecture (4 steps):
/// 1. `allocate_space(36×36)` reserves exact layout slot, advances cursor by 36 px
///    in both LTR and RTL (no `ui.put` backtrack — Trap 2 avoided).
/// 2. `ui.rect_contains_pointer(rect)` queries hover state before paint
///    (we need it to choose fill/icon colors).
/// 3. `new_child(Sense::click())` creates a child Ui whose native response covers
///    the full 36×36 rect (single interact registration — Trap 3 avoided).
/// 4. `Frame::inner_margin((36-14)/2, (36-14)/2)` precisely centers the 14 px
///    icon inside the 36 px square, independent of layout direction
///    (Trap 5 avoided — no reliance on `Layout::left_to_right` inside child).
///
/// No `ui.put` is used (Trap 2 ban). No `advance_cursor_after_rect` is needed
/// because step 1 already advanced the parent cursor by exactly 36 px.
pub fn window_control_button(
    ui: &mut egui::Ui,
    icon: &str,
    theme: &Theme,
    hover_fill: egui::Color32,
    hover_icon_color: egui::Color32,
    normal_icon_color: egui::Color32,
) -> egui::Response {
    let desired_size = egui::vec2(36.0, 36.0);

    // 1. Reserve 36×36 in the parent's flow. RTL-safe.
    let (_id, rect) = ui.allocate_space(desired_size);

    // 2. Hover state before paint.
    let hovered = ui.rect_contains_pointer(rect);
    let fill = if hovered {
        hover_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    let color = if hovered {
        hover_icon_color
    } else {
        normal_icon_color
    };

    // 3. Child Ui covers the full rect with click sense.
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .sense(egui::Sense::click()),
    );

    // 4. Frame inner_margin handles centering (no layout-direction dependency).
    let icon_size = 14.0_f32;
    let margin = ((desired_size.x - icon_size) / 2.0).max(0.0);
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm.round() as u8))
        .inner_margin(egui::Margin::symmetric(margin as i8, margin as i8))
        .show(&mut child_ui, |ui| {
            ui.label(
                egui::RichText::new(icon)
                    .font(theme.font_icon(icon_size))
                    .color(color),
            );
        });

    child_ui.response()
}
