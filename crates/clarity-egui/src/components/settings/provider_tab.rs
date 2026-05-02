use crate::provider::{ApiFormat, ProviderDefinition, ProviderRegistry};
use crate::ui::types::{ToastLevel, UiEvent};
use crate::App;

use clarity_core::llm::runtime::{
    list_models, set_provider_config, test_connection, RuntimeProviderConfig,
};

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

    let all: Vec<ProviderDefinition> = app
        .settings_store
        .provider_registry
        .list()
        .into_iter()
        .cloned()
        .collect();
    let current = app.settings_store.settings_edit.provider.clone();

    egui::ScrollArea::vertical()
        .min_scrolled_height(200.0)
        .show(ui, |ui| {
            for p in &all {
                let is_active = p.id == current;
                let id = p.id.clone();
                let h = 36.0;

                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), h),
                    egui::Sense::click(),
                );

                let has_key = !p.api_key_ref.is_empty() && p.resolve_api_key().is_some();
                let text_color = if is_active {
                    theme.accent
                } else {
                    theme.text
                };

                let bg = if is_active || resp.hovered() {
                    theme.surface_strong
                } else {
                    theme.surface
                };

                let cr = egui::CornerRadius::same(theme.radius_sm as u8);
                ui.painter().rect_filled(rect, cr, bg);

                if is_active {
                    let bar = egui::Rect::from_min_max(
                        rect.min,
                        egui::pos2(rect.min.x + 2.0, rect.max.y),
                    );
                    ui.painter()
                        .rect_filled(bar, egui::CornerRadius::ZERO, theme.accent);
                }

                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(6.0, 0.0))),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.set_min_height(h);
                            crate::widgets::status_dot(ui, has_key, &theme);
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(p.display())
                                    .font(theme.font(theme.text_base))
                                    .color(text_color),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if !p.models.is_empty() {
                                        ui.label(
                                            egui::RichText::new(format!("{}", p.models.len()))
                                                .font(theme.font(theme.text_xs))
                                                .color(theme.text_muted),
                                        );
                                    }
                                },
                            );
                        });
                    },
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
                    egui::RichText::new(crate::theme::ICON_X)
                        .font(theme.font_icon(theme.text_sm)),
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
    egui::Frame::new()
        .fill(theme.input_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(1.0, theme.border))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut te = egui::TextEdit::singleline(&mut key_buffer)
                    .password(!show_key)
                    .desired_width(ui.available_width() - 50.0)
                    .font(theme.font(theme.text_base));
                if resolved_key.is_empty() {
                    te = te.hint_text("Enter API key...");
                }
                let resp = ui.add(te);
                if resp.changed() {
                    ui.data_mut(|d| d.insert_temp(key_edit_id, key_buffer.clone()));
                    let mut updated = prov.clone();
                    updated.api_key_ref = key_buffer;
                    let _ = app.settings_store.provider_registry.update_provider(&updated);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let eye_text = if show_key { "Hide" } else { "Show" };
                    if ui.add(theme.ghost_button(eye_text)).clicked() {
                        show_key = !show_key;
                        ui.data_mut(|d| d.insert_temp(show_key_id, show_key));
                    }
                });
            });
        });

    ui.add_space(theme.space_8);

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
    egui::Frame::new()
        .fill(theme.input_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(1.0, theme.border))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            let mut te = egui::TextEdit::singleline(&mut url_buffer)
                .desired_width(ui.available_width())
                .font(theme.font(theme.text_base));
            if prov.base_url.is_empty() {
                te = te.hint_text("https://api.example.com/v1");
            }
            let resp = ui.add(te);
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(url_edit_id, url_buffer.clone()));
                let mut updated = prov.clone();
                updated.base_url = url_buffer;
                let _ = app.settings_store.provider_registry.update_provider(&updated);
            }
        });

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
        let is_local = current.is_empty()
            || app
                .settings_store
                .provider_registry
                .get(&current)
                .map(|p| p.base_url.is_empty())
                .unwrap_or(true);
        let is_testing = app.settings_store.testing_provider.as_deref() == Some(&current);
        let is_refreshing = app.settings_store.refreshing_provider.as_deref() == Some(&current);

        // Test Connection
        let test_label = if is_testing { "Testing..." } else { "Test Connection" };
        let test_btn = egui::Button::new(
            egui::RichText::new(test_label).size(theme.text_sm),
        )
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        if ui.add_enabled(!is_local && !is_testing, test_btn).clicked() {
            let (display_name, base_url, api_fmt) = app
                .settings_store
                .provider_registry
                .get(&current)
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        map_api_format(p.api_format.as_str()).to_string(),
                    )
                })
                .unwrap_or_default();
            let key = app
                .settings_store
                .provider_registry
                .get(&current)
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = app.settings_store.settings_edit.model.clone();
            if key.is_empty() {
                app.push_toast(
                    format!("{}: No API key configured", display_name),
                    ToastLevel::Warn,
                );
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
        let refresh_btn = egui::Button::new(
            egui::RichText::new(refresh_label).size(theme.text_sm),
        )
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        if ui.add_enabled(!is_local && !is_refreshing, refresh_btn).clicked() {
            let (_, base_url, api_fmt) = app
                .settings_store
                .provider_registry
                .get(&current)
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        map_api_format(p.api_format.as_str()).to_string(),
                    )
                })
                .unwrap_or_default();
            let display_name = app
                .settings_store
                .provider_registry
                .get(&current)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let key = app
                .settings_store
                .provider_registry
                .get(&current)
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = app.settings_store.settings_edit.model.clone();
            if key.is_empty() {
                app.push_toast(
                    format!("{}: No API key configured", display_name),
                    ToastLevel::Warn,
                );
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
            let (display_name, base_url, api_fmt) = app
                .settings_store
                .provider_registry
                .get(&current)
                .map(|p| {
                    (
                        p.display().to_string(),
                        p.base_url.clone(),
                        map_api_format(p.api_format.as_str()).to_string(),
                    )
                })
                .unwrap_or_default();
            let key = app
                .settings_store
                .provider_registry
                .get(&current)
                .and_then(|p| p.resolve_api_key())
                .unwrap_or_default();
            let model = app.settings_store.settings_edit.model.clone();
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
        .stroke(egui::Stroke::new(1.0, theme.border))
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
                            api_key_ref: app.settings_store.add_provider_key.trim().into(),
                            models: vec![],
                            builtin: false,
                        };
                        match app.settings_store.provider_registry.save_custom(&def) {
                            Ok(()) => {
                                app.settings_store.provider_registry = ProviderRegistry::load();
                                app.push_toast(format!("Added: {}", name), ToastLevel::Info);
                                app.settings_store.add_provider_name.clear();
                                app.settings_store.add_provider_url.clear();
                                app.settings_store.add_provider_key.clear();
                                app.settings_store.show_add_provider = false;
                                app.settings_store.settings_edit.provider = name.clone();
                                if let Some(prov) = app.settings_store.provider_registry.get(&name) {
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
        _ => "openai_chat",
    }
}
