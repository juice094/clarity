use crate::App;
use crate::ui;
use crate::ui::types::Role;

pub fn render_message_list(app: &mut App, ui: &mut egui::Ui) {
    let available_height = ui.available_height() - 70.0;
    let is_loading = app.chat_store.is_loading;
    let theme = app.ui_store.theme.clone();
    let active_id = app.session_store.active_session_id.clone();
    let scroll_y = app.ui_store.last_scroll_offset;
    let mut configure_clicked = false;

    let mut scroll_up = false;
    let output = egui::ScrollArea::vertical()
        .id_salt("chat_scroll")
        .stick_to_bottom(app.chat_store.stick_to_bottom)
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(session) = app.session_store.sessions.iter_mut().find(|s| s.id == active_id) {
                if session.messages.is_empty() && !is_loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.label(
                            egui::RichText::new("Clarity")
                                .size(theme.text_2xl)
                                .strong()
                                .color(theme.text_dim),
                        );
                        ui.add_space(app.ui_store.theme.space_8);
                        ui.label(
                            egui::RichText::new("Local-first AI agent runtime")
                                .size(app.ui_store.theme.text_base)
                                .color(theme.text_dim),
                        );
                        ui.add_space(app.ui_store.theme.space_24);
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Configure Settings")
                                        .size(app.ui_store.theme.text_base)
                                        .color(theme.text),
                                )
                                .fill(theme.surface)
                                .corner_radius(egui::CornerRadius::same(
                                    theme.radius_sm as u8,
                                ))
                                .min_size(egui::vec2(180.0, 40.0)),
                            )
                            .clicked()
                        {
                            configure_clicked = true;
                        }
                    });
                } else {
                    // --- Virtualized message list ---
                    let estimates: Vec<f32> = session
                        .messages
                        .iter()
                        .map(|m| {
                            m.cached_height
                                .unwrap_or_else(|| ui::render::estimate_height(m))
                        })
                        .collect();

                    let mut cumulative = 0.0;
                    let mut start_idx = 0;
                    let mut end_idx = session.messages.len();

                    for (i, h) in estimates.iter().enumerate() {
                        if cumulative + h >= scroll_y && start_idx == 0 {
                            start_idx = i.saturating_sub(3);
                        }
                        cumulative += h;
                        if cumulative >= scroll_y + available_height
                            && end_idx == session.messages.len()
                        {
                            end_idx = (i + 3).min(session.messages.len());
                            break;
                        }
                    }

                    if start_idx > 0 {
                        let top = estimates[..start_idx].iter().sum::<f32>();
                        ui.allocate_space(egui::vec2(ui.available_width(), top));
                    }

                    for i in start_idx..end_idx {
                        let actual =
                            ui::render::message_bubble(ui, &session.messages[i], &theme);
                        session.messages[i].cached_height = Some(actual);
                    }

                    if end_idx < session.messages.len() {
                        let bottom = estimates[end_idx..].iter().sum::<f32>();
                        ui.allocate_space(egui::vec2(ui.available_width(), bottom));
                    }

                    if is_loading
                        && session.messages.last().is_none_or(|m| m.role == Role::User)
                        && app.chat_store.tool_calls.is_empty()
                    {
                        ui::render::typing_indicator(ui, &theme);
                    }
                }
            }
            // Detect user scroll-up intent to release stick-to-bottom.
            ui.input(|i| {
                for event in &i.events {
                    if let egui::Event::MouseWheel { delta, .. } = event {
                        if delta.y > 0.0 {
                            scroll_up = true;
                        }
                    }
                }
            });
        });

    if scroll_up {
        app.chat_store.stick_to_bottom = false;
    }
    app.ui_store.last_scroll_offset = output.state.offset.y;
    if configure_clicked {
        app.settings_store.settings_open = true;
        app.settings_store.settings_edit = {
            let guard = app.state.cached_settings.lock();
            guard.clone()
        };
    }

}
