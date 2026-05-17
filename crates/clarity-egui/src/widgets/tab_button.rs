use crate::theme::Theme;

/// Response produced by [`tab_button`].
pub struct TabResponse {
    pub response: egui::Response,
    pub close_clicked: bool,
    pub double_clicked: bool,
}

/// Browser-style tab widget with hover-reveal close button.
///
/// Uses egui's built-in [`Label::truncate`] for text culling instead of
/// manual width estimation, eliminating the frame-to-frame jitter caused by
/// threshold-based truncation logic.
pub fn tab_button(
    ui: &mut egui::Ui,
    title: &str,
    is_active: bool,
    theme: &Theme,
    width: f32,
) -> TabResponse {
    let mut close_clicked = false;

    let inner = ui.allocate_ui(egui::vec2(width, theme.size_tab_h), |ui| {
        let tab_rect = ui.max_rect();
        let tab_hovered = ui.rect_contains_pointer(tab_rect);

        let text_color = if is_active {
            theme.text_strong
        } else if tab_hovered {
            theme.text
        } else {
            theme.text_muted
        };

        // Two-column layout inside the tab: label (left) + close (right).
        // Close column is always allocated (18 px) so hover-in/out never
        // shifts the label.
        ui.horizontal(|ui| {
            ui.add_space(theme.space_4);

            // Label: let egui handle truncation (stable, no threshold jitter).
            // Sense::empty() prevents the label hitbox from leaking outside the tab
            // and blocking adjacent widgets (official egui pattern for non-interactive text).
            let spacing = ui.spacing().item_spacing.x;
            let label_w =
                (ui.available_width() - theme.size_close_btn_w - spacing - theme.space_4).max(0.0);
            if label_w > 0.0 {
                ui.add_sized(
                    egui::vec2(label_w, theme.size_tab_h),
                    egui::Label::new(
                        egui::RichText::new(title)
                            .size(theme.text_md)
                            .color(text_color),
                    )
                    .truncate()
                    .sense(egui::Sense::empty()),
                );
            }

            // Close button: always present in layout, visible only on hover.
            let close_visible = tab_hovered;
            let close_color = if close_visible {
                text_color
            } else {
                egui::Color32::TRANSPARENT
            };
            let close_resp = ui.add_sized(
                egui::vec2(theme.size_close_btn_w, theme.size_tab_h),
                egui::Button::new(
                    egui::RichText::new(crate::theme::ICON_X)
                        .font(theme.font_icon(theme.text_xs))
                        .color(close_color),
                )
                .fill(egui::Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm.round() as u8))
                .frame(false),
            );
            if close_resp.clicked() && close_visible {
                close_clicked = true;
            }
        });

        // Active tab: accent line at the bottom (full width).
        if is_active {
            let line_rect = egui::Rect::from_min_max(
                egui::pos2(tab_rect.min.x, tab_rect.max.y - theme.size_accent_line_h),
                egui::pos2(tab_rect.max.x, tab_rect.max.y),
            );
            ui.painter()
                .rect_filled(line_rect, egui::CornerRadius::ZERO, theme.accent);
        }
    });

    let response = inner.response.interact(egui::Sense::click());
    let double_clicked = response.double_clicked();

    // Focus ring (P0.5.E.1) — visible when Tab navigation reaches this tab.
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            egui::CornerRadius::same(theme.radius_sm as u8),
            egui::Stroke::new(2.0, theme.focus_ring),
            egui::StrokeKind::Inside,
        );
    }

    TabResponse {
        response,
        close_clicked,
        double_clicked,
    }
}
