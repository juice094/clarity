use crate::theme::Theme;

/// Status capsule widget — a rounded pill showing a colored dot + label.
///
/// Replaces inline `Frame::new()` + `ui.painter().circle_filled()` constructions
/// in the title-bar status indicators.
///
/// # Layout
/// Uses `allocate_exact_size` (allowed per RULE 4 for custom widgets in `widgets/`)
/// so the capsule has a predictable, measured footprint.  A 1px subtle stroke is
/// added to prevent visual merging with adjacent capsules in cramped titlebars.
pub fn status_capsule(
    ui: &mut egui::Ui,
    dot_color: egui::Color32,
    label: &str,
    label_color: egui::Color32,
    is_clickable: bool,
    theme: &Theme,
) -> egui::Response {
    // ── Measure content to derive exact size ──
    let dot_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            "●".to_string(),
            egui::FontId::new(theme.text_sm, egui::FontFamily::Proportional),
            dot_color,
        )
    });
    let label_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            label.to_string(),
            egui::FontId::new(theme.text_xs, egui::FontFamily::Proportional),
            label_color,
        )
    });

    let has_label = !label.is_empty();
    let gap = if has_label { 2.0_f32 } else { 0.0 };
    let inner_h = dot_galley.rect.height().max(label_galley.rect.height());
    let inner_w = dot_galley.rect.width() + gap + label_galley.rect.width();

    let margin_x = theme.space_8;
    let margin_y = 5.0_f32;
    let desired_size = egui::vec2(inner_w + margin_x * 2.0, inner_h + margin_y * 2.0);

    let sense = if is_clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired_size, sense);

    // ── Background frame with subtle stroke to prevent visual merging ──
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::new()
            .fill(theme.bg_elevated)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::symmetric(margin_x as i8, margin_y as i8))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("●")
                            .size(theme.text_sm)
                            .color(dot_color),
                    );
                    if has_label {
                        ui.add_space(gap);
                        ui.label(
                            egui::RichText::new(label)
                                .size(theme.text_xs)
                                .color(label_color),
                        );
                    }
                });
            });
    });

    response
}
