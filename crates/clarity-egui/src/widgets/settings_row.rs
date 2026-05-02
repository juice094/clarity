use crate::theme::Theme;

/// Standard settings row: label (left) + control (right), optional hint below label.
///
/// ```
/// ┌─────────────────────────────────────────────┐
/// │  Label                    [     control     ]│
/// │  hint text                                    │
/// └─────────────────────────────────────────────┘
/// ```
pub fn settings_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    label: &str,
    hint: Option<&str>,
    control: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        // Left: label + optional hint
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .font(theme.font(theme.text_base))
                    .color(theme.text)
                    .strong(),
            );
            if let Some(h) = hint {
                ui.label(
                    egui::RichText::new(h)
                        .font(theme.font(theme.text_sm))
                        .color(theme.text_dim),
                );
            }
        });

        // Right: control, pushed to the far right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            control(ui);
        });
    });
    ui.add_space(theme.space_8);
}
