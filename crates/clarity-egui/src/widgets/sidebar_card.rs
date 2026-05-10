use crate::theme::Theme;

/// Sidebar category card.
///
/// Replaces the inline role-category card in `panels/sidebar.rs`
/// (lines 387-493) with an idiomatic `Frame` + layout implementation.
///
/// Visual spec:
/// - 56 px height, full available width
/// - Icon left (Phosphor), title top, subtitle middle, badge bottom
/// - Active fill = `theme.bg_hover`
/// - Hover fill (non-active) = `theme.bg_hover.linear_multiply(0.5)`
pub fn sidebar_card(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    subtitle: Option<&str>,
    badge: Option<&str>,
    is_active: bool,
    theme: &Theme,
) -> egui::Response {
    let desired_size = egui::vec2(ui.available_width(), 56.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let fill = if is_active {
        theme.bg_hover
    } else if response.hovered() {
        theme.bg_hover.linear_multiply(0.5)
    } else {
        egui::Color32::TRANSPARENT
    };

    let text_color = if is_active || response.hovered() {
        theme.text
    } else {
        theme.text_dim
    };

    ui.allocate_new_ui(
        egui::UiBuilder::new().max_rect(rect),
        |ui| {
            egui::Frame::new()
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(
                    theme.space_12 as i8,
                    theme.space_8 as i8,
                ))
                .show(ui, |ui| {
                    // Force the frame content to fill the allocated rect.
                    ui.set_min_size(ui.available_size());
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            // ---- Icon (Phosphor) ----
                            ui.label(
                                egui::RichText::new(icon)
                                    .font(theme.font_icon(theme.text_base))
                                    .color(text_color),
                            );

                            ui.add_space(theme.space_4);

                            // ---- Text stack ----
                            ui.vertical(|ui| {
                                // Title
                                ui.label(
                                    egui::RichText::new(title)
                                        .font(theme.font_bold(theme.text_base))
                                        .color(text_color),
                                );

                                // Subtitle with status dot
                                if let Some(sub) = subtitle {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("●")
                                                .size(theme.text_xs * 1.2)
                                                .color(theme.status_online),
                                        );
                                        ui.label(
                                            egui::RichText::new(sub)
                                                .font(theme.font(theme.text_xs))
                                                .color(theme.text_dim),
                                        );
                                    });
                                }

                                // Badge with micro dot
                                if let Some(badge_text) = badge {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("●")
                                                .size(4.0)
                                                .color(theme.border.linear_multiply(0.6)),
                                        );
                                        ui.label(
                                            egui::RichText::new(badge_text)
                                                .font(theme.font(theme.text_xs))
                                                .color(theme.text_dim),
                                        );
                                    });
                                }
                            });
                        });
                    });
                });
        },
    );

    response
}
