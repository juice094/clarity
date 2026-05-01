use crate::provider::{ApiFormat, ProviderDefinition, ProviderRegistry};
use crate::ui::types::ToastLevel;
use crate::App;

/// Tabs in the settings panel.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SettingsTab {
    Provider,
    Interface,
    About,
}

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }

    // Dimmer overlay — click outside to close
    let screen_rect = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen_rect,
        egui::CornerRadius::same(0),
        app.theme.overlay,
    );

    let tabs = [
        (SettingsTab::Provider, app.t("Provider")),
        (SettingsTab::Interface, app.t("Interface")),
        (SettingsTab::About, app.t("About")),
    ];
    let mut active_tab = app.settings_active_tab;

    egui::Window::new(app.t("Settings"))
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(540.0, 460.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_lg as u8))
                .inner_margin(egui::Margin::symmetric(0, 0)),
        )
        .show(ctx, |ui| {
            // ── Tab bar ──
            egui::Frame::new()
                .fill(app.theme.bg_accent)
                .inner_margin(egui::Margin::symmetric(8, 0))
                .corner_radius(egui::CornerRadius {
                    nw: app.theme.radius_lg as u8,
                    ne: app.theme.radius_lg as u8,
                    sw: 0, se: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(36.0);
                        for (i, (_tab, name)) in tabs.iter().enumerate() {
                            let is_active = i as u8 == active_tab;
                            let bg = if is_active { app.theme.surface } else { egui::Color32::TRANSPARENT };
                            let text_color = if is_active { app.theme.text } else { app.theme.text_muted };
                            if ui.add(
                                egui::Button::new(egui::RichText::new(*name).size(13.0).color(text_color))
                                    .fill(bg)
                                    .corner_radius(app.theme.radius_sm as u8)
                                    .min_size(egui::vec2(80.0, 28.0))
                            ).clicked() {
                                active_tab = i as u8;
                            }
                        }
                    });
                });

            ui.add_space(app.theme.space_4);

            // ── Content area ──
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    match tabs[active_tab as usize].0 {
                        SettingsTab::Provider => render_provider_tab(app, ui),
                        SettingsTab::Interface => render_interface_tab(app, ui),
                        SettingsTab::About => render_about_tab(app, ui),
                    }
                });
        });

    app.settings_active_tab = active_tab;
}

// ============================================================================
// Tab: Provider — full provider management (merged former General + Provider)
// ============================================================================

fn render_provider_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("Provider")).color(app.theme.text).size(15.0).strong());
    ui.add_space(app.theme.space_4);
    ui.label(egui::RichText::new(app.t("Select and configure your AI providers."))
        .size(11.0).color(app.theme.text_dim));
    ui.add_space(app.theme.space_12);

    let all: Vec<ProviderDefinition> = app.provider_registry.list().into_iter().cloned().collect();

    // ── Active provider pill ──
    let active_id = &app.settings_edit.provider.clone();
    if let Some(active) = app.provider_registry.get(active_id) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(app.t("Active:")).size(12.0).color(app.theme.text_dim));
            ui.label(egui::RichText::new(active.display()).size(13.0).color(app.theme.accent).strong());
            ui.label(egui::RichText::new(active.api_format.as_str()).size(10.0).color(app.theme.text_dim).monospace());
        });
    }
    ui.add_space(app.theme.space_8);
    ui.separator();
    ui.add_space(app.theme.space_8);

    // ── Provider list (scrollable) ──
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            for p in &all {
                let is_active = p.id == app.settings_edit.provider;
                let id = p.id.clone();

                let stroke = if is_active {
                    egui::Stroke::new(1.5, app.theme.accent)
                } else {
                    egui::Stroke::new(1.0, app.theme.border)
                };

                let (_, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 40.0),
                    egui::Sense::click(),
                );

                // Background
                let bg = if resp.hovered() || is_active {
                    app.theme.bg_hover
                } else {
                    app.theme.bg_elevated
                };
                ui.painter().rect_filled(resp.rect, egui::CornerRadius::same(app.theme.radius_sm as u8), bg);
                ui.painter().rect_stroke(resp.rect, egui::CornerRadius::same(app.theme.radius_sm as u8), stroke, egui::StrokeKind::Outside);

                // Label
                let text_pos = egui::pos2(resp.rect.left() + 12.0, resp.rect.center().y - 6.0);
                ui.painter().text(
                    egui::pos2(text_pos.x, text_pos.y),
                    egui::Align2::LEFT_TOP,
                    p.display(),
                    egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    if is_active { app.theme.accent } else { app.theme.text },
                );
                ui.painter().text(
                    egui::pos2(text_pos.x, text_pos.y + 16.0),
                    egui::Align2::LEFT_TOP,
                    &p.base_url,
                    egui::FontId::new(10.0, egui::FontFamily::Monospace),
                    app.theme.text_dim,
                );

                // Active indicator
                if is_active {
                    ui.painter().text(
                        egui::pos2(resp.rect.right() - 10.0, resp.rect.center().y - 6.0),
                        egui::Align2::RIGHT_TOP,
                        app.t("Active"),
                        egui::FontId::new(10.0, egui::FontFamily::Proportional),
                        app.theme.ok,
                    );
                }

                if resp.clicked() && !is_active {
                    app.settings_edit.provider = id.clone();
                    // Auto-select first model
                    if let Some(prov) = app.provider_registry.get(&id) {
                        if !prov.models.is_empty() {
                            app.settings_edit.model = prov.models[0].clone();
                        }
                    }
                    app.auto_save_settings();
                }

                ui.add_space(4.0);
            }
        });

    ui.add_space(app.theme.space_8);

    // ── Model selection for active provider ──
    ui.label(egui::RichText::new(app.t("Model")).size(12.0).color(app.theme.text).strong());
    let models: Vec<String> = app.provider_registry
        .get(&app.settings_edit.provider)
        .map(|p| {
            let mut m = p.models.clone();
            if !m.contains(&app.settings_edit.model) {
                m.push(app.settings_edit.model.clone());
            }
            m
        })
        .unwrap_or_else(|| vec![app.settings_edit.model.clone()]);
    let current_idx = models.iter().position(|m| *m == app.settings_edit.model).unwrap_or(0);
    let mut sel = current_idx;
    egui::ComboBox::from_id_salt("settings_model")
        .selected_text(&app.settings_edit.model)
        .show_ui(ui, |ui| {
            for (i, m) in models.iter().enumerate() {
                ui.selectable_value(&mut sel, i, m.as_str());
            }
        });
    if sel != current_idx && sel < models.len() {
        app.settings_edit.model = models[sel].clone();
        app.auto_save_settings();
    }

    ui.add_space(app.theme.space_8);

    // ── Approval mode ──
    ui.label(egui::RichText::new(app.t("Approval Mode")).size(12.0).color(app.theme.text).strong());
    let modes = ["interactive", "smart", "plan", "yolo"];
    let cur_mode = modes.iter().position(|m| *m == app.settings_edit.approval_mode).unwrap_or(0);
    let mut mode_sel = cur_mode;
    egui::ComboBox::from_id_salt("settings_approval_mode")
        .selected_text(&app.settings_edit.approval_mode)
        .show_ui(ui, |ui| {
            for (i, m) in modes.iter().enumerate() {
                ui.selectable_value(&mut mode_sel, i, *m);
            }
        });
    if mode_sel != cur_mode {
        app.settings_edit.approval_mode = modes[mode_sel].to_string();
        app.auto_save_settings();
    }

    ui.add_space(app.theme.space_12);

    // ── Add custom provider button ──
    if ui.add(app.theme.primary_button("+ Add")).clicked() {
        app.show_add_provider = !app.show_add_provider;
    }

    if app.show_add_provider {
        render_add_provider_form(app, ui);
    }
}

fn render_add_provider_form(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(app.theme.bg_accent)
        .corner_radius(app.theme.radius_md as u8)
        .stroke(egui::Stroke::new(1.0, app.theme.border))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(app.t("Add Custom Provider")).strong().color(app.theme.text).size(13.0));
            ui.add_space(app.theme.space_8);

            ui.label(egui::RichText::new(app.t("Name")).size(11.0).color(app.theme.text));
            ui.add(egui::TextEdit::singleline(&mut app.add_provider_name).hint_text("my-provider").desired_width(240.0));
            ui.add_space(app.theme.space_4);
            ui.label(egui::RichText::new("Base URL").size(11.0).color(app.theme.text));
            ui.add(egui::TextEdit::singleline(&mut app.add_provider_url).hint_text("https://api.example.com/v1").desired_width(240.0));
            ui.add_space(app.theme.space_4);
            ui.label(egui::RichText::new("API Format").size(11.0).color(app.theme.text));
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
            ui.add_space(app.theme.space_4);
            ui.label(egui::RichText::new(app.t("API Key")).size(11.0).color(app.theme.text));
            ui.add(egui::TextEdit::singleline(&mut app.add_provider_key)
                .hint_text("${env:KEY} or literal")
                .desired_width(240.0));

            ui.add_space(app.theme.space_8);
            if ui.add(app.theme.primary_button(app.t("Save"))).clicked() {
                let name = app.add_provider_name.trim().to_lowercase().replace(' ', "-");
                if !name.is_empty() && !app.add_provider_url.trim().is_empty() {
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
                            app.provider_registry = ProviderRegistry::load();
                            app.push_toast(format!("Added: {}", name), ToastLevel::Info);
                            app.add_provider_name.clear();
                            app.add_provider_url.clear();
                            app.add_provider_key.clear();
                            app.show_add_provider = false;
                        }
                        Err(e) => app.push_toast(format!("Error: {}", e), ToastLevel::Error),
                    }
                }
            }
            if ui.add(app.theme.secondary_button(app.t("Cancel"))).clicked() {
                app.show_add_provider = false;
            }
        });
}

// ============================================================================
// Tab: Interface
// ============================================================================

fn render_interface_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("Interface")).color(app.theme.text).size(15.0).strong());
    ui.add_space(app.theme.space_16);

    ui.label(egui::RichText::new(app.t("Theme")).size(12.0).color(app.theme.text).strong());
    let themes = ["dark", "light"];
    let cur_theme = themes.iter().position(|t| *t == app.settings_edit.theme).unwrap_or(0);
    let mut theme_sel = cur_theme;
    egui::ComboBox::from_id_salt("settings_theme")
        .selected_text(&app.settings_edit.theme)
        .show_ui(ui, |ui| {
            for (i, t) in themes.iter().enumerate() {
                ui.selectable_value(&mut theme_sel, i, *t);
            }
        });
    if theme_sel != cur_theme {
        app.settings_edit.theme = themes[theme_sel].to_string();
        app.theme = match app.settings_edit.theme.as_str() {
            "light" => crate::theme::Theme::light(),
            _ => crate::theme::Theme::dark(),
        };
        app.auto_save_settings();
    }

    ui.add_space(app.theme.space_12);
    ui.label(egui::RichText::new(app.t("Language")).size(12.0).color(app.theme.text).strong());
    ui.horizontal(|ui| {
        let en_active = matches!(app.locale, crate::i18n::Locale::EnUS);
        let zh_active = matches!(app.locale, crate::i18n::Locale::ZhCN);
        if ui.add(
            egui::Button::new(egui::RichText::new("English").size(12.0))
                .fill(if en_active { app.theme.accent } else { app.theme.surface })
                .corner_radius(app.theme.radius_sm as u8)
        ).clicked() {
            app.locale = crate::i18n::Locale::EnUS;
        }
        if ui.add(
            egui::Button::new(egui::RichText::new("简体中文").size(12.0))
                .fill(if zh_active { app.theme.accent } else { app.theme.surface })
                .corner_radius(app.theme.radius_sm as u8)
        ).clicked() {
            app.locale = crate::i18n::Locale::ZhCN;
        }
    });
}

// ============================================================================
// Tab: About
// ============================================================================

fn render_about_tab(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("About")).color(app.theme.text).size(15.0).strong());
    ui.add_space(app.theme.space_16);
    ui.label(egui::RichText::new("Clarity").size(22.0).strong().color(app.theme.text));
    ui.label(egui::RichText::new("Local-first AI agent runtime").size(13.0).color(app.theme.text_muted));
    ui.add_space(app.theme.space_8);
    ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(12.0).color(app.theme.text_dim));
    ui.label(egui::RichText::new("egui 0.31 · glow backend").size(11.0).color(app.theme.text_dim));
}
