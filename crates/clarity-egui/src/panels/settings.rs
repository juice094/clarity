use crate::{app_state::reload_llm, App};

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open { return; }

    // Non-interactive dimmer overlay — paint only, no event capture
    let screen_rect = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background())
        .rect_filled(screen_rect, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(100));

    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_pos(ctx.screen_rect().center())
        .frame(egui::Frame::window(&ctx.style()).fill(app.theme.surface).corner_radius(egui::CornerRadius::same(app.theme.radius_lg as u8)).inner_margin(egui::Margin::same(20)))
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.add_space(4.0);

            // Provider
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Provider").size(13.0).color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("provider_combo")
                        .selected_text(&app.settings_edit.provider)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (key, label, models) in crate::settings::get_available_models() {
                                if ui.selectable_value(&mut app.settings_edit.provider, key.clone(), label).changed() {
                                    // Auto-select first model when provider changes
                                    if let Some(first) = models.first() {
                                        app.settings_edit.model = first.clone();
                                    }
                                }
                            }
                        });
                });
            });
            ui.add_space(8.0);

            // Model — ComboBox populated from get_available_models for the current provider
            let available_models = crate::settings::get_available_models();
            let current_models = available_models
                .iter()
                .find(|(k, _, _)| k == &app.settings_edit.provider)
                .map(|(_, _, m)| m.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Model").size(13.0).color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("model_combo")
                        .selected_text(&app.settings_edit.model)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for m in &current_models {
                                ui.selectable_value(&mut app.settings_edit.model, m.clone(), m);
                            }
                        });
                });
            });
            ui.add_space(8.0);

            // API Key
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("API Key").size(13.0).color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut key = app.settings_edit.api_key.clone().unwrap_or_default();
                    let response = ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut key).password(true).text_color(app.theme.text));
                    if response.changed() {
                        app.settings_edit.api_key = if key.is_empty() { None } else { Some(key) };
                    }
                });
            });
            ui.add_space(8.0);

            // Local model path
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Local Model Path").size(13.0).color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut path = app.settings_edit.local_model_path.clone().unwrap_or_default();
                    let response = ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut path).text_color(app.theme.text));
                    if response.changed() {
                        app.settings_edit.local_model_path = if path.is_empty() { None } else { Some(path) };
                    }
                });
            });
            ui.add_space(8.0);

            // Approval mode
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Approval Mode").size(13.0).color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("approval_combo")
                        .selected_text(&app.settings_edit.approval_mode)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.settings_edit.approval_mode, "interactive".into(), "Interactive — Approve each tool call");
                            ui.selectable_value(&mut app.settings_edit.approval_mode, "yolo".into(), "Yolo — Auto-approve all");
                            ui.selectable_value(&mut app.settings_edit.approval_mode, "plan".into(), "Plan — Review plan before execution");
                        });
                });
            });
            ui.add_space(16.0);

            // Buttons
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Save").size(13.0).color(app.theme.text)).fill(app.theme.accent).min_size(egui::vec2(80.0, 32.0))).clicked() {
                        if let Err(e) = app.settings_edit.save() {
                            tracing::error!("Failed to save settings: {}", e);
                        } else {
                            {
                                let mut guard = app.state.cached_settings.lock();
                                *guard = app.settings_edit.clone();
                            }
                            // Sync approval mode to the running agent.
                            let mode = crate::app_state::parse_approval_mode(&app.settings_edit.approval_mode);
                            app.state.agent.set_approval_mode(mode);
                            let state = app.state.clone();
                            app.runtime.spawn(async move {
                                if let Err(e) = reload_llm(&state).await {
                                    tracing::warn!("reload_llm failed: {}", e);
                                }
                            });
                        }
                        app.settings_open = false;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("Cancel").size(13.0).color(app.theme.text)).fill(app.theme.border).min_size(egui::vec2(80.0, 32.0))).clicked() {
                        app.settings_open = false;
                    }
                });
            });
        });
}
