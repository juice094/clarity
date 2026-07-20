//! Settings UI for managing OpenClaw remote Gateway connections.

use crate::settings::SettingsStore;
use crate::settings_data::{OpenClawConnection, normalize_gateway_url, to_ws_url};
use clarity_contract::settings::OpenClawAuthMode;
use clarity_shell::{AppState, PairingState};
use clarity_ui::design_system::{self, Space, TextStyle};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::text_input::TextInput;

/// Renders the OpenClaw connections tab.
pub fn render_claw(store: &mut SettingsStore, state: &mut dyn AppState, ui: &mut egui::Ui) {
    let theme = state.theme().clone();

    design_system::text(ui, state.t("Claw"), TextStyle::Title);
    design_system::gap(ui, Space::S0);
    ui.label(
        egui::RichText::new(state.t("Manage OpenClaw Gateway connections"))
            .size(theme.text_sm)
            .color(theme.text_dim),
    );
    design_system::gap(ui, Space::S2);

    let pairing_state = state.claw_pairing_state();
    let pairing_url: Option<String> = match &pairing_state {
        PairingState::Waiting { gateway_url, .. } => Some(gateway_url.clone()),
        PairingState::Approved { gateway_url, .. } => Some(gateway_url.clone()),
        _ => None,
    };
    let pairing_status = pairing_status_text(state, &pairing_state);

    let list_height = 160.0;
    design_system::surface_panel(ui, |ui| {
        let empty_label = state.t("No OpenClaw connections configured.");
        let delete_label = state.t("Delete");
        let edit_label = state.t("Edit");
        let pair_label = state.t("Pair");
        let mut delete_idx: Option<usize> = None;
        let mut edit_idx: Option<(usize, OpenClawConnection)> = None;
        let mut pair_idx: Option<usize> = None;
        let mut save_needed = false;
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .show(ui, |ui| {
                let conns = &mut store.settings_edit.openclaw_connections;
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
                                        .add(Button::new(delete_label).danger_ghost().small())
                                        .clicked()
                                    {
                                        delete_idx = Some(idx);
                                    }
                                    if ui.add(Button::new(edit_label).ghost().small()).clicked() {
                                        edit_idx = Some((idx, conn.clone()));
                                    }
                                    if ui.add(Button::new(pair_label).ghost().small()).clicked() {
                                        pair_idx = Some(idx);
                                    }
                                },
                            );
                        });
                        design_system::gap(ui, Space::S0);
                    }
                }
            });
        if let Some((idx, conn)) = edit_idx {
            store.claw_form = conn;
            store.claw_editing_index = Some(idx);
        }
        if let Some(idx) = delete_idx {
            store.settings_edit.openclaw_connections.remove(idx);
            save_needed = true;
        }
        if let Some(idx) = pair_idx {
            state.start_openclaw_pairing(idx);
        }
        if save_needed {
            state.auto_save_settings();
        }
    });

    if !matches!(pairing_state, PairingState::Idle) {
        let cancel_label = state.t("Cancel");
        let status = pairing_status_text(state, &state.claw_pairing_state());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(status)
                    .size(theme.text_sm)
                    .color(theme.accent),
            );
            if ui.add(Button::new(cancel_label).ghost()).clicked() {
                state.cancel_openclaw_pairing();
            }
        });
    }

    design_system::gap(ui, Space::S3);

    // ── Add / edit form ──
    design_system::text(ui, state.t("Connection"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S1);

    ui.horizontal(|ui| {
        design_system::field_label(ui, state.t("Name"));
        ui.add(TextInput::singleline(&mut store.claw_form.name));
    });
    ui.horizontal(|ui| {
        design_system::field_label(ui, state.t("Gateway URL"));
        ui.add(TextInput::singleline(&mut store.claw_form.gateway_url));
    });
    ui.horizontal(|ui| {
        design_system::field_label(ui, state.t("Token"));
        ui.add(
            TextInput::singleline(store.claw_form.token.get_or_insert_with(String::new))
                .password(true)
                .width(280.0),
        );
    });

    let selected_auth_label = auth_mode_label(store.claw_form.auth_mode.clone());
    ui.horizontal(|ui| {
        clarity_ui::design_system::field_label(ui, state.t("Auth Mode"));
        egui::ComboBox::from_id_salt("claw_auth_mode")
            .selected_text(selected_auth_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut store.claw_form.auth_mode,
                    OpenClawAuthMode::TokenOnly,
                    auth_mode_label(OpenClawAuthMode::TokenOnly),
                );
                ui.selectable_value(
                    &mut store.claw_form.auth_mode,
                    OpenClawAuthMode::TokenWithDevice,
                    auth_mode_label(OpenClawAuthMode::TokenWithDevice),
                );
                ui.selectable_value(
                    &mut store.claw_form.auth_mode,
                    OpenClawAuthMode::DevicePaired,
                    auth_mode_label(OpenClawAuthMode::DevicePaired),
                );
            });
    });
    if matches!(store.claw_form.auth_mode, OpenClawAuthMode::DevicePaired) {
        ui.horizontal(|ui| {
            design_system::field_label(ui, state.t("Device Token"));
            ui.add(
                TextInput::singleline(store.claw_form.device_token.get_or_insert_with(String::new))
                    .password(true)
                    .width(280.0),
            );
        });
    }
    ui.horizontal(|ui| {
        let mut enabled = store.claw_form.enabled;
        if ui.checkbox(&mut enabled, state.t("Enabled")).changed() {
            store.claw_form.enabled = enabled;
        }
    });

    design_system::gap(ui, Space::S2);
    ui.horizontal(|ui| {
        let is_editing = store.claw_editing_index.is_some();
        let label = if is_editing {
            state.t("Update")
        } else {
            state.t("Add")
        };
        if ui.add(Button::new(label).primary()).clicked() {
            let form = store.claw_form.clone();
            if !form.gateway_url.is_empty() {
                if let Some(idx) = store.claw_editing_index {
                    if let Some(conn) = store.settings_edit.openclaw_connections.get_mut(idx) {
                        *conn = form;
                    }
                } else {
                    store.settings_edit.openclaw_connections.push(form);
                }
                store.claw_form = OpenClawConnection::default();
                store.claw_editing_index = None;
                state.auto_save_settings();
            }
        }
        if is_editing && ui.add(Button::new(state.t("Cancel")).ghost()).clicked() {
            store.claw_form = OpenClawConnection::default();
            store.claw_editing_index = None;
        }
    });
}

fn pairing_status_text(state: &dyn AppState, pairing_state: &PairingState) -> &'static str {
    match pairing_state {
        PairingState::Requesting => state.t("Pairing..."),
        PairingState::Waiting { .. } => state.t("Waiting for approval"),
        PairingState::Approved { .. } => state.t("Paired"),
        PairingState::Error(_) => state.t("Pairing failed"),
        PairingState::Idle => "",
    }
}

fn auth_mode_label(mode: OpenClawAuthMode) -> &'static str {
    match mode {
        OpenClawAuthMode::TokenOnly => "Token Only",
        OpenClawAuthMode::TokenWithDevice => "Token + Device",
        OpenClawAuthMode::DevicePaired => "Device Paired",
    }
}
