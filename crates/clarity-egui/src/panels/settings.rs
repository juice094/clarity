use crate::{app_state::reload_llm, App};
use clarity_wire::UserAction;

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }

    // Non-interactive dimmer overlay — paint only, no event capture
    let screen_rect = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background())
        .rect_filled(screen_rect, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(100));

    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_pos(ctx.screen_rect().center())
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(20)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.add_space(4.0);

            let commands = app.settings_vm.commands();
            let mut actions = Vec::new();

            ui.vertical(|ui| {
                crate::ui::protocol_renderer::render_view_commands(ui, &commands, &app.theme, &mut actions);
            });

            for action in actions {
                app.settings_vm.handle_action(action.clone());
                match action {
                    UserAction::ButtonClick { id } if id == "save" => {
                        let snapshot = app.settings_vm.snapshot();
                        app.settings_edit.active_profile = snapshot.active_profile.clone();

                        // If an active profile is selected, overlay its fields onto settings
                        if let Some(ref profile_id) = snapshot.active_profile {
                            if let Some(profile) = app.settings_edit.profiles.get(profile_id) {
                                app.settings_edit.provider = profile.provider.clone();
                                app.settings_edit.model = profile.model.clone();
                                app.settings_edit.approval_mode = profile.approval_mode.clone();
                                if profile.api_key.is_some() {
                                    app.settings_edit.api_key = profile.api_key.clone();
                                }
                                if profile.local_model_path.is_some() {
                                    app.settings_edit.local_model_path = profile.local_model_path.clone();
                                }
                            }
                        } else {
                            // No profile selected: use the directly edited values
                            app.settings_edit.provider = snapshot.provider;
                            app.settings_edit.model = snapshot.model;
                            app.settings_edit.approval_mode = snapshot.approval_mode;
                            app.settings_edit.api_key = snapshot.api_key;
                            app.settings_edit.local_model_path = snapshot.local_model_path;
                        }
                        app.settings_edit.theme = snapshot.theme;

                        if let Err(e) = app.settings_edit.save() {
                            tracing::error!("Failed to save settings: {}", e);
                        } else {
                            {
                                let mut guard = app.state.cached_settings.lock();
                                *guard = app.settings_edit.clone();
                            }
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
                    UserAction::ButtonClick { id } if id == "cancel" => {
                        app.settings_open = false;
                    }
                    _ => {}
                }
                app.settings_vm.sync_to_wire(&app.wire);
            }
        });
}
