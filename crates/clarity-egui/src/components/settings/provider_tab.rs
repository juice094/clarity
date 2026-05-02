use crate::provider::{ApiFormat, ProviderDefinition, ProviderRegistry};
use crate::ui::types::{ToastLevel, UiEvent};
use crate::App;

use clarity_core::llm::runtime::{
    list_models, set_provider_config, test_connection, RuntimeProviderConfig,
};

pub fn render_provider(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("Provider")).color(app.ui_store.theme.text).size(app.ui_store.theme.text_lg).strong());
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Connect to an AI service").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text_dim));
    ui.add_space(12.0);

    let all: Vec<ProviderDefinition> = app.settings_store.provider_registry.list().into_iter().cloned().collect();
    let current = app.settings_store.settings_edit.provider.clone();

    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        for p in &all {
            let is_active = p.id == current;
            let id = p.id.clone();
            let h = 48.0;
            let s = egui::Stroke::new(if is_active { 1.5 } else { 1.0 },
                if is_active { app.ui_store.theme.accent } else { app.ui_store.theme.border });

            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
            let bg = if resp.hovered() { app.ui_store.theme.surface_strong } else { app.ui_store.theme.surface };

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                egui::Frame::group(ui.style())
                    .fill(bg)
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                    .stroke(s)
                    .inner_margin(egui::Margin::symmetric(12, 0))
                    .show(ui, |ui| {
                        ui.set_min_height(h);
                        ui.horizontal(|ui| {
                            let has_key = !p.api_key_ref.is_empty();
                            crate::widgets::status_dot(ui, has_key, &app.ui_store.theme);

                            ui.add_space(8.0);

                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(p.display())
                                    .font(egui::FontId::new(app.ui_store.theme.text_base, egui::FontFamily::Proportional))
                                    .color(if is_active { app.ui_store.theme.accent } else { app.ui_store.theme.text }));
                                let url = if p.base_url.len() > 40 { format!("{}...", &p.base_url[..37]) } else { p.base_url.clone() };
                                ui.label(egui::RichText::new(&url)
                                    .font(egui::FontId::new(app.ui_store.theme.text_xs, egui::FontFamily::Monospace))
                                    .color(app.ui_store.theme.text_dim));
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.vertical(|ui| {
                                    crate::widgets::badge(ui, p.api_format.as_str(), &app.ui_store.theme);
                                    if is_active {
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new("Active")
                                                .font(egui::FontId::new(app.ui_store.theme.text_xs, egui::FontFamily::Proportional))
                                                .color(app.ui_store.theme.ok));
                                        });
                                    }
                                });
                            });
                        });
                    });
            });

            if resp.clicked() && !is_active {
                app.settings_store.settings_edit.provider = id.clone();
                if let Some(prov) = app.settings_store.provider_registry.get(&id) {
                    if !prov.models.is_empty() { app.settings_store.settings_edit.model = prov.models[0].clone(); }
                }
                app.auto_save_settings();
            }
            ui.add_space(2.0);
        }
    });

    ui.add_space(12.0);

    // ── Model for active provider ──
    if let Some(prov) = app.settings_store.provider_registry.get(&current) {
        if !prov.models.is_empty() {
            ui.label(egui::RichText::new(app.t("Model")).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text).strong());
            let mut models = prov.models.clone();
            if !models.contains(&app.settings_store.settings_edit.model) { models.push(app.settings_store.settings_edit.model.clone()); }
            let cur = models.iter().position(|m| *m == app.settings_store.settings_edit.model).unwrap_or(0);
            let mut sel = cur;
            egui::ComboBox::from_id_salt("st_model").selected_text(&app.settings_store.settings_edit.model)
                .show_ui(ui, |ui| { for (i, m) in models.iter().enumerate() { ui.selectable_value(&mut sel, i, m.as_str()); }});
            if sel != cur && sel < models.len() { app.settings_store.settings_edit.model = models[sel].clone(); app.auto_save_settings(); }
        }
    }

    ui.add_space(app.ui_store.theme.space_8);

    // ── Approval mode ──
    ui.label(egui::RichText::new(app.t("Approval Mode")).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text).strong());
    let modes = ["interactive","smart","plan","yolo"];
    let cur = modes.iter().position(|m| *m == app.settings_store.settings_edit.approval_mode).unwrap_or(0);
    let mut ms = cur;
    egui::ComboBox::from_id_salt("st_amode").selected_text(&app.settings_store.settings_edit.approval_mode)
        .show_ui(ui, |ui| { for (i, m) in modes.iter().enumerate() { ui.selectable_value(&mut ms, i, *m); }});
    if ms != cur { app.settings_store.settings_edit.approval_mode = modes[ms].to_string(); app.auto_save_settings(); }

    ui.add_space(12.0);

    // ── Action buttons: Test Connection / Refresh Models / Apply ──
    ui.horizontal(|ui| {
        let is_local = current.is_empty()
            || app.settings_store.provider_registry.get(&current)
                .map(|p| p.base_url.is_empty())
                .unwrap_or(true);
        let is_testing = app.settings_store.testing_provider.as_deref() == Some(&current);
        let is_refreshing = app.settings_store.refreshing_provider.as_deref() == Some(&current);

        // Test Connection
        let test_label = if is_testing { "Testing..." } else { "Test Connection" };
        let test_btn = egui::Button::new(egui::RichText::new(test_label).size(app.ui_store.theme.text_sm))
            .fill(app.ui_store.theme.surface)
            .corner_radius(app.ui_store.theme.radius_sm as u8);
        if ui.add_enabled(!is_local && !is_testing, test_btn).clicked() {
            let (display_name, base_url, api_fmt) = app.settings_store.provider_registry.get(&current)
                .map(|p| (p.display().to_string(), p.base_url.clone(), map_api_format(p.api_format.as_str()).to_string()))
                .unwrap_or_default();
            let key = app.settings_store.provider_registry.get(&current)
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
        let refresh_label = if is_refreshing { "Refreshing..." } else { "Refresh Models" };
        let refresh_btn = egui::Button::new(egui::RichText::new(refresh_label).size(app.ui_store.theme.text_sm))
            .fill(app.ui_store.theme.surface)
            .corner_radius(app.ui_store.theme.radius_sm as u8);
        if ui.add_enabled(!is_local && !is_refreshing, refresh_btn).clicked() {
            let (_, base_url, api_fmt) = app.settings_store.provider_registry.get(&current)
                .map(|p| (p.display().to_string(), p.base_url.clone(), map_api_format(p.api_format.as_str()).to_string()))
                .unwrap_or_default();
            let display_name = app.settings_store.provider_registry.get(&current)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let key = app.settings_store.provider_registry.get(&current)
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
        let apply_btn = egui::Button::new(egui::RichText::new("Apply").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text))
            .fill(app.ui_store.theme.accent)
            .corner_radius(app.ui_store.theme.radius_sm as u8);
        if ui.add_enabled(!is_local, apply_btn).clicked() {
            let (display_name, base_url, api_fmt) = app.settings_store.provider_registry.get(&current)
                .map(|p| (p.display().to_string(), p.base_url.clone(), map_api_format(p.api_format.as_str()).to_string()))
                .unwrap_or_default();
            let key = app.settings_store.provider_registry.get(&current)
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

    ui.add_space(12.0);

    // ── Add custom ──
    if ui.add(app.ui_store.theme.primary_button("+ Add Custom")).clicked() { app.settings_store.show_add_provider = !app.settings_store.show_add_provider; }
    if app.settings_store.show_add_provider { ui.add_space(8.0); render_add_form(app, ui); }
}

fn render_add_form(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::new().fill(app.ui_store.theme.bg_accent)
        .corner_radius(app.ui_store.theme.radius_md as u8).stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
        .inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        ui.label(egui::RichText::new("Add Custom Provider").strong().color(app.ui_store.theme.text).size(app.ui_store.theme.text_base));
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Name").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text));
        ui.add(egui::TextEdit::singleline(&mut app.settings_store.add_provider_name).hint_text("my-provider").desired_width(240.0));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Base URL").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text));
        ui.add(egui::TextEdit::singleline(&mut app.settings_store.add_provider_url).hint_text("https://...").desired_width(240.0));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("API Format").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text));
        let fmts = ["openai-completions","anthropic-messages"];
        let mut fi = fmts.iter().position(|f| *f == app.settings_store.add_provider_format).unwrap_or(0);
        egui::ComboBox::from_id_salt("add_fmt").selected_text(app.settings_store.add_provider_format.as_str())
            .show_ui(ui, |ui| { for (i,f) in fmts.iter().enumerate() { ui.selectable_value(&mut fi, i, *f); }});
        if fi < fmts.len() { app.settings_store.add_provider_format = fmts[fi].to_string(); }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("API Key").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text));
        ui.add(egui::TextEdit::singleline(&mut app.settings_store.add_provider_key).hint_text("${env:KEY}").desired_width(240.0));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add(app.ui_store.theme.primary_button("Save")).clicked() {
                let name = app.settings_store.add_provider_name.trim().to_lowercase().replace(' ', "-");
                if !name.is_empty() && !app.settings_store.add_provider_url.trim().is_empty() {
                    let def = ProviderDefinition { id: name.clone(), display_name: app.settings_store.add_provider_name.trim().into(),
                        base_url: app.settings_store.add_provider_url.trim().into(), api_format: ApiFormat::from_str(&app.settings_store.add_provider_format),
                        api_key_ref: app.settings_store.add_provider_key.trim().into(), models: vec![], builtin: false };
                    match app.settings_store.provider_registry.save_custom(&def) {
                        Ok(()) => { app.settings_store.provider_registry = ProviderRegistry::load();
                            app.push_toast(format!("Added: {}", name), ToastLevel::Info);
                            app.settings_store.add_provider_name.clear(); app.settings_store.add_provider_url.clear();
                            app.settings_store.add_provider_key.clear(); app.settings_store.show_add_provider = false; }
                        Err(e) => app.push_toast(e.to_string(), ToastLevel::Error),
                    }
                }
            }
            if ui.add(app.ui_store.theme.secondary_button("Cancel")).clicked() { app.settings_store.show_add_provider = false; }
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
