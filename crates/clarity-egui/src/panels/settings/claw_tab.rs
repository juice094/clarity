//! Settings UI for managing OpenClaw remote Gateway connections.

use crate::App;
use crate::claw::{normalize_gateway_url, to_ws_url};
use crate::settings::OpenClawAuthMode;

/// Renders the OpenClaw connections tab.
pub fn render_claw(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new(app.t("Claw"))
            .color(theme.text)
            .size(theme.text_lg)
            .strong(),
    );
    ui.add_space(theme.space_4);
    ui.label(
        egui::RichText::new(app.t("Manage OpenClaw Gateway connections"))
            .size(theme.text_sm)
            .color(theme.text_dim),
    );
    ui.add_space(theme.space_12);

    let pairing_url: Option<String> = match &app.claw_pairing_state {
        crate::PairingState::Waiting { gateway_url, .. } => Some(gateway_url.clone()),
        crate::PairingState::Approved { gateway_url, .. } => Some(gateway_url.clone()),
        _ => None,
    };
    let pairing_status = pairing_status_text(app);

    let list_height = 160.0;
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(theme.radius_md as u8)
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            let empty_label = app.t("No OpenClaw connections configured.");
            let delete_label = app.t("Delete");
            let edit_label = app.t("Edit");
            let pair_label = app.t("Pair");
            let mut delete_idx: Option<usize> = None;
            let mut edit_idx: Option<(usize, crate::settings::OpenClawConnection)> = None;
            let mut pair_idx: Option<usize> = None;
            let mut save_needed = false;
            egui::ScrollArea::vertical()
                .max_height(list_height)
                .show(ui, |ui| {
                    let conns = &mut app.settings_store.settings_edit.openclaw_connections;
                    if conns.is_empty() {
                        ui.label(
                            egui::RichText::new(empty_label)
                                .size(theme.text_sm)
                                .color(theme.text_muted),
                        );
                    } else {
                        for (idx, conn) in conns.iter_mut().enumerate() {
                            let is_pairing = pairing_url.as_deref().map(|u| {
                                normalize_gateway_url(u)
                                    == normalize_gateway_url(&to_ws_url(&conn.gateway_url))
                            }) == Some(true);
                            ui.horizontal(|ui| {
                                ui.set_min_width(ui.available_width());
                                let mut enabled = conn.enabled;
                                if ui.checkbox(&mut enabled, "").changed() {
                                    conn.enabled = enabled;
                                    save_needed = true;
                                }
                                ui.label(
                                    egui::RichText::new(&conn.name)
                                        .size(theme.text_sm)
                                        .color(theme.text)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(&conn.gateway_url)
                                        .size(theme.text_xs)
                                        .color(theme.text_muted),
                                );
                                if is_pairing {
                                    ui.label(
                                        egui::RichText::new(pairing_status)
                                            .size(theme.text_xs)
                                            .color(theme.accent),
                                    );
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(delete_label)
                                                    .size(theme.text_xs),
                                            )
                                            .clicked()
                                        {
                                            delete_idx = Some(idx);
                                        }
                                        if ui
                                            .button(
                                                egui::RichText::new(edit_label).size(theme.text_xs),
                                            )
                                            .clicked()
                                        {
                                            edit_idx = Some((idx, conn.clone()));
                                        }
                                        if ui
                                            .button(
                                                egui::RichText::new(pair_label).size(theme.text_xs),
                                            )
                                            .clicked()
                                        {
                                            pair_idx = Some(idx);
                                        }
                                    },
                                );
                            });
                            ui.add_space(theme.space_4);
                        }
                    }
                });
            if let Some((idx, conn)) = edit_idx {
                app.settings_store.claw_form = conn;
                app.settings_store.claw_editing_index = Some(idx);
            }
            if let Some(idx) = delete_idx {
                app.settings_store
                    .settings_edit
                    .openclaw_connections
                    .remove(idx);
                save_needed = true;
            }
            if let Some(idx) = pair_idx {
                app.start_openclaw_pairing(idx);
            }
            if save_needed {
                app.auto_save_settings();
            }
        });

    if !matches!(app.claw_pairing_state, crate::PairingState::Idle) {
        let cancel_label = app.t("Cancel");
        let status = pairing_status_text(app);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(status)
                    .size(theme.text_sm)
                    .color(theme.accent),
            );
            if ui
                .button(egui::RichText::new(cancel_label).size(theme.text_sm))
                .clicked()
            {
                app.cancel_openclaw_pairing();
            }
        });
    }

    ui.add_space(theme.space_16);

    // ── Add / edit form ──
    ui.label(
        egui::RichText::new(app.t("Connection"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_8);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(app.t("Name")).size(theme.text_sm));
        ui.text_edit_singleline(&mut app.settings_store.claw_form.name);
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(app.t("Gateway URL")).size(theme.text_sm));
        ui.text_edit_singleline(&mut app.settings_store.claw_form.gateway_url);
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(app.t("Token")).size(theme.text_sm));
        ui.add(
            egui::TextEdit::singleline(&mut app.settings_store.claw_form.token)
                .password(true)
                .desired_width(280.0),
        );
    });

    let selected_auth_label = auth_mode_label(app.settings_store.claw_form.auth_mode.clone());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(app.t("Auth Mode")).size(theme.text_sm));
        egui::ComboBox::from_id_salt("claw_auth_mode")
            .selected_text(selected_auth_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut app.settings_store.claw_form.auth_mode,
                    OpenClawAuthMode::TokenOnly,
                    auth_mode_label(OpenClawAuthMode::TokenOnly),
                );
                ui.selectable_value(
                    &mut app.settings_store.claw_form.auth_mode,
                    OpenClawAuthMode::TokenWithDevice,
                    auth_mode_label(OpenClawAuthMode::TokenWithDevice),
                );
                ui.selectable_value(
                    &mut app.settings_store.claw_form.auth_mode,
                    OpenClawAuthMode::DevicePaired,
                    auth_mode_label(OpenClawAuthMode::DevicePaired),
                );
            });
    });
    if matches!(
        app.settings_store.claw_form.auth_mode,
        OpenClawAuthMode::DevicePaired
    ) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(app.t("Device Token")).size(theme.text_sm));
            ui.add(
                egui::TextEdit::singleline(
                    app.settings_store
                        .claw_form
                        .device_token
                        .get_or_insert_with(String::new),
                )
                .password(true)
                .desired_width(280.0),
            );
        });
    }
    ui.horizontal(|ui| {
        let mut enabled = app.settings_store.claw_form.enabled;
        if ui.checkbox(&mut enabled, app.t("Enabled")).changed() {
            app.settings_store.claw_form.enabled = enabled;
        }
    });

    ui.add_space(theme.space_12);
    ui.horizontal(|ui| {
        let is_editing = app.settings_store.claw_editing_index.is_some();
        let label = if is_editing {
            app.t("Update")
        } else {
            app.t("Add")
        };
        if ui
            .add(
                egui::Button::new(egui::RichText::new(label).size(theme.text_sm))
                    .fill(theme.accent)
                    .corner_radius(theme.radius_sm as u8),
            )
            .clicked()
        {
            let form = app.settings_store.claw_form.clone();
            if !form.gateway_url.is_empty() {
                if let Some(idx) = app.settings_store.claw_editing_index {
                    if let Some(conn) = app
                        .settings_store
                        .settings_edit
                        .openclaw_connections
                        .get_mut(idx)
                    {
                        *conn = form;
                    }
                } else {
                    app.settings_store
                        .settings_edit
                        .openclaw_connections
                        .push(form);
                }
                app.settings_store.claw_form = crate::settings::OpenClawConnection::default();
                app.settings_store.claw_editing_index = None;
                app.auto_save_settings();
            }
        }
        if is_editing
            && ui
                .button(egui::RichText::new(app.t("Cancel")).size(theme.text_sm))
                .clicked()
        {
            app.settings_store.claw_form = crate::settings::OpenClawConnection::default();
            app.settings_store.claw_editing_index = None;
        }
    });
}

fn pairing_status_text(app: &App) -> &'static str {
    match &app.claw_pairing_state {
        crate::PairingState::Requesting => app.t("Pairing..."),
        crate::PairingState::Waiting { .. } => app.t("Waiting for approval"),
        crate::PairingState::Approved { .. } => app.t("Paired"),
        crate::PairingState::Error(_) => app.t("Pairing failed"),
        crate::PairingState::Idle => "",
    }
}

fn auth_mode_label(mode: OpenClawAuthMode) -> &'static str {
    match mode {
        OpenClawAuthMode::TokenOnly => "Token Only",
        OpenClawAuthMode::TokenWithDevice => "Token + Device",
        OpenClawAuthMode::DevicePaired => "Device Paired",
    }
}
