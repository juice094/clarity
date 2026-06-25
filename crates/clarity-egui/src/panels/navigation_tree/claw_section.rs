//! Collapsible Claw role/session section in the left navigation tree.
//!
//! Claw is modelled as a long-lived role companion. Multiple discovered devices
//! may route to the *same* target session (same role + session key), so the
//! sidebar is session-centric: one row per `(role, session_key)` and a
//! collapsible sub-list of the devices that can serve it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::App;
use crate::stores::BotStatus;
use crate::ui::types::SessionContext;

/// Render the collapsible Claw session list.
pub fn render_claw_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    // Extract bool so the closure can also borrow `app`.
    let mut expanded = app.view_state.expansions.nav_claw;

    // Map (role, session_key) -> (session_id, title) for persisted sessions.
    let mut sessions_by_key: HashMap<(String, String), (String, String)> = HashMap::new();
    for s in &app.session_store.sessions {
        if let SessionContext::Claw {
            role, session_key, ..
        } = &s.context
        {
            sessions_by_key
                .entry((role.clone(), session_key.clone()))
                .or_insert_with(|| (s.id.clone(), s.title.clone()));
        }
    }

    // Session-centric device snapshot.
    let groups = app.device_state.snapshot_by_session();

    // Show every (role, session_key) that has either a persisted session or a
    // discovered device.
    let mut all_keys: BTreeSet<(String, String)> = sessions_by_key.keys().cloned().collect();
    for g in &groups {
        all_keys.insert((g.role.clone(), g.session_key.clone()));
    }

    // Group by role so Claw sessions are visually separated from the loose chat
    // history and easy to locate/preserve.
    let mut by_role: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (role, session_key) in &all_keys {
        by_role
            .entry(role.clone())
            .or_default()
            .push(session_key.clone());
    }

    crate::widgets::collapsible_section::collapsible_section(
        ui,
        "nav_claw",
        app.t("Claw"),
        crate::theme::ICON_CPU,
        &mut expanded,
        &theme,
        |ui| {
            if all_keys.is_empty() {
                ui.label(
                    egui::RichText::new(app.t("No devices"))
                        .size(theme.text_xs)
                        .color(theme.text_muted),
                );
                return;
            }

            for (role, session_keys) in by_role {
                // Role header: a subtle but distinct grouping label.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.space_8;
                    ui.label(
                        egui::RichText::new(crate::theme::ICON_CPU)
                            .font(theme.font_icon(theme.text_xs))
                            .color(theme.text_dim),
                    );
                    ui.label(
                        egui::RichText::new(&role)
                            .size(theme.text_xs)
                            .strong()
                            .color(theme.text_dim),
                    );
                });
                ui.add_space(theme.space_4);

                for session_key in session_keys {
                    let group = groups
                        .iter()
                        .find(|g| g.role == role && g.session_key == session_key);

                    // Session row status = best status of any device in the group.
                    let status = group
                        .and_then(|g| {
                            g.devices
                                .iter()
                                .map(|b| b.status)
                                .find(|s| matches!(s, BotStatus::Online | BotStatus::Syncing))
                        })
                        .unwrap_or(BotStatus::Offline);
                    let dot_color = match status {
                        BotStatus::Online => theme.status_online,
                        BotStatus::Offline => theme.status_offline,
                        BotStatus::Syncing => theme.status_busy,
                    };

                    let (session_id, title) = sessions_by_key
                        .get(&(role.clone(), session_key.clone()))
                        .cloned()
                        .unwrap_or_default();
                    let is_active =
                        !session_id.is_empty() && app.session_store.active_session_id == session_id;
                    let label_text = if title.is_empty() {
                        session_key.clone()
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
                        app.enter_claw_session(role.clone(), Some(session_key.clone()), None);
                    }

                    // Collapsible device sub-list for pinning / failover.
                    if let Some(group) = group.filter(|g| !g.devices.is_empty()) {
                        let device_count = group.devices.len();
                        let header_text = format!(
                            "{} device{}",
                            device_count,
                            if device_count == 1 { "" } else { "s" }
                        );
                        let expand_id = ui
                            .id()
                            .with("claw_devices_expanded")
                            .with(&role)
                            .with(&session_key);
                        let mut expanded_devices =
                            ui.data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));

                        let chevron = if expanded_devices {
                            crate::theme::ICON_CARET_DOWN
                        } else {
                            crate::theme::ICON_CARET_RIGHT
                        };

                        let header_resp = ui
                            .horizontal(|ui| {
                                ui.add_space(theme.space_16);
                                ui.label(
                                    egui::RichText::new(chevron)
                                        .font(theme.font_icon(theme.text_xs))
                                        .color(theme.text_muted),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(header_text)
                                            .size(theme.text_xs)
                                            .color(theme.text_muted),
                                    )
                                    .selectable(false)
                                    .sense(egui::Sense::click()),
                                )
                            })
                            .inner;

                        if header_resp.clicked() {
                            expanded_devices = !expanded_devices;
                            ui.data_mut(|d| d.insert_temp(expand_id, expanded_devices));
                        }

                        if expanded_devices {
                            for bot in &group.devices {
                                let bot_id = bot.id.clone();
                                let is_bot_active = app.ui_store.active_bot_id == bot_id;
                                let dot_color = match bot.status {
                                    BotStatus::Online => theme.status_online,
                                    BotStatus::Offline => theme.status_offline,
                                    BotStatus::Syncing => theme.status_busy,
                                };

                                let bot_label = if let Some(src) =
                                    bot.source.as_ref().filter(|s| !s.is_empty())
                                {
                                    format!("{} ({})", bot.name, src)
                                } else {
                                    bot.name.clone()
                                };

                                let resp = crate::widgets::interactive_row(
                                    ui,
                                    is_bot_active,
                                    &theme,
                                    |ui| {
                                        ui.horizontal(|ui| {
                                            ui.add_space(theme.space_16);
                                            ui.spacing_mut().item_spacing.x = theme.space_8;
                                            crate::widgets::nav_status_dot(ui, &theme, dot_color);
                                            ui.label(
                                                egui::RichText::new(&bot_label)
                                                    .size(theme.text_sm)
                                                    .color(if is_bot_active {
                                                        theme.accent
                                                    } else {
                                                        theme.text
                                                    }),
                                            );
                                        });
                                    },
                                );

                                if resp.response.clicked() {
                                    app.enter_claw_session(
                                        role.clone(),
                                        Some(session_key.clone()),
                                        Some(bot_id),
                                    );
                                }
                            }
                        }
                    }
                }

                ui.add_space(theme.space_4);
            }

            ui.add_space(theme.space_8);
        },
    );

    // Write back the expanded state.
    app.view_state.expansions.nav_claw = expanded;
}
