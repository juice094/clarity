use crate::theme::Theme;

/// Response produced by [`tab_button`].
pub struct TabResponse {
    pub response: egui::Response,
    pub close_clicked: bool,
    pub double_clicked: bool,
}

/// Browser-style tab widget with tail-preserve truncation and hover-reveal close button.
///
/// Replaces the anti-pattern of `allocate_exact_size` + manual painter calls for text,
/// accent line, and close icon.
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

        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);

            // Tail-preserve truncation if the title is too wide for the tab.
            let font_id = theme.font(theme.text_md);
            let text_galley =
                ui.fonts(|f| f.layout_no_wrap(title.to_string(), font_id, egui::Color32::PLACEHOLDER));
            let text_width = text_galley.rect.width();
            const TAB_PAD: f32 = 20.0;
            let max_text_w = width - TAB_PAD;
            let display_title = if text_width > max_text_w {
                truncate_title(title)
            } else {
                title.to_string()
            };

            ui.label(
                egui::RichText::new(display_title)
                    .size(theme.text_md)
                    .color(text_color),
            );

            // Close button — revealed on hover, placed at the right edge.
            if tab_hovered {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(2.0);
                    let close_resp = ui.add(
                        egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_X)
                                .font(theme.font_icon(theme.text_xs))
                                .color(text_color),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                    );
                    if close_resp.clicked() {
                        close_clicked = true;
                    }
                });
            }
        });

        // Active tab: 1px accent line at the bottom.
        if is_active {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                egui::Frame::new()
                    .fill(theme.accent)
                    .inner_margin(egui::Margin::ZERO)
                    .outer_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
                    });
            });
        }
    });

    // `allocate_ui` returns Sense::hover() by default; re-register with click so
    // callers can detect tab selection and double-click to rename.
    let response = ui.interact(inner.response.rect, inner.response.id, egui::Sense::click());
    let double_clicked = response.double_clicked();

    TabResponse {
        response,
        close_clicked,
        double_clicked,
    }
}

/// Tail-preserve truncation: keep first 4 chars + "…" + last 3 chars.
fn truncate_title(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    if chars.len() <= 8 {
        title.to_string()
    } else {
        let prefix: String = chars.iter().take(4).collect();
        let suffix: String = chars
            .iter()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{}…{}", prefix, suffix)
    }
}
