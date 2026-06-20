//! Claw SSH terminal panel in the right IDE rail.
//!
//! Provides an embedded terminal interface to the selected Claw device.
//! Commands are sent to the Gateway's remote-exec endpoint and output
//! is displayed in a scrollable log.

use crate::App;

/// Render the Claw SSH terminal panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    let bot = app
        .ui_store
        .bot_instances
        .iter()
        .find(|b| b.id == app.ui_store.active_bot_id)
        .or_else(|| app.ui_store.bot_instances.first());

    let (bot_name, bot_host) = match bot {
        Some(b) => (b.name.clone(), b.device_id.clone()),
        None => {
            ui.label(
                egui::RichText::new(app.t("No devices"))
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            );
            return;
        }
    };

    // Header: device name + host.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(crate::theme::ICON_TERMINAL)
                .size(theme.text_sm)
                .color(theme.accent),
        );
        ui.label(
            egui::RichText::new(format!("{}@{}", bot_name, bot_host))
                .size(theme.text_sm)
                .color(theme.text_strong),
        );
    });
    ui.add_space(theme.space_4);

    // OpenClaw Gateway connection status.
    // Per-device gateway connection status.
    let conn = app
        .device_state
        .active_connection(&app.ui_store.active_bot_id);
    let gw_url = conn
        .as_ref()
        .map(|c| c.gateway_url.clone())
        .unwrap_or_default();
    let gw_has_token = conn
        .as_ref()
        .map(|c| !c.gateway_token.is_empty())
        .unwrap_or(false);

    let gw_status = if gw_has_token {
        format!(
            "{} {} ({})",
            crate::theme::ICON_CHECK,
            app.t("Gateway connected"),
            gw_url
        )
    } else if !gw_url.is_empty() {
        format!(
            "{} {} ({})",
            crate::theme::ICON_WARNING,
            app.t("Gateway token not configured"),
            gw_url
        )
    } else {
        format!(
            "{} {}",
            crate::theme::ICON_PROHIBIT,
            app.t("No gateway configured")
        )
    };
    ui.label(
        egui::RichText::new(&gw_status)
            .size(theme.text_xs)
            .color(if gw_has_token {
                theme.text_muted
            } else {
                theme.warn
            }),
    );

    ui.add_space(theme.space_8);

    // Terminal output area — scrollable log.
    let history_count = app.ui_store.claw_history.len();
    let output_height = (ui.available_height() - 80.0).max(120.0);
    egui::ScrollArea::vertical()
        .max_height(output_height)
        .auto_shrink([false; 2])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Show loaded session history.
            if history_count > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} ({})",
                        crate::theme::ICON_LIST,
                        app.t("Session history"),
                        history_count
                    ))
                    .size(theme.text_xs)
                    .color(theme.text_muted),
                );
                ui.add_space(theme.space_4);
                for line in &app.ui_store.claw_history {
                    // Parse role-based coloring.
                    let (color, prefix) = if line.starts_with("[user]") {
                        (theme.accent, "")
                    } else if line.starts_with("[assistant]") {
                        (theme.text, "")
                    } else {
                        (theme.text_dim, "")
                    };
                    ui.label(
                        egui::RichText::new(format!("{}{}", prefix, line))
                            .size(theme.text_sm)
                            .color(color),
                    );
                }
            } else if app.claw_ws.is_some() {
                ui.label(
                    egui::RichText::new(app.t("Loading session history…"))
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            } else {
                ui.label(
                    egui::RichText::new(app.t("Connect to a Claw device to view history"))
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            }
        });

    ui.add_space(theme.space_8);

    // Command input bar.
    ui.horizontal(|ui| {
        let prompt = format!("root@{}:~# ", bot_name);
        ui.label(
            egui::RichText::new(&prompt)
                .size(theme.text_sm)
                .color(theme.accent)
                .code(),
        );
        let hint = app.t("Enter command…");
        let resp = ui.add_sized(
            egui::vec2(ui.available_width() - 4.0, 24.0),
            egui::TextEdit::singleline(&mut app.chat_store.input)
                .hint_text(hint)
                .font(egui::TextStyle::Monospace),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let cmd = app.chat_store.input.trim().to_string();
            if !cmd.is_empty() {
                if let Some(ref ws) = app.claw_ws {
                    let session_key = app
                        .session_store
                        .active_session()
                        .and_then(|s| match &s.context {
                            crate::ui::types::SessionContext::Claw { session_key, .. } => {
                                Some(session_key.clone())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| "agent:main:main".to_string());
                    ws.send_message(&session_key, &cmd);
                } else {
                    app.push_toast(
                        app.t("Not connected to Claw Gateway"),
                        crate::ui::types::ToastLevel::Warn,
                    );
                }
                app.chat_store.input.clear();
            }
        }
    });
}
