//! Claw remote-device settings panel placeholder.

use crate::App;

/// Render the Claw settings panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            crate::theme::ICON_MONITOR,
            app.t("Claw remote settings")
        ))
        .size(theme.text_sm)
        .color(theme.text_dim),
    );

    for bot in &app.ui_store.bot_instances {
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(format!("{} — {}", bot.name, bot.device_id))
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
    }
}
