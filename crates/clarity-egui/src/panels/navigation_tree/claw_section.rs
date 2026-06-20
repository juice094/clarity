//! Collapsible Claw remote-device section in the left navigation tree.

use crate::App;
use crate::stores::BotStatus;

/// Render the collapsible Claw device list.
pub fn render_claw_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    // Extract bool so the closure can also borrow `app`.
    let mut expanded = app.view_state.expansions.nav_claw;

    // Materialise the role-grouped snapshot once so the borrow checker is happy
    // inside the egui closure.
    let grouped = app.device_state.snapshot_grouped();

    crate::widgets::collapsible_section::collapsible_section(
        ui,
        "nav_claw",
        app.t("Claw"),
        crate::theme::ICON_CPU,
        &mut expanded,
        &theme,
        |ui| {
            if grouped.is_empty() {
                ui.label(
                    egui::RichText::new(app.t("No devices"))
                        .size(theme.text_xs)
                        .color(theme.text_muted),
                );
            } else {
                for (role, devices) in &grouped {
                    ui.label(
                        egui::RichText::new(role)
                            .size(theme.text_xs)
                            .strong()
                            .color(theme.text_dim),
                    );
                    ui.add_space(theme.space_4);

                    for bot in devices {
                        let is_active = app.ui_store.active_bot_id == bot.id;
                        let bot_id = bot.id.clone();
                        let bot_name = bot.name.clone();
                        let dot_color = match bot.status {
                            BotStatus::Online => theme.status_online,
                            BotStatus::Offline => theme.status_offline,
                            BotStatus::Syncing => theme.status_busy,
                        };

                        let resp = crate::widgets::interactive_row(ui, is_active, &theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = theme.space_8;
                                crate::widgets::nav_status_dot(ui, &theme, dot_color);
                                ui.label(egui::RichText::new(&bot_name).size(theme.text_sm).color(
                                    if is_active {
                                        theme.text_strong
                                    } else {
                                        theme.text
                                    },
                                ));
                            });
                        });

                        if resp.response.clicked() {
                            app.ui_store.active_bot_id = bot_id;
                        }
                    }

                    ui.add_space(theme.space_4);
                }
            }
        },
    );

    // Write back the expanded state.
    app.view_state.expansions.nav_claw = expanded;
}
