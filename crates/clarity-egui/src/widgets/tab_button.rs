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

    let inner = ui.allocate_ui(egui::vec2(width, 28.0), |ui| {
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
            ui.add_space(4.0);

            // Label: let egui handle truncation (stable, no threshold jitter).
            let spacing = ui.spacing().item_spacing.x;
            let label_w = (ui.available_width() - 18.0 - spacing - 4.0).max(10.0);
            ui.add_sized(
                egui::vec2(label_w, 28.0),
                egui::Label::new(
                    egui::RichText::new(title)
                        .size(theme.text_md)
                        .color(text_color),
                )
                .truncate(),
            );

            // Close button: always present in layout, visible only on hover.
            let close_visible = tab_hovered;
            let close_color = if close_visible {
                text_color
            } else {
                egui::Color32::TRANSPARENT
            };
            let close_resp = ui.add_sized(
                egui::vec2(18.0, 28.0),
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

        // Active tab: 1px accent line at the bottom (full width).
        if is_active {
            let line_rect = egui::Rect::from_min_max(
                egui::pos2(tab_rect.min.x, tab_rect.max.y - 1.0),
                egui::pos2(tab_rect.max.x, tab_rect.max.y),
            );
            ui.painter().rect_filled(line_rect, egui::CornerRadius::ZERO, theme.accent);
        }

    });

    let response = inner.response.interact(egui::Sense::click());
    let double_clicked = response.double_clicked();

    TabResponse {
        response,
        close_clicked,
        double_clicked,
    }
}
