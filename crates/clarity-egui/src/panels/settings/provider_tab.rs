use crate::App;
use crate::provider::{ApiFormat, AuthMode, ProviderDefinition, ProviderRegistry};
use crate::ui::types::{SessionContext, ToastLevel, UiEvent};

use clarity_llm::runtime::{
    RuntimeProviderConfig, list_models, set_provider_config, test_connection,
};

/// Renders the provider UI.
pub fn render_provider(app: &mut App, ui: &mut egui::Ui) {
    let left_w = (ui.available_width() * 0.35).clamp(180.0, 260.0);

    ui.horizontal(|ui| {
        // ── Left column: provider list ──
        ui.allocate_ui_with_layout(
            egui::vec2(left_w, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_left_column(app, ui),
        );

        ui.add_space(app.ui_store.theme.space_12);

        // ── Right column: detail / add form / empty state ──
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_right_column(app, ui),
        );
    });
}

// ---------------------------------------------------------------------------
// Left column
// ---------------------------------------------------------------------------
fn render_left_column(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new(app.t("Provider"))
            .color(theme.text)
            .size(theme.text_lg)
            .strong(),
    );
    ui.add_space(theme.space_4);
    ui.label(
        egui::RichText::new("Connect to an AI service")
            .size(theme.text_sm)
            .color(theme.text_dim),
    );
    ui.add_space(theme.space_12);

    let active_context = app
        .session_store
        .active_session()
        .map(|s| s.context.clone())
        .unwrap_or(SessionContext::Chat);
    let show_chat_only = matches!(active_context, SessionContext::Chat);
    let all: Vec<ProviderDefinition> = app
        .settings_store
        .provider_registry
        .list()
        .into_iter()
        .filter(|p| show_chat_only || !p.is_chat_only())
        .cloned()
        .collect();
    let current = app.settings_store.settings_edit.provider.clone();

    egui::ScrollArea::vertical()
        .min_scrolled_height(200.0)
        .show(ui, |ui| {
            for p in &all {
                let is_active = p.id == current;
                let id = p.id.clone();
                let has_key = !p.api_key_ref.is_empty() && p.resolve_api_key().is_some();

                // S4-α (2026-05-11): extracted to widgets/provider_row.rs.
                // The previous inline implementation used `allocate_exact_size +
                // Sense::click()` plus two `painter.rect_filled` calls. Both
                // painter calls are gone; the widget uses `Frame::fill` +
                // `Frame::stroke` for backgrounds and active accent.
                let resp = crate::widgets::provider_row(
                    ui,
                    &theme,
                    p.display(),
                    has_key,
                    p.models.len(),
                    is_active,
                );

                if resp.clicked() && !is_active {
                    app.settings_store.settings_edit.provider = id.clone();
                    app.settings_store.show_add_provider = false;
                    if let Some(prov) = app.settings_store.provider_registry.get(&id) {
                        if !prov.models.is_empty() {
                            app.settings_store.settings_edit.model = prov.models[0].clone();
                        }
                    }
                    app.auto_save_settings();
                }
            }
        });

    ui.add_space(theme.space_12);

    if ui.add(theme.primary_button("+ Add Custom")).clicked() {
        app.settings_store.show_add_provider = !app.settings_store.show_add_provider;
    }
}

// ---------------------------------------------------------------------------
// Right column
// ---------------------------------------------------------------------------
fn render_right_column(app: &mut App, ui: &mut egui::Ui) {
    if app.settings_store.show_add_provider {
        render_add_form(app, ui);
        return;
    }

    let current = app.settings_store.settings_edit.provider.clone();
    let prov_opt = app.settings_store.provider_registry.get(&current).cloned();

    if let Some(prov) = prov_opt {
        render_provider_detail(app, ui, prov);
    } else {
        render_empty_state(app, ui);
    }
}

fn render_empty_state(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40 * 2.0);
        ui.label(
            egui::RichText::new("Select a provider")
                .font(theme.font(theme.text_md))
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_4);
        ui.label(
            egui::RichText::new("Choose from the list or add a custom provider")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
    });
}

fn render_provider_detail(app: &mut App, ui: &mut egui::Ui, prov: ProviderDefinition) {
    let theme = app.ui_store.theme.clone();
    let current = prov.id.clone();

    // ── Title row ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(prov.display())
                .font(theme.font(theme.text_lg))
                .color(theme.text)
                .strong(),
        );
        if !prov.builtin {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn = egui::Button::new(
                    egui::RichText::new(crate::theme::ICON_X).font(theme.font_icon(theme.text_sm)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                if ui.add(btn).clicked() {
                    match app.settings_store.provider_registry.delete_custom(&current) {
                        Ok(()) => {
                            app.settings_store.provider_registry = ProviderRegistry::load();
                            if app.settings_store.settings_edit.provider == current {
                                app.settings_store.settings_edit.provider.clear();
                                app.settings_store.settings_edit.model.clear();
                            }
                            app.push_toast(
                                format!("Deleted: {}", prov.display()),
                                ToastLevel::Info,
                            );
                        }
                        Err(e) => app.push_toast(e, ToastLevel::Error),
                    }
                }
            });
        }
    });

    ui.add_space(theme.space_12);

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
                app.view_state
                    .open_modal(clarity_core::ui::ModalType::KimiCodeLogin);
                app.settings_store.kimi_code_login_state = crate::stores::KimiCodeLoginState::Idle;
            }
            if has_token {
                let logout_btn = egui::Button::new(
                    egui::RichText::new("Logout")
                        .font(theme.font(theme.text_sm))
                        .color(theme.danger),
                )
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                if ui.add(logout_btn).clicked() {
                    match clarity_llm::auth::TokenStore::for_provider(token_key).delete() {
                        Ok(()) => {
                            app.push_toast(format!("{} logged out", display), ToastLevel::Info);
                        }
                        Err(e) => {
                            app.push_toast(format!("Logout failed: {}", e), ToastLevel::Error);
                        }
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
        ui.add_space(theme.space_8);
    }

    // ── Auth mode selector (deepseek-device only) ──
    let is_deepseek_device = prov.id == "deepseek-device";
    let mut is_password_mode = prov.auth_mode.is_password();
    if is_deepseek_device {
        ui.label(
            egui::RichText::new("Auth Mode")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
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
            let _ = app
                .settings_store
                .provider_registry
                .update_provider(&updated);
            is_password_mode = sel == 1;
        }
        ui.add_space(theme.space_8);
    }

    if !is_password_mode {
        // ── API Key (editable with show/hide) ──
        let show_key_id = ui.id().with(&prov.id).with("show_key");
        let mut show_key = ui.data(|d| d.get_temp::<bool>(show_key_id).unwrap_or(false));

        ui.label(
            egui::RichText::new("API Key")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
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
            let mut te = egui::TextEdit::singleline(&mut key_buffer)
                .password(!show_key)
                .desired_width(ui.available_width() - 50.0)
                .font(theme.font(theme.text_base))
                .frame(false);
            if resolved_key.is_empty() {
                te = te.hint_text("Enter API key...");
            }
            let resp = ui.add(te);
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(key_edit_id, key_buffer.clone()));
                let mut updated = prov.clone();
                updated.api_key_ref = key_buffer;
                let _ = app
                    .settings_store
                    .provider_registry
                    .update_provider(&updated);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let eye_text = if show_key { "Hide" } else { "Show" };
                if ui.add(theme.ghost_button(eye_text)).clicked() {
                    show_key = !show_key;
                    ui.data_mut(|d| d.insert_temp(show_key_id, show_key));
                }
            });
        });

        ui.add_space(theme.space_8);
    } else {
        // ── Mobile + Password login ──
        ui.label(
            egui::RichText::new("Mobile")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
        let mobile_edit_id = ui.id().with(&prov.id).with("mobile_edit");
        let mut mobile_buffer = ui.data(|d| {
            d.get_temp::<String>(mobile_edit_id)
                .unwrap_or(prov.mobile.clone())
        });
        let mobile_resp = ui.add(
            egui::TextEdit::singleline(&mut mobile_buffer)
                .desired_width(ui.available_width())
                .font(theme.font(theme.text_base))
                .frame(false)
                .hint_text("+86 13800138000"),
        );
        if mobile_resp.changed() {
            ui.data_mut(|d| d.insert_temp(mobile_edit_id, mobile_buffer.clone()));
            let mut updated = prov.clone();
            updated.mobile = mobile_buffer;
            let _ = app
                .settings_store
                .provider_registry
                .update_provider(&updated);
        }
        ui.add_space(theme.space_8);

        ui.label(
            egui::RichText::new("Password")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
        let password_edit_id = ui.id().with(&prov.id).with("password_edit");
        let mut password_buffer =
            ui.data(|d| d.get_temp::<String>(password_edit_id).unwrap_or_default());
        let password_resp = ui.add(
            egui::TextEdit::singleline(&mut password_buffer)
                .password(true)
                .desired_width(ui.available_width())
                .font(theme.font(theme.text_base))
                .frame(false)
                .hint_text("Enter password..."),
        );
        if password_resp.changed() {
            ui.data_mut(|d| d.insert_temp(password_edit_id, password_buffer.clone()));
        }
        if password_resp.lost_focus() && !password_buffer.is_empty() {
            let mut updated = prov.clone();
            if let Err(e) = updated.set_password(&password_buffer) {
                app.push_toast(e, ToastLevel::Error);
            } else {
                let _ = app
                    .settings_store
                    .provider_registry
                    .update_provider(&updated);
            }
        }
        if prov.password_enc.is_some() {
            ui.label(
                egui::RichText::new("Password saved (encrypted)")
                    .size(theme.text_xs)
                    .color(theme.ok),
            );
        }

        ui.add_space(theme.space_8);
    }

    // ── Base URL (editable) ──
    ui.label(
        egui::RichText::new("Base URL")
            .font(theme.font(theme.text_sm))
            .color(theme.text_muted),
    );
    let url_edit_id = ui.id().with(&prov.id).with("base_url_edit");
    let mut url_buffer = ui.data(|d| {
        d.get_temp::<String>(url_edit_id)
            .unwrap_or_else(|| prov.base_url.clone())
    });
    let mut te = egui::TextEdit::singleline(&mut url_buffer)
        .desired_width(ui.available_width())
        .font(theme.font(theme.text_base))
        .frame(false);
    if prov.base_url.is_empty() {
        te = te.hint_text("https://api.example.com/v1");
    }
    let resp = ui.add(te);
    if resp.changed() {
        ui.data_mut(|d| d.insert_temp(url_edit_id, url_buffer.clone()));
        let mut updated = prov.clone();
        updated.base_url = url_buffer;
        let _ = app
            .settings_store
            .provider_registry
            .update_provider(&updated);
    }

    ui.add_space(theme.space_12);

    // ── Model selector ──
    if !prov.models.is_empty() {
        ui.label(
            egui::RichText::new(app.t("Model"))
                .font(theme.font(theme.text_sm))
                .color(theme.text)
                .strong(),
        );
        let mut models = prov.models.clone();
        let model_str = app.settings_store.settings_edit.model.clone();
        if !models.contains(&model_str) {
            models.push(model_str.clone());
        }
        let cur = models.iter().position(|m| *m == model_str).unwrap_or(0);
        let mut sel = cur;
        egui::ComboBox::from_id_salt("st_model")
            .selected_text(&app.settings_store.settings_edit.model)
            .show_ui(ui, |ui| {
                for (i, m) in models.iter().enumerate() {
                    ui.selectable_value(&mut sel, i, m.as_str());
                }
            });
        if sel != cur && sel < models.len() {
            app.settings_store.settings_edit.model = models[sel].clone();
            app.auto_save_settings();
        }
        ui.add_space(theme.space_8);
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
                app.settings_store
                    .settings_edit
                    .local_model_path
                    .clone()
                    .unwrap_or_default()
            })
        });
        ui.horizontal(|ui| {
            let is_empty = path_buffer.is_empty();
            let mut te = egui::TextEdit::singleline(&mut path_buffer)
                .desired_width(ui.available_width() - 80.0)
                .font(theme.font(theme.text_base))
                .frame(false);
            if is_empty {
                te = te.hint_text("Path to .gguf file...");
            }
            let resp = ui.add(te);
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(path_edit_id, path_buffer.clone()));
                app.settings_store.settings_edit.local_model_path = Some(path_buffer.clone());
            }
            if ui.add(theme.secondary_button("Browse")).clicked() {
                if let Some(file) = rfd::FileDialog::new()
                    .add_filter("GGUF", &["gguf"])
                    .pick_file()
                {
                    let picked = file.display().to_string();
                    ui.data_mut(|d| d.insert_temp(path_edit_id, picked.clone()));
                    app.settings_store.settings_edit.local_model_path = Some(picked);
                }
            }
        });
        ui.add_space(theme.space_8);
    }

    // ── Approval mode ──
    ui.label(
        egui::RichText::new(app.t("Approval Mode"))
            .font(theme.font(theme.text_sm))
            .color(theme.text)
            .strong(),
    );
    let modes = ["interactive", "smart", "plan", "yolo"];
    let cur = modes
        .iter()
        .position(|m| *m == app.settings_store.settings_edit.approval_mode)
        .unwrap_or(0);
    let mut ms = cur;
    egui::ComboBox::from_id_salt("st_amode")
        .selected_text(&app.settings_store.settings_edit.approval_mode)
        .show_ui(ui, |ui| {
            for (i, m) in modes.iter().enumerate() {
                ui.selectable_value(&mut ms, i, *m);
            }
        });
    if ms != cur {
        app.settings_store.settings_edit.approval_mode = modes[ms].to_string();
        app.auto_save_settings();
    }

    ui.add_space(theme.space_12);

    // ── Action buttons ──
    ui.horizontal(|ui| {
        let prov = app.settings_store.provider_registry.get(&current).cloned();
        let is_local =
            current.is_empty() || prov.as_ref().map(|p| p.base_url.is_empty()).unwrap_or(true);
        let is_chat_only = prov.as_ref().map(|p| p.is_chat_only()).unwrap_or(false);
        let is_testing = app.settings_store.testing_provider.as_deref() == Some(&current);
        let is_refreshing = app.settings_store.refreshing_provider.as_deref() == Some(&current);

        // Test Connection
        let test_label = if is_testing {
            "Testing..."
        } else {
            "Test Connection"
        };
        let test_btn = egui::Button::new(egui::RichText::new(test_label).size(theme.text_sm))
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        let test_hover = if is_chat_only {
            "Connection test is not supported for chat-only providers"
        } else {
            "Test provider connectivity"
        };
        let test_response = ui
            .add_enabled(!is_local && !is_testing && !is_chat_only, test_btn)
            .on_hover_text(test_hover);
        if test_response.clicked() {
            let (display_name, base_url, api_fmt) = prov
                .as_ref()
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        map_api_format(p.api_format.as_str()).to_string(),
                    )
                })
                .unwrap_or_default();
            let key = prov
                .as_ref()
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = app.settings_store.settings_edit.model.clone();
            if key.is_empty() {
                app.push_toast(
                    format!("{}: No API key configured", display_name),
                    ToastLevel::Warn,
                );
            } else if let Some(Err(e)) = prov.as_ref().map(|p| p.validate_api_key_prefix()) {
                app.push_toast(e, ToastLevel::Warn);
            } else {
                let cfg = RuntimeProviderConfig {
                    provider_id: current.clone(),
                    base_url,
                    api_format: api_fmt,
                    api_key: key,
                    model,
                };
                app.settings_store.testing_provider = Some(current.clone());
                let tx = app.ui_tx.clone();
                let pid = current.clone();
                app.runtime.spawn(async move {
                    let result = test_connection(&cfg).await;
                    let (success, error) = match result {
                        Ok(()) => (true, None),
                        Err(e) => (false, Some(e)),
                    };
                    let _ = tx.send(UiEvent::ProviderTestResult {
                        provider_id: pid,
                        success,
                        error,
                    });
                });
            }
        }

        // Refresh Models
        let refresh_label = if is_refreshing {
            "Refreshing..."
        } else {
            "Refresh Models"
        };
        let refresh_btn = egui::Button::new(egui::RichText::new(refresh_label).size(theme.text_sm))
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        let refresh_hover = if is_chat_only {
            "Model refresh is not supported for chat-only providers"
        } else {
            "Refresh model list"
        };
        let refresh_response = ui
            .add_enabled(!is_local && !is_refreshing && !is_chat_only, refresh_btn)
            .on_hover_text(refresh_hover);
        if refresh_response.clicked() {
            let (_, base_url, api_fmt) = prov
                .as_ref()
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        map_api_format(p.api_format.as_str()).to_string(),
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
            let model = app.settings_store.settings_edit.model.clone();
            if key.is_empty() {
                app.push_toast(
                    format!("{}: No API key configured", display_name),
                    ToastLevel::Warn,
                );
            } else if let Some(Err(e)) = prov.as_ref().map(|p| p.validate_api_key_prefix()) {
                app.push_toast(e, ToastLevel::Warn);
            } else {
                let cfg = RuntimeProviderConfig {
                    provider_id: current.clone(),
                    base_url,
                    api_format: api_fmt,
                    api_key: key,
                    model,
                };
                app.settings_store.refreshing_provider = Some(current.clone());
                let tx = app.ui_tx.clone();
                let pid = current.clone();
                app.runtime.spawn(async move {
                    let models = list_models(&cfg).await.unwrap_or_default();
                    let _ = tx.send(UiEvent::ProviderModelList {
                        provider_id: pid,
                        models,
                    });
                });
            }
        }

        // Apply
        let apply_btn = egui::Button::new(
            egui::RichText::new("Apply")
                .size(theme.text_sm)
                .color(theme.text),
        )
        .fill(theme.accent)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        if ui.add_enabled(!is_local, apply_btn).clicked() {
            let (display_name, base_url, api_fmt) = prov
                .as_ref()
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        map_api_format(p.api_format.as_str()).to_string(),
                    )
                })
                .unwrap_or_default();
            let key = prov
                .as_ref()
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = app.settings_store.settings_edit.model.clone();
            if let Some(Err(e)) = prov.as_ref().map(|p| p.validate_api_key_prefix()) {
                app.push_toast(e.clone(), ToastLevel::Warn);
            } else if is_chat_only {
                // Chat-only providers (e.g. deepseek-device) are loaded through the
                // ModelRegistry rather than the runtime active-config pipeline.
                if current == "deepseek-device" && is_password_mode {
                    // Flush any password that has been typed but not yet encrypted
                    // (e.g. the user clicked Apply without unfocusing the field).
                    let password_edit_id = ui.id().with(&current).with("password_edit");
                    if let Some(password_buffer) =
                        ui.data(|d| d.get_temp::<String>(password_edit_id))
                    {
                        if !password_buffer.is_empty() {
                            if let Some(mut updated) = prov.clone() {
                                if let Err(e) = updated.set_password(&password_buffer) {
                                    app.push_toast(e, ToastLevel::Error);
                                } else {
                                    let _ = app
                                        .settings_store
                                        .provider_registry
                                        .update_provider(&updated);
                                }
                            }
                        }
                    }
                    let prov = app.settings_store.provider_registry.get(&current).cloned();
                    let mobile = prov.as_ref().map(|p| p.mobile.clone()).unwrap_or_default();
                    let has_password = prov.as_ref().and_then(|p| p.resolve_password()).is_some();
                    if mobile.is_empty() || !has_password {
                        app.push_toast(
                            "DeepSeek (Device) password mode requires mobile and password",
                            ToastLevel::Warn,
                        );
                        return;
                    }
                }
                app.auto_save_settings();
                app.push_toast(
                    format!("Applied: {} / {} (registry-backed)", display_name, model),
                    ToastLevel::Info,
                );
            } else {
                let cfg = RuntimeProviderConfig {
                    provider_id: current.clone(),
                    base_url,
                    api_format: api_fmt,
                    api_key: key,
                    model: model.clone(),
                };
                set_provider_config(cfg);
                app.auto_save_settings();
                app.push_toast(
                    format!("Applied: {} / {}", display_name, model),
                    ToastLevel::Info,
                );
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Add custom provider form
// ---------------------------------------------------------------------------
fn render_add_form(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    egui::Frame::new()
        .fill(theme.bg_accent)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Add Custom Provider")
                    .strong()
                    .color(theme.text)
                    .size(theme.text_base),
            );
            ui.add_space(8.0);

            ui.label(
                egui::RichText::new("Name")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_store.add_provider_name)
                    .hint_text("my-provider")
                    .desired_width(240.0),
            );
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Base URL")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_store.add_provider_url)
                    .hint_text("https://...")
                    .desired_width(240.0),
            );
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("API Format")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            let fmts = ["openai-completions", "anthropic-messages"];
            let mut fi = fmts
                .iter()
                .position(|f| *f == app.settings_store.add_provider_format)
                .unwrap_or(0);
            egui::ComboBox::from_id_salt("add_fmt")
                .selected_text(app.settings_store.add_provider_format.as_str())
                .show_ui(ui, |ui| {
                    for (i, f) in fmts.iter().enumerate() {
                        ui.selectable_value(&mut fi, i, *f);
                    }
                });
            if fi < fmts.len() {
                app.settings_store.add_provider_format = fmts[fi].to_string();
            }
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("API Key")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_store.add_provider_key)
                    .hint_text("${env:KEY}")
                    .desired_width(240.0),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add(theme.primary_button("Save")).clicked() {
                    let name = app
                        .settings_store
                        .add_provider_name
                        .trim()
                        .to_lowercase()
                        .replace(' ', "-");
                    if !name.is_empty() && !app.settings_store.add_provider_url.trim().is_empty() {
                        let def = ProviderDefinition {
                            id: name.clone(),
                            display_name: app.settings_store.add_provider_name.trim().into(),
                            base_url: app.settings_store.add_provider_url.trim().into(),
                            api_format: ApiFormat::from_str(
                                &app.settings_store.add_provider_format,
                            ),
                            auth_type: crate::provider::AuthType::ApiKey,
                            api_key_ref: app.settings_store.add_provider_key.trim().into(),
                            auth_token_key: String::new(),
                            models: vec![],
                            builtin: false,
                            tags: vec![],
                            ..Default::default()
                        };
                        match def.validate_api_key_prefix() {
                            Err(e) => app.push_toast(e, ToastLevel::Warn),
                            Ok(()) => {
                                match app.settings_store.provider_registry.save_custom(&def) {
                                    Ok(()) => {
                                        app.settings_store.provider_registry =
                                            ProviderRegistry::load();
                                        app.push_toast(
                                            format!("Added: {}", name),
                                            ToastLevel::Info,
                                        );
                                        app.settings_store.add_provider_name.clear();
                                        app.settings_store.add_provider_url.clear();
                                        app.settings_store.add_provider_key.clear();
                                        app.settings_store.show_add_provider = false;
                                        app.settings_store.settings_edit.provider = name.clone();
                                        if let Some(prov) =
                                            app.settings_store.provider_registry.get(&name)
                                        {
                                            if !prov.models.is_empty() {
                                                app.settings_store.settings_edit.model =
                                                    prov.models[0].clone();
                                            }
                                        }
                                    }
                                    Err(e) => app.push_toast(e.to_string(), ToastLevel::Error),
                                }
                            }
                        }
                    }
                }
                if ui.add(theme.secondary_button("Cancel")).clicked() {
                    app.settings_store.show_add_provider = false;
                }
            });
        });
}

/// Map frontend API format string (kebab-case) to core runtime format string (snake_case).
///
/// Frontend `ApiFormat` serializes as:
///   "openai-completions" | "anthropic-messages" | "kimi"
///
/// Core `RuntimeProviderConfig.api_format` expects:
///   "openai_chat" | "anthropic_messages" | "ollama" | "llama_server"
pub(crate) fn map_api_format(frontend: &str) -> &'static str {
    match frontend {
        "openai-completions" | "kimi" => "openai_chat",
        "anthropic-messages" => "anthropic_messages",
        "ollama" => "ollama",
        "llama_server" => "llama_server",
        "deepseek-device" => "deepseek_device",
        _ => "openai_chat",
    }
}
