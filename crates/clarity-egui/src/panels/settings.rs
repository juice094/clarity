use crate::{app_state::reload_llm, App};
use clarity_wire::{ButtonStyle, TextRole, UserAction, ViewCommand};

/// Pure function: given current settings + theme, produce the declarative command tree.
fn settings_commands(settings: &crate::settings::GuiSettings, _theme: &crate::theme::Theme) -> Vec<ViewCommand> {
    let providers = crate::settings::get_available_models();

    let provider_options: Vec<(String, String)> = providers
        .iter()
        .map(|(k, l, _)| (k.clone(), l.clone()))
        .collect();

    let current_models = providers
        .iter()
        .find(|(k, _, _)| k == &settings.provider)
        .map(|(_, _, m)| m.clone())
        .unwrap_or_default();

    let model_options: Vec<(String, String)> = current_models
        .into_iter()
        .map(|m| (m.clone(), m))
        .collect();

    let approval_options = vec![
        ("interactive".into(), "Interactive — Approve each tool call".into()),
        ("yolo".into(), "Yolo — Auto-approve all".into()),
        ("plan".into(), "Plan — Review plan before execution".into()),
    ];

    vec![
        ViewCommand::HStack {
            children: vec![
                ViewCommand::Text { content: "Provider".into(), role: TextRole::Label, size: 13.0 },
                ViewCommand::ComboBox {
                    id: "provider".into(),
                    selected_value: settings.provider.clone(),
                    options: provider_options,
                    width: 200.0,
                },
            ],
        },
        ViewCommand::Space { height: 8.0 },
        ViewCommand::HStack {
            children: vec![
                ViewCommand::Text { content: "Model".into(), role: TextRole::Label, size: 13.0 },
                ViewCommand::ComboBox {
                    id: "model".into(),
                    selected_value: settings.model.clone(),
                    options: model_options,
                    width: 200.0,
                },
            ],
        },
        ViewCommand::Space { height: 8.0 },
        ViewCommand::HStack {
            children: vec![
                ViewCommand::Text { content: "API Key".into(), role: TextRole::Label, size: 13.0 },
                ViewCommand::TextInput {
                    id: "api_key".into(),
                    value: settings.api_key.clone().unwrap_or_default(),
                    placeholder: String::new(),
                    password: true,
                    width: 200.0,
                },
            ],
        },
        ViewCommand::Space { height: 8.0 },
        ViewCommand::HStack {
            children: vec![
                ViewCommand::Text { content: "Local Model Path".into(), role: TextRole::Label, size: 13.0 },
                ViewCommand::TextInput {
                    id: "local_model_path".into(),
                    value: settings.local_model_path.clone().unwrap_or_default(),
                    placeholder: String::new(),
                    password: false,
                    width: 200.0,
                },
            ],
        },
        ViewCommand::Space { height: 8.0 },
        ViewCommand::HStack {
            children: vec![
                ViewCommand::Text { content: "Approval Mode".into(), role: TextRole::Label, size: 13.0 },
                ViewCommand::ComboBox {
                    id: "approval_mode".into(),
                    selected_value: settings.approval_mode.clone(),
                    options: approval_options,
                    width: 200.0,
                },
            ],
        },
        ViewCommand::Space { height: 16.0 },
        ViewCommand::HStack {
            children: vec![
                ViewCommand::Button {
                    id: "cancel".into(),
                    label: "Cancel".into(),
                    style: ButtonStyle::Secondary,
                    min_width: 80.0,
                    min_height: 32.0,
                },
                ViewCommand::Button {
                    id: "save".into(),
                    label: "Save".into(),
                    style: ButtonStyle::Primary,
                    min_width: 80.0,
                    min_height: 32.0,
                },
            ],
        },
    ]
}

/// Route a user action back into application state.
fn handle_settings_action(action: UserAction, app: &mut App) {
    match action {
        UserAction::ComboChange { id, selected } if id == "provider" => {
            app.settings_edit.provider = selected.clone();
            let providers = crate::settings::get_available_models();
            if let Some((_, _, models)) = providers.iter().find(|(k, _, _)| k == &selected) {
                if let Some(first) = models.first() {
                    app.settings_edit.model = first.clone();
                }
            }
        }
        UserAction::ComboChange { id, selected } if id == "model" => {
            app.settings_edit.model = selected;
        }
        UserAction::ComboChange { id, selected } if id == "approval_mode" => {
            app.settings_edit.approval_mode = selected;
        }
        UserAction::TextInputChange { id, value } if id == "api_key" => {
            app.settings_edit.api_key = if value.is_empty() { None } else { Some(value) };
        }
        UserAction::TextInputChange { id, value } if id == "local_model_path" => {
            app.settings_edit.local_model_path = if value.is_empty() { None } else { Some(value) };
        }
        UserAction::ButtonClick { id } if id == "save" => {
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
}

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

            let commands = settings_commands(&app.settings_edit, &app.theme);
            let mut actions = Vec::new();

            ui.vertical(|ui| {
                crate::ui::protocol_renderer::render_view_commands(ui, &commands, &app.theme, &mut actions);
            });

            for action in actions {
                handle_settings_action(action, app);
            }
        });
}
