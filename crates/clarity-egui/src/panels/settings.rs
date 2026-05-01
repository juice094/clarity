use crate::ui::types::ToastLevel;
use crate::App;

/// Tabs available in the settings panel.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SettingsTab {
    General,
    Provider,
    Interface,
    About,
}

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }

    // ── Track active tab ──
    // Stored as a u8 in App to avoid lifetime issues.
    let tabs = [
        (SettingsTab::General, app.t("General")),
        (SettingsTab::Provider, app.t("Provider")),
        (SettingsTab::Interface, app.t("Interface")),
        (SettingsTab::About, app.t("About")),
    ];
    let mut active_tab = app.settings_active_tab;

    // Non-interactive dimmer overlay
    let screen_rect = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen_rect,
        egui::CornerRadius::ZERO,
        app.theme.overlay,
    );

    let tab_names: Vec<&str> = tabs.iter().map(|(_, name)| *name).collect();

    egui::Window::new(app.t("Settings"))
        .collapsible(false)
        .resizable(true)
        .min_width(520.0)
        .default_width(560.0)
        .default_height(480.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(0)), // no margin; handled inside
        )
        .show(ctx, |ui| {
            // ── Tab bar ──
            let tab_height = 36.0;
            egui::Frame::new()
                .fill(app.theme.bg_accent)
                .corner_radius(egui::CornerRadius {
                    nw: app.theme.radius_lg as u8,
                    ne: app.theme.radius_lg as u8,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(egui::Margin::symmetric(8, 0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(tab_height);
                        for (i, (tab, name)) in tabs.iter().enumerate() {
                            let is_active = i as u8 == active_tab;
                            let bg = if is_active {
                                app.theme.surface
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let text_color = if is_active {
                                app.theme.text
                            } else {
                                app.theme.text_muted
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(*name)
                                            .size(13.0)
                                            .color(text_color),
                                    )
                                    .fill(bg)
                                    .corner_radius(app.theme.radius_sm as u8)
                                    .min_size(egui::vec2(80.0, 28.0)),
                                )
                                .clicked()
                            {
                                active_tab = i as u8;
                            }
                        }
                    });
                });

            ui.add_space(app.theme.space_4);

            // ── Tab content area ──
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(20, 16))
                .show(ui, |ui| {
                    match tabs[active_tab as usize].0 {
                        SettingsTab::General => render_general_tab(app, ui, ctx),
                        SettingsTab::Provider => render_provider_tab(app, ui),
                        SettingsTab::Interface => render_interface_tab(app, ui),
                        SettingsTab::About => render_about_tab(app, ui),
                    }
                });
        });

    app.settings_active_tab = active_tab;
}

// ============================================================================
// Tab: General
// ============================================================================

fn render_general_tab(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading(egui::RichText::new(app.t("General")).color(app.theme.text).size(16.0));
    ui.add_space(app.theme.space_16);

    // ── Provider selection ──
    ui.label(egui::RichText::new(app.t("Provider")).strong().color(app.theme.text).size(13.0));
    let all_providers = app.provider_registry.list();
    let provider_ids: Vec<&str> = all_providers.iter().map(|p| p.id.as_str()).collect();
    let current_idx = provider_ids.iter().position(|id| *id == app.settings_edit.provider).unwrap_or(0);
    let mut selected = current_idx;
    egui::ComboBox::from_id_salt("settings_provider")
        .selected_text(provider_ids.get(current_idx).copied().unwrap_or(""))
        .show_ui(ui, |ui| {
            for (i, pid) in provider_ids.iter().enumerate() {
                ui.selectable_value(&mut selected, i, *pid);
            }
        });
    if selected != current_idx {
        app.settings_edit.provider = provider_ids[selected].to_string();
        // Auto-select first model for this provider
        if let Some(prov) = app.provider_registry.get(&app.settings_edit.provider) {
            if !prov.models.is_empty() {
                app.settings_edit.model = prov.models[0].clone();
            }
        }
        app.save_settings_and_reload();
    }

    ui.add_space(app.theme.space_12);

    // ── Model selection ──
    ui.label(egui::RichText::new(app.t("Model")).strong().color(app.theme.text).size(13.0));
    let models: Vec<String> = app.provider_registry
        .get(&app.settings_edit.provider)
        .map(|p| {
            let mut m = p.models.clone();
            // Always allow custom model input by keeping the current value as an option
            if !m.contains(&app.settings_edit.model) {
                m.push(app.settings_edit.model.clone());
            }
            m
        })
        .unwrap_or_else(|| vec![app.settings_edit.model.clone()]);
    let current_model_idx = models.iter().position(|m| *m == app.settings_edit.model).unwrap_or(0);
    let mut model_sel = current_model_idx;
    egui::ComboBox::from_id_salt("settings_model")
        .selected_text(&app.settings_edit.model)
        .show_ui(ui, |ui| {
            for (i, m) in models.iter().enumerate() {
                ui.selectable_value(&mut model_sel, i, m.as_str());
            }
        });
    if model_sel != current_model_idx && model_sel < models.len() {
        app.settings_edit.model = models[model_sel].clone();
    }

    ui.add_space(app.theme.space_12);

    // ── API Key ──
    ui.label(egui::RichText::new(app.t("API Key")).strong().color(app.theme.text).size(13.0));
    let provider = app.provider_registry.get(&app.settings_edit.provider);
    let key_hint = provider.map(|p| p.api_key_ref.as_str()).unwrap_or("");
    ui.horizontal(|ui| {
        let mut key_display = app.settings_edit.api_key.clone().unwrap_or_default();
        let resp = ui.add_sized(
            egui::vec2(280.0, 28.0),
            egui::TextEdit::singleline(&mut key_display)
                .hint_text(key_hint)
                .password(true),
        );
        if resp.changed() {
            let trimmed = key_display.trim().to_string();
            if trimmed.is_empty() {
                app.settings_edit.api_key = None;
            } else {
                app.settings_edit.api_key = Some(trimmed);
            }
        }
        ui.label(
            egui::RichText::new(format!("({})", key_hint))
                .size(11.0)
                .color(app.theme.text_dim),
        );
    });

    ui.add_space(app.theme.space_12);

    // ── Approval mode ──
    ui.label(egui::RichText::new(app.t("Approval Mode")).strong().color(app.theme.text).size(13.0));
    let modes = ["interactive", "smart", "plan", "yolo"];
    let current_mode = modes.iter().position(|m| *m == app.settings_edit.approval_mode).unwrap_or(0);
    let mut mode_sel = current_mode;
    egui::ComboBox::from_id_salt("settings_approval_mode")
        .selected_text(&app.settings_edit.approval_mode)
        .show_ui(ui, |ui| {
            for (i, m) in modes.iter().enumerate() {
                ui.selectable_value(&mut mode_sel, i, *m);
            }
        });
    if mode_sel != current_mode {
        app.settings_edit.approval_mode = modes[mode_sel].to_string();
    }

    ui.add_space(app.theme.space_20);

    // ── Action buttons ──
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(app.theme.primary_button(app.t("Save")))
                .clicked()
            {
                app.save_settings_and_reload();
                app.settings_open = false;
            }
            ui.add_space(app.theme.space_8);
            if ui
                .add(app.theme.secondary_button(app.t("Cancel")))
                .clicked()
            {
                app.settings_open = false;
            }
        });
    });
}

// ============================================================================
// Tab: Provider (manage custom providers)
// ============================================================================

fn render_provider_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new(app.t("Provider")).color(app.theme.text).size(16.0));
    ui.add_space(app.theme.space_8);

    // ── Built-in providers (read-only list) ──
    ui.label(egui::RichText::new(app.t("Built-in")).strong().color(app.theme.text_muted).size(12.0));
    ui.add_space(app.theme.space_4);
    for p in app.provider_registry.list().iter().filter(|p| p.builtin) {
        egui::Frame::new()
            .fill(app.theme.bg_elevated)
            .corner_radius(app.theme.radius_sm as u8)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(p.display()).size(12.0).color(app.theme.text));
                    ui.label(egui::RichText::new(p.api_format.as_str()).size(10.0).color(app.theme.text_dim).monospace());
                    if !p.models.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("{} models", p.models.len()))
                                .size(10.0).color(app.theme.text_dim));
                    }
                });
            });
        ui.add_space(app.theme.space_4);
    }

    ui.add_space(app.theme.space_12);

    // ── Custom providers ──
    ui.label(egui::RichText::new(app.t("Custom")).strong().color(app.theme.text_muted).size(12.0));
    ui.add_space(app.theme.space_4);
    let custom_ids: Vec<(String, String)> = app.provider_registry.list_custom()
        .iter()
        .map(|p| (p.id.clone(), p.display().to_string()))
        .collect();
    if custom_ids.is_empty() {
        ui.label(egui::RichText::new(app.t("No custom providers configured."))
            .size(12.0).color(app.theme.text_dim));
    } else {
        for (id, display_name) in &custom_ids {
            let id_c = id.clone();
            egui::Frame::new()
                .fill(app.theme.bg_elevated)
                .corner_radius(app.theme.radius_sm as u8)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(display_name.as_str()).size(12.0).color(app.theme.text));
                        if ui.button(app.t("Delete")).clicked() {
                            let _ = app.provider_registry.delete_custom(&id_c);
                            app.push_toast(
                                format!("Deleted provider: {}", id_c),
                                ToastLevel::Info,
                            );
                            // Reload registry after deletion
                            app.provider_registry = crate::provider::ProviderRegistry::load();
                        }
                    });
                });
            ui.add_space(app.theme.space_4);
        }
    }

    ui.add_space(app.theme.space_12);

    // ── Add custom provider ──
    if ui.button(app.t("+ Add Custom Provider")).clicked() {
        app.show_add_provider = true;
    }

    // ── Add provider overlay (inline) ──
    if app.show_add_provider {
        render_add_provider_form(app, ui);
    }
}

fn render_add_provider_form(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(app.theme.bg_accent)
        .corner_radius(app.theme.radius_md as u8)
        .stroke(egui::Stroke::new(1.0, app.theme.border))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(app.t("Add Custom Provider")).strong().color(app.theme.text).size(14.0));
            ui.add_space(app.theme.space_8);

            // Name
            ui.label(egui::RichText::new(app.t("Name")).size(12.0).color(app.theme.text));
            ui.add(
                egui::TextEdit::singleline(&mut app.add_provider_name)
                    .hint_text("my-provider"),
            );
            ui.add_space(app.theme.space_8);

            // Base URL
            ui.label(egui::RichText::new("Base URL").size(12.0).color(app.theme.text));
            ui.add(
                egui::TextEdit::singleline(&mut app.add_provider_url)
                    .hint_text("https://api.example.com/v1"),
            );
            ui.add_space(app.theme.space_8);

            // API format
            ui.label(egui::RichText::new("API Format").size(12.0).color(app.theme.text));
            let formats = ["openai-completions", "anthropic-messages", "kimi"];
            let mut fmt_idx = formats.iter().position(|f| *f == app.add_provider_format).unwrap_or(0);
            egui::ComboBox::from_id_salt("add_provider_format")
                .selected_text(app.add_provider_format.as_str())
                .show_ui(ui, |ui| {
                    for (i, f) in formats.iter().enumerate() {
                        ui.selectable_value(&mut fmt_idx, i, *f);
                    }
                });
            if fmt_idx < formats.len() {
                app.add_provider_format = formats[fmt_idx].to_string();
            }
            ui.add_space(app.theme.space_8);

            // API key
            ui.label(egui::RichText::new(app.t("API Key")).size(12.0).color(app.theme.text));
            ui.add(
                egui::TextEdit::singleline(&mut app.add_provider_key)
                    .hint_text("${env:MY_KEY} or literal key"),
            );
            ui.add_space(app.theme.space_12);

            // Buttons
            ui.horizontal(|ui| {
                if ui.add(app.theme.primary_button(app.t("Save"))).clicked() {
                    let name = app.add_provider_name.trim().to_lowercase().replace(' ', "-");
                    if !name.is_empty() && !app.add_provider_url.trim().is_empty() {
                        use crate::provider::{ApiFormat, ProviderDefinition};
                        let def = ProviderDefinition {
                            id: name.clone(),
                            display_name: app.add_provider_name.trim().to_string(),
                            base_url: app.add_provider_url.trim().to_string(),
                            api_format: ApiFormat::from_str(&app.add_provider_format),
                            api_key_ref: app.add_provider_key.trim().to_string(),
                            models: vec![],
                            builtin: false,
                        };
                        match app.provider_registry.save_custom(&def) {
                            Ok(()) => {
                                // Reload registry
                                app.provider_registry = crate::provider::ProviderRegistry::load();
                                app.push_toast(
                                    format!("Added provider: {}", name),
                                    ToastLevel::Info,
                                );
                                app.add_provider_name.clear();
                                app.add_provider_url.clear();
                                app.add_provider_key.clear();
                                app.show_add_provider = false;
                            }
                            Err(e) => {
                                app.push_toast(format!("Failed to save: {}", e), ToastLevel::Error);
                            }
                        }
                    }
                }
                if ui.add(app.theme.secondary_button(app.t("Cancel"))).clicked() {
                    app.show_add_provider = false;
                }
            });
        });
}

// ============================================================================
// Tab: Interface
// ============================================================================

fn render_interface_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new(app.t("Interface")).color(app.theme.text).size(16.0));
    ui.add_space(app.theme.space_16);

    // Theme toggle
    ui.label(egui::RichText::new(app.t("Theme")).strong().color(app.theme.text).size(13.0));
    let themes = ["dark", "light"];
    let current_theme = themes.iter().position(|t| *t == app.settings_edit.theme).unwrap_or(0);
    let mut theme_sel = current_theme;
    egui::ComboBox::from_id_salt("settings_theme")
        .selected_text(&app.settings_edit.theme)
        .show_ui(ui, |ui| {
            for (i, t) in themes.iter().enumerate() {
                ui.selectable_value(&mut theme_sel, i, *t);
            }
        });
    if theme_sel != current_theme {
        app.settings_edit.theme = themes[theme_sel].to_string();
        app.theme = match app.settings_edit.theme.as_str() {
            "light" => crate::theme::Theme::light(),
            _ => crate::theme::Theme::dark(),
        };
        app.save_settings_internal();
    }

    ui.add_space(app.theme.space_12);

    // Locale toggle
    ui.label(egui::RichText::new(app.t("Language")).strong().color(app.theme.text).size(13.0));
    ui.horizontal(|ui| {
        let en_active = matches!(app.locale, crate::i18n::Locale::EnUS);
        let zh_active = matches!(app.locale, crate::i18n::Locale::ZhCN);
        if ui
            .add(
                egui::Button::new(egui::RichText::new("English").size(12.0))
                    .fill(if en_active { app.theme.accent } else { app.theme.surface })
                    .corner_radius(app.theme.radius_sm as u8),
            )
            .clicked()
        {
            app.locale = crate::i18n::Locale::EnUS;
        }
        ui.add_space(app.theme.space_4);
        if ui
            .add(
                egui::Button::new(egui::RichText::new("简体中文").size(12.0))
                    .fill(if zh_active { app.theme.accent } else { app.theme.surface })
                    .corner_radius(app.theme.radius_sm as u8),
            )
            .clicked()
        {
            app.locale = crate::i18n::Locale::ZhCN;
        }
    });

    ui.add_space(app.theme.space_12);

    // Clear batch grants
    if ui.button(app.t("Clear Batch Grants")).clicked() {
        app.state.mode_aware_approval_runtime.clear_batch_grants();
        app.push_toast("Batch grants cleared", ToastLevel::Info);
    }

    ui.add_space(app.theme.space_20);

    // Save button
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(app.theme.primary_button(app.t("Save"))).clicked() {
                app.save_settings_internal();
                app.settings_open = false;
            }
            ui.add_space(app.theme.space_8);
            if ui.add(app.theme.secondary_button(app.t("Cancel"))).clicked() {
                app.settings_open = false;
            }
        });
    });
}

// ============================================================================
// Tab: About
// ============================================================================

fn render_about_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.heading(egui::RichText::new(app.t("About")).color(app.theme.text).size(16.0));
    ui.add_space(app.theme.space_16);
    ui.label(egui::RichText::new("Clarity").size(24.0).strong().color(app.theme.text));
    ui.label(egui::RichText::new("Local-first AI agent runtime").size(13.0).color(app.theme.text_muted));
    ui.add_space(app.theme.space_12);
    ui.label(egui::RichText::new(format!("Version: {}", env!("CARGO_PKG_VERSION"))).size(12.0).color(app.theme.text_dim));
    ui.label(egui::RichText::new("egui 0.31 + glow backend").size(12.0).color(app.theme.text_dim));
    ui.add_space(app.theme.space_24);
    ui.label(egui::RichText::new("https://github.com/juice094/clarity").size(11.0).color(app.theme.accent));
}

/// Helper: extract settings tab index from App state.
pub fn settings_tab_index(app: &App) -> u8 {
    app.settings_active_tab
}
