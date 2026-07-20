//! Provider detail panel — edit / view a provider configuration.

use crate::provider::{ApiFormat, AuthMode, ProviderDefinition, ProviderRegistry};
use crate::settings::{KimiCodeLoginState, SettingsStore};
use clarity_shell::{AppState, ToastLevel};
use clarity_ui::design_system::{self, Space, TextStyle};
use clarity_ui::theme::ICON_X;
use clarity_ui::widgets::icon_button::icon_button_toolbar;
use clarity_ui::widgets::text_input::TextInput;

fn deepseek_model_to_mode(model_id: &str) -> Option<&'static str> {
    match model_id {
        "deepseek-chat" | "fast" | "default" => Some("default"),
        "deepseek-reasoner" | "expert" => Some("expert"),
        "deepseek-vision" | "vision" => Some("vision"),
        _ => None,
    }
}

fn deepseek_mode_to_model(mode: &str) -> &'static str {
    match mode {
        "expert" => "deepseek-reasoner",
        "vision" => "deepseek-vision",
        _ => "deepseek-chat",
    }
}

pub(super) fn render_provider_detail(
    store: &mut SettingsStore,
    state: &mut dyn AppState,
    theme: &clarity_ui::theme::Theme,
    ui: &mut egui::Ui,
    prov: ProviderDefinition,
) {
    let current = prov.id.clone();

    // ── Title row ──
    ui.horizontal(|ui| {
        design_system::text(ui, prov.display(), TextStyle::Title);
        if !prov.builtin {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button_toolbar(ui, ICON_X, theme.text_sm, theme).clicked() {
                    match store.provider_registry.delete_custom(&current) {
                        Ok(()) => {
                            store.provider_registry = ProviderRegistry::load();
                            if store.settings_edit.provider == current {
                                store.settings_edit.provider.clear();
                                store.settings_edit.model.clear();
                            }
                            state.push_toast(
                                format!("Deleted: {}", prov.display()),
                                ToastLevel::Info,
                            );
                        }
                        Err(e) => state.push_toast(e, ToastLevel::Error),
                    }
                }
            });
        }
    });

    design_system::gap(ui, Space::S2);

    // ── OAuth Login / Logout buttons ──
    if prov.auth_type == crate::provider::AuthType::OAuth {
        let token_key = if prov.auth_token_key.is_empty() {
            &prov.id
        } else {
            &prov.auth_token_key
        };
        let has_token = clarity_llm::auth::TokenStore::for_provider(token_key)
            .load()
            .ok()
            .flatten()
            .is_some();
        let display = prov.display();
        let login_label = if has_token {
            format!("Re-login with {}", display)
        } else {
            format!("Login with {}", display)
        };
        ui.horizontal(|ui| {
            if ui.add(theme.primary_button(&login_label)).clicked() {
                state.open_modal(clarity_core::ui::ModalType::KimiCodeLogin);
                store.kimi_code_login_state = KimiCodeLoginState::Idle;
            }
            if has_token
                && ui
                    .add(
                        clarity_ui::widgets::Button::new("Logout")
                            .danger_ghost()
                            .small(),
                    )
                    .clicked()
            {
                match clarity_llm::auth::TokenStore::for_provider(token_key).delete() {
                    Ok(()) => {
                        state.push_toast(format!("{} logged out", display), ToastLevel::Info);
                    }
                    Err(e) => {
                        state.push_toast(format!("Logout failed: {}", e), ToastLevel::Error);
                    }
                }
            }
        });
        let status_text = if has_token {
            egui::RichText::new("Connected ✓")
                .color(theme.ok)
                .font(theme.font(theme.text_sm))
        } else {
            egui::RichText::new("Not connected")
                .color(theme.text_muted)
                .font(theme.font(theme.text_sm))
        };
        ui.label(status_text);
        design_system::gap(ui, Space::S1);
    }

    // ── Auth mode selector (deepseek-device format only) ──
    let is_deepseek_device = prov.api_format == ApiFormat::DeepSeekDevice;
    let mut is_password_mode = prov.auth_mode.is_password();
    if is_deepseek_device {
        design_system::field_label(ui, "Auth Mode");
        let modes = [(false, "Token"), (true, "Password")];
        let cur = if is_password_mode { 1 } else { 0 };
        let mut sel = cur;
        egui::ComboBox::from_id_salt(format!("{}_auth_mode", prov.id))
            .selected_text(modes[sel].1)
            .show_ui(ui, |ui| {
                for (i, (_, label)) in modes.iter().enumerate() {
                    ui.selectable_value(&mut sel, i, *label);
                }
            });
        if sel != cur {
            let mut updated = prov.clone();
            updated.auth_mode = if sel == 1 {
                AuthMode::Password
            } else {
                AuthMode::Token
            };
            if sel == 1 {
                // Switching to password: clear any saved token.
                updated.api_key_ref.clear();
            } else {
                // Switching to token: clear encrypted password.
                updated.clear_password();
            }
            let _ = store.provider_registry.update_provider(&updated);
            is_password_mode = sel == 1;
        }
        design_system::gap(ui, Space::S1);
    }

    if !is_password_mode {
        // ── API Key (editable with show/hide) ──
        let show_key_id = ui.id().with(&prov.id).with("show_key");
        let mut show_key = ui.data(|d| d.get_temp::<bool>(show_key_id).unwrap_or(false));

        design_system::field_label(ui, "API Key");
        let key_edit_id = ui.id().with(&prov.id).with("api_key_edit");
        let resolved_key = if prov.api_key_ref.starts_with("${env:") {
            prov.resolve_api_key().unwrap_or_default()
        } else {
            prov.api_key_ref.clone()
        };
        let mut key_buffer = ui.data(|d| {
            d.get_temp::<String>(key_edit_id)
                .unwrap_or(resolved_key.clone())
        });
        ui.horizontal(|ui| {
            let mut key_input = TextInput::singleline(&mut key_buffer)
                .password(!show_key)
                .width(ui.available_width() - 50.0);
            if resolved_key.is_empty() {
                key_input = key_input.hint_text("Enter API key...");
            }
            let resp = ui.add(key_input);
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(key_edit_id, key_buffer.clone()));
                let mut updated = prov.clone();
                updated.api_key_ref = key_buffer;
                let _ = store.provider_registry.update_provider(&updated);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let eye_text = if show_key { "Hide" } else { "Show" };
                if ui.add(theme.ghost_button(eye_text).small()).clicked() {
                    show_key = !show_key;
                    ui.data_mut(|d| d.insert_temp(show_key_id, show_key));
                }
            });
        });

        design_system::gap(ui, Space::S1);
    } else {
        // ── Mobile + Password login ──
        design_system::field_label(ui, "Mobile");
        let mobile_edit_id = ui.id().with(&prov.id).with("mobile_edit");
        let mut mobile_buffer = ui.data(|d| {
            d.get_temp::<String>(mobile_edit_id)
                .unwrap_or(prov.mobile.clone())
        });
        let mobile_resp = ui.add(
            TextInput::singleline(&mut mobile_buffer)
                .width(ui.available_width())
                .hint_text("+86 13800138000"),
        );
        if mobile_resp.changed() {
            ui.data_mut(|d| d.insert_temp(mobile_edit_id, mobile_buffer.clone()));
            let mut updated = prov.clone();
            updated.mobile = mobile_buffer;
            let _ = store.provider_registry.update_provider(&updated);
        }
        design_system::gap(ui, Space::S1);

        design_system::field_label(ui, "Password");
        let password_edit_id = ui.id().with(&prov.id).with("password_edit");
        let mut password_buffer =
            ui.data(|d| d.get_temp::<String>(password_edit_id).unwrap_or_default());
        let password_resp = ui.add(
            TextInput::singleline(&mut password_buffer)
                .password(true)
                .width(ui.available_width())
                .hint_text("Enter password..."),
        );
        if password_resp.changed() {
            ui.data_mut(|d| d.insert_temp(password_edit_id, password_buffer.clone()));
        }
        if password_resp.lost_focus() && !password_buffer.is_empty() {
            let mut updated = prov.clone();
            if let Err(e) = updated.set_password(&password_buffer) {
                state.push_toast(e, ToastLevel::Error);
            } else {
                let _ = store.provider_registry.update_provider(&updated);
            }
        }
        if prov.password_enc.is_some() {
            ui.label(
                egui::RichText::new("Password saved (encrypted)")
                    .size(theme.text_xs)
                    .color(theme.ok),
            );
        }

        design_system::gap(ui, Space::S1);
    }

    // ── Base URL (editable) ──
    design_system::field_label(ui, "Base URL");
    let url_edit_id = ui.id().with(&prov.id).with("base_url_edit");
    let mut url_buffer = ui.data(|d| {
        d.get_temp::<String>(url_edit_id)
            .unwrap_or_else(|| prov.base_url.clone())
    });
    let mut url_input = TextInput::singleline(&mut url_buffer).width(ui.available_width());
    if prov.base_url.is_empty() {
        url_input = url_input.hint_text("https://api.example.com/v1");
    }
    let resp = ui.add(url_input);
    if resp.changed() {
        ui.data_mut(|d| d.insert_temp(url_edit_id, url_buffer.clone()));
        let mut updated = prov.clone();
        updated.base_url = url_buffer;
        let _ = store.provider_registry.update_provider(&updated);
    }

    design_system::gap(ui, Space::S2);

    // ── Model selector ──
    if !prov.models.is_empty() {
        ui.label(
            egui::RichText::new(state.t("Model"))
                .font(theme.font(theme.text_sm))
                .color(theme.text)
                .strong(),
        );
        let mut models = prov.models.clone();
        let model_str = store.settings_edit.model.clone();
        if !models.contains(&model_str) {
            models.push(model_str.clone());
        }
        let cur = models.iter().position(|m| *m == model_str).unwrap_or(0);
        let mut sel = cur;
        egui::ComboBox::from_id_salt("st_model")
            .selected_text(&store.settings_edit.model)
            .show_ui(ui, |ui| {
                for (i, m) in models.iter().enumerate() {
                    ui.selectable_value(&mut sel, i, m.as_str());
                }
            });
        if sel != cur && sel < models.len() {
            store.settings_edit.model = models[sel].clone();
            state.auto_save_settings();
        }
        design_system::gap(ui, Space::S1);
    }

    // ── DeepSeek device mode / search toggles ──
    if is_deepseek_device {
        ui.label(
            egui::RichText::new("Mode")
                .font(theme.font(theme.text_sm))
                .color(theme.text)
                .strong(),
        );
        let modes = [("default", "快速"), ("expert", "专家"), ("vision", "识图")];
        let current_mode = prov
            .extra
            .get("model_type")
            .cloned()
            .or_else(|| deepseek_model_to_mode(&store.settings_edit.model).map(|s| s.to_string()))
            .unwrap_or_else(|| "default".to_string());
        let cur = modes
            .iter()
            .position(|(v, _)| *v == current_mode)
            .unwrap_or(0);
        let mut sel = cur;
        egui::ComboBox::from_id_salt(format!("{}_ds_mode", prov.id))
            .selected_text(modes[sel].1)
            .show_ui(ui, |ui| {
                for (i, (_, label)) in modes.iter().enumerate() {
                    ui.selectable_value(&mut sel, i, *label);
                }
            });
        if sel != cur && sel < modes.len() {
            let chosen = modes[sel].0.to_string();
            store.settings_edit.model = deepseek_mode_to_model(&chosen).to_string();
            let mut updated = prov.clone();
            updated.extra.insert("model_type".to_string(), chosen);
            let _ = store.provider_registry.update_provider(&updated);
            state.auto_save_settings();
        }
        design_system::gap(ui, Space::S1);

        let mut search_enabled = prov
            .extra
            .get("search_enabled")
            .map(|s| s == "true")
            .unwrap_or(false);
        if ui.checkbox(&mut search_enabled, "联网搜索").changed() {
            let mut updated = prov.clone();
            updated
                .extra
                .insert("search_enabled".to_string(), search_enabled.to_string());
            let _ = store.provider_registry.update_provider(&updated);
            state.auto_save_settings();
        }
        design_system::gap(ui, Space::S1);
    }

    // ── Local model path (only for local provider) ──
    if current == "local" {
        ui.label(
            egui::RichText::new("Local Model Path")
                .font(theme.font(theme.text_sm))
                .color(theme.text)
                .strong(),
        );
        let path_edit_id = ui.id().with(&prov.id).with("local_model_path");
        let mut path_buffer = ui.data(|d| {
            d.get_temp::<String>(path_edit_id).unwrap_or_else(|| {
                store
                    .settings_edit
                    .local_model_path
                    .clone()
                    .unwrap_or_default()
            })
        });
        ui.horizontal(|ui| {
            let is_empty = path_buffer.is_empty();
            let mut path_input =
                TextInput::singleline(&mut path_buffer).width(ui.available_width() - 80.0);
            if is_empty {
                path_input = path_input.hint_text("Path to .gguf file...");
            }
            let resp = ui.add(path_input);
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(path_edit_id, path_buffer.clone()));
                store.settings_edit.local_model_path = Some(path_buffer.clone());
            }
            if ui.add(theme.secondary_button("Browse")).clicked() {
                if let Some(file) = rfd::FileDialog::new()
                    .add_filter("GGUF", &["gguf"])
                    .pick_file()
                {
                    let picked = file.display().to_string();
                    ui.data_mut(|d| d.insert_temp(path_edit_id, picked.clone()));
                    store.settings_edit.local_model_path = Some(picked);
                }
            }
        });
        design_system::gap(ui, Space::S1);
    }

    // ── Approval mode ──
    ui.label(
        egui::RichText::new(state.t("Approval Mode"))
            .font(theme.font(theme.text_sm))
            .color(theme.text)
            .strong(),
    );
    let modes = ["interactive", "smart", "plan", "yolo"];
    let cur = modes
        .iter()
        .position(|m| *m == store.settings_edit.approval_mode)
        .unwrap_or(0);
    let mut ms = cur;
    egui::ComboBox::from_id_salt("st_amode")
        .selected_text(&store.settings_edit.approval_mode)
        .show_ui(ui, |ui| {
            for (i, m) in modes.iter().enumerate() {
                ui.selectable_value(&mut ms, i, *m);
            }
        });
    if ms != cur {
        store.settings_edit.approval_mode = modes[ms].to_string();
        state.auto_save_settings();
    }

    design_system::gap(ui, Space::S2);

    // ── Action buttons ──
    ui.horizontal(|ui| {
        let prov = store.provider_registry.get(&current).cloned();
        let is_local =
            current.is_empty() || prov.as_ref().map(|p| p.base_url.is_empty()).unwrap_or(true);
        let is_chat_only = prov.as_ref().map(|p| p.is_chat_only()).unwrap_or(false);
        let is_deepseek_device = prov
            .as_ref()
            .map(|p| p.api_format == ApiFormat::DeepSeekDevice)
            .unwrap_or(false);
        let supports_connection_test = !is_local && !is_chat_only && !is_deepseek_device;
        let supports_model_refresh = !is_local && !is_chat_only && !is_deepseek_device;
        let is_testing = store.testing_provider.as_deref() == Some(&current);
        let is_refreshing = store.refreshing_provider.as_deref() == Some(&current);

        // Test Connection
        let test_label = if is_testing {
            "Testing..."
        } else {
            "Test Connection"
        };
        let test_hover = if is_chat_only {
            "Connection test is not supported for chat-only providers"
        } else if is_deepseek_device {
            "Connection test is not supported for DeepSeek (Device)"
        } else {
            "Test provider connectivity"
        };
        let test_response = ui
            .add_enabled(
                !is_local && !is_testing && supports_connection_test,
                clarity_ui::widgets::Button::new(test_label)
                    .secondary()
                    .small(),
            )
            .on_hover_text(test_hover);
        if test_response.clicked() {
            let (display_name, base_url, api_fmt) = prov
                .as_ref()
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        p.api_format.runtime_api_format().to_string(),
                    )
                })
                .unwrap_or_default();
            let key = prov
                .as_ref()
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = store.settings_edit.model.clone();
            if key.is_empty() {
                state.push_toast(
                    format!("{}: No API key configured", display_name),
                    ToastLevel::Warn,
                );
            } else if let Some(Err(e)) = prov.as_ref().map(|p| p.validate_api_key_prefix()) {
                state.push_toast(e, ToastLevel::Warn);
            } else {
                store.testing_provider = Some(current.clone());
                state.spawn_provider_test(current.clone(), base_url, api_fmt, key, model);
            }
        }

        // Refresh Models
        let refresh_label = if is_refreshing {
            "Refreshing..."
        } else {
            "Refresh Models"
        };
        let refresh_hover = if is_chat_only {
            "Model refresh is not supported for chat-only providers"
        } else if is_deepseek_device {
            "Model refresh is not supported for DeepSeek (Device)"
        } else {
            "Refresh model list"
        };
        let refresh_response = ui
            .add_enabled(
                !is_local && !is_refreshing && supports_model_refresh,
                clarity_ui::widgets::Button::new(refresh_label)
                    .secondary()
                    .small(),
            )
            .on_hover_text(refresh_hover);
        if refresh_response.clicked() {
            let (_, base_url, api_fmt) = prov
                .as_ref()
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        p.api_format.runtime_api_format().to_string(),
                    )
                })
                .unwrap_or_default();
            let display_name = prov
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let key = prov
                .as_ref()
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = store.settings_edit.model.clone();
            if key.is_empty() {
                state.push_toast(
                    format!("{}: No API key configured", display_name),
                    ToastLevel::Warn,
                );
            } else if let Some(Err(e)) = prov.as_ref().map(|p| p.validate_api_key_prefix()) {
                state.push_toast(e, ToastLevel::Warn);
            } else {
                store.refreshing_provider = Some(current.clone());
                state.spawn_provider_refresh(current.clone(), base_url, api_fmt, key, model);
            }
        }

        // Apply
        if ui
            .add_enabled(
                !is_local,
                clarity_ui::widgets::Button::new("Apply").primary().small(),
            )
            .clicked()
        {
            let display_name = prov
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let model = store.settings_edit.model.clone();
            if let Some(Err(e)) = prov.as_ref().map(|p| p.validate_api_key_prefix()) {
                state.push_toast(e.clone(), ToastLevel::Warn);
            } else if is_deepseek_device {
                // DeepSeek (Device) uses a private API and must be loaded through the
                // registry-backed loader rather than the generic runtime config pipeline.
                if is_password_mode {
                    // Flush any password that has been typed but not yet encrypted
                    // (e.g. the user clicked Apply without unfocusing the field).
                    let password_edit_id = ui.id().with(&current).with("password_edit");
                    if let Some(password_buffer) =
                        ui.data(|d| d.get_temp::<String>(password_edit_id))
                    {
                        if !password_buffer.is_empty() {
                            if let Some(mut updated) = prov.clone() {
                                if let Err(e) = updated.set_password(&password_buffer) {
                                    state.push_toast(e, ToastLevel::Error);
                                } else {
                                    let _ = store.provider_registry.update_provider(&updated);
                                }
                            }
                        }
                    }
                    let prov = store.provider_registry.get(&current).cloned();
                    let mobile = prov.as_ref().map(|p| p.mobile.clone()).unwrap_or_default();
                    let has_password = prov
                        .as_ref()
                        .and_then(|p| p.resolve_password().ok().flatten())
                        .is_some();
                    if mobile.is_empty() || !has_password {
                        state.push_toast(
                            "DeepSeek (Device) password mode requires mobile and password"
                                .to_string(),
                            ToastLevel::Warn,
                        );
                        return;
                    }
                }
                state.auto_save_settings();
                state.push_toast(
                    format!("Applied: {} / {} (registry-backed)", display_name, model),
                    ToastLevel::Info,
                );
            } else {
                state.auto_save_settings();
                state.push_toast(
                    format!("Applied: {} / {}", display_name, model),
                    ToastLevel::Info,
                );
            }
        }
    });
}
