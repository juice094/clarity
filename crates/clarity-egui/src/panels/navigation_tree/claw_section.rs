//! Collapsible Claw role/session section in the left navigation tree.
//!
//! Claw is modelled as a long-lived role companion: each role has at most one
//! persistent session, and the device list below each role shows the currently
//! discovered instances that can serve that role. Role sessions live here, not
//! in the generic History section, because they are expected to follow the user
//! across restarts rather than behave like transient chat threads.

use crate::App;
use crate::stores::BotStatus;
use crate::ui::types::{DeviceAffinity, SessionContext};
use std::collections::{BTreeSet, HashMap};

/// Render the collapsible Claw role/session list.
pub fn render_claw_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    // Extract bool so the closure can also borrow `app`.
    let mut expanded = app.view_state.expansions.nav_claw;

    // Snapshot Claw sessions by role (owned copies so the closure can mutably
    // borrow `app`). In the current model there is one session per role.
    let mut sessions_by_role: HashMap<String, (String, String)> = HashMap::new();
    for s in &app.session_store.sessions {
        if let SessionContext::Claw { role, .. } = &s.context {
            sessions_by_role
                .entry(role.clone())
                .or_insert_with(|| (s.id.clone(), s.title.clone()));
        }
    }

    // Materialise the role-grouped device snapshot once.
    let grouped = app.device_state.snapshot_grouped();

    // Show every role that has either a persisted session or a discovered device.
    let mut all_roles: BTreeSet<String> = sessions_by_role.keys().cloned().collect();
    for (role, _) in &grouped {
        all_roles.insert(role.clone());
    }

    crate::widgets::collapsible_section::collapsible_section(
        ui,
        "nav_claw",
        app.t("Claw"),
        crate::theme::ICON_CPU,
        &mut expanded,
        &theme,
        |ui| {
            if all_roles.is_empty() {
                ui.label(
                    egui::RichText::new(app.t("No devices"))
                        .size(theme.text_xs)
                        .color(theme.text_muted),
                );
                return;
            }

            for role in all_roles {
                ui.label(
                    egui::RichText::new(&role)
                        .size(theme.text_xs)
                        .strong()
                        .color(theme.text_dim),
                );
                ui.add_space(theme.space_4);

                // 1. Long-lived role session (if it exists).
                if let Some((session_id, title)) = sessions_by_role.get(&role) {
                    let is_active = app.session_store.active_session_id == *session_id;
                    let status = app
                        .device_state
                        .pick_instance(&role, &DeviceAffinity::AnyOnline)
                        .map(|b| b.status)
                        .unwrap_or(BotStatus::Offline);
                    let dot_color = match status {
                        BotStatus::Online => theme.status_online,
                        BotStatus::Offline => theme.status_offline,
                        BotStatus::Syncing => theme.status_busy,
                    };
                    let label_text = if title.is_empty() {
                        role.clone()
                    } else {
                        title.clone()
                    };

                    let resp = crate::widgets::interactive_row(ui, is_active, &theme, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = theme.space_8;
                            crate::widgets::nav_status_dot(ui, &theme, dot_color);
                            ui.label(egui::RichText::new(&label_text).size(theme.text_sm).color(
                                if is_active {
                                    theme.text_strong
                                } else {
                                    theme.text
                                },
                            ));
                        });
                    });

                    if resp.response.clicked() {
                        let target_id = session_id.clone();
                        app.switch_to_session(target_id);
                        app.ui_store.active_bot_id = app
                            .device_state
                            .pick_instance(&role, &DeviceAffinity::AnyOnline)
                            .map(|b| b.id)
                            .unwrap_or_default();
                    }
                }

                // 2. Discovered devices for this role (connection targets / pinning).
                if let Some(devices) = grouped.iter().find(|(r, _)| r == &role).map(|(_, d)| d) {
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
                            app.enter_claw_session(role.clone(), Some(bot_id));
                        }
                    }
                }

                ui.add_space(theme.space_4);
            }
        },
    );

    // Write back the expanded state.
    app.view_state.expansions.nav_claw = expanded;
}
