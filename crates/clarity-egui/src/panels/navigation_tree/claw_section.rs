//! Claw remote-device section in the left navigation tree.

use crate::App;
use crate::stores::BotStatus;

/// Render the Claw device list.
pub fn render_claw_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    render_section_header(ui, &theme, app.t("Claw"));

    if app.ui_store.bot_instances.is_empty() {
        ui.label(
            egui::RichText::new(app.t("No devices"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    for bot in &app.ui_store.bot_instances {
        let is_active = app.ui_store.active_bot_id == bot.id;
        let dot_color = match bot.status {
            BotStatus::Online => theme.status_online,
            BotStatus::Offline => theme.status_offline,
            BotStatus::Syncing => theme.status_busy,
        };

        let resp = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_8;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, dot_color);
            ui.label(
                egui::RichText::new(&bot.name)
                    .size(theme.text_sm)
                    .color(if is_active {
                        theme.text
                    } else {
                        theme.text_dim
                    }),
            );
        });

        if resp.response.clicked() {
            app.ui_store.active_bot_id = bot.id.clone();
        }
    }
}

fn render_section_header(ui: &mut egui::Ui, theme: &crate::theme::Theme, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_4);
}
