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
///
/// # Layout stability
/// Close button space is **always reserved** (18 px) so that hover-in / hover-out
/// never changes text position or tab width — eliminating the jitter caused by
/// conditional widget insertion.
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

        // ── Content row: label (left) + close (right, always reserved) ──
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            const CLOSE_RESERVE: f32 = 18.0;
            const RIGHT_PAD: f32 = 4.0;
            let max_text_w = (width - 4.0 - RIGHT_PAD - CLOSE_RESERVE).max(20.0);

            let font_id = theme.font(theme.text_md);
            let text_galley =
                ui.fonts(|f| f.layout_no_wrap(title.to_string(), font_id, egui::Color32::PLACEHOLDER));
            let text_width = text_galley.rect.width();

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

            // Close button: reserved space, visible only on hover.
            // Using a right-to-left sub-layout guarantees the × sits at the
            // trailing edge even when the label width varies.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(RIGHT_PAD);
                let close_visible = tab_hovered;
                let close_color = if close_visible {
                    text_color
                } else {
                    egui::Color32::TRANSPARENT
                };
                let close_resp = ui.add(
                    egui::Button::new(
                        egui::RichText::new(crate::theme::ICON_X)
                            .font(theme.font_icon(theme.text_xs))
                            .color(close_color),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                );
                if close_resp.clicked() && close_visible {
                    close_clicked = true;
                }
            });
        });

        // ── Active tab: 1px accent line at the bottom (full width) ──
        if is_active {
            let line_rect = egui::Rect::from_min_max(
                egui::pos2(tab_rect.min.x, tab_rect.max.y - 1.0),
                egui::pos2(tab_rect.max.x, tab_rect.max.y),
            );
            ui.painter().rect_filled(line_rect, egui::CornerRadius::ZERO, theme.accent);
        }
    });

    // `allocate_ui` returns Sense::hover() by default; re-register with click so
    // callers can detect tab selection and double-click to rename.
    let response = inner.response.interact(egui::Sense::click());
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
