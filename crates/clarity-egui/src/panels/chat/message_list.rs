use crate::ui;
use crate::ui::types::Role;
use crate::App;

pub fn render_message_list(app: &mut App, ui: &mut egui::Ui) {
    let available_height = ui.available_height() - 70.0;
    let is_loading = app.chat_store.is_loading;
    let theme = app.ui_store.theme.clone();
    let active_id = app.session_store.active_session_id.clone();
    let scroll_y = app.ui_store.last_scroll_offset;
    let mut configure_clicked = false;
    let agent_turn_style = app.ui_store.agent_turn_style;
    let agent_turn_glass = app.ui_store.agent_turn_glass;

    // Pre-calculate content height to avoid stick-to-bottom when messages are short
    // (prevents large top-padding and content clipping in windowed mode).
    let total_estimated: f32 = if let Some(session) = app
        .session_store
        .sessions
        .iter()
        .find(|s| s.id == active_id)
    {
        if session.messages.is_empty() && !is_loading {
            0.0
        } else {
            let typing_h = if is_loading
                && session.messages.last().is_none_or(|m| m.role == Role::User)
                && app.chat_store.tool_calls.is_empty()
            {
                60.0
            } else {
                0.0
            };
            if agent_turn_style {
                let units = aggregate_turns(&session.messages);
                let estimates: Vec<f32> = units
                    .iter()
                    .enumerate()
                    .map(|(i, u)| match u {
                        RenderUnit::User(msg) => msg
                            .cached_height
                            .unwrap_or_else(|| ui::render::estimate_height(msg)),
                        RenderUnit::AgentTurn(msgs) => session
                            .turn_heights
                            .get(i)
                            .copied()
                            .flatten()
                            .unwrap_or_else(|| {
                                let turn =
                                    crate::components::agent_turn::AgentTurn::from_messages(msgs);
                                turn.estimate_height(&theme)
                            }),
                    })
                    .collect();
                estimates.iter().sum::<f32>() + typing_h
            } else {
                session
                    .messages
                    .iter()
                    .map(|m| {
                        m.cached_height
                            .unwrap_or_else(|| crate::ui::render::estimate_height(m))
                    })
                    .sum::<f32>()
                    + typing_h
            }
        }
    } else {
        0.0
    };
    let should_stick = app.chat_store.stick_to_bottom && total_estimated >= available_height;

    let mut scroll_up = false;
    let output = egui::ScrollArea::vertical()
        .id_salt("chat_scroll")
        .scroll_bar_visibility(egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .stick_to_bottom(should_stick)
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(session) = app
                .session_store
                .sessions
                .iter_mut()
                .find(|s| s.id == active_id)
            {
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
                                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                                .min_size(egui::vec2(180.0, 40.0)),
                            )
                            .clicked()
                        {
                            configure_clicked = true;
                        }
                    });
                } else if agent_turn_style {
                    // --- AgentTurn aggregation mode ---
                    let units = aggregate_turns(&session.messages);
                    if session.turn_heights.len() < units.len() {
                        session.turn_heights.resize(units.len(), None);
                    }

                    let estimates: Vec<f32> = units
                        .iter()
                        .enumerate()
                        .map(|(i, u)| match u {
                            RenderUnit::User(msg) => msg
                                .cached_height
                                .unwrap_or_else(|| ui::render::estimate_height(msg)),
                            RenderUnit::AgentTurn(msgs) => session
                                .turn_heights
                                .get(i)
                                .copied()
                                .flatten()
                                .unwrap_or_else(|| {
                                    let turn =
                                        crate::components::agent_turn::AgentTurn::from_messages(
                                            msgs,
                                        );
                                    turn.estimate_height(&theme)
                                }),
                        })
                        .collect();

                    let mut cumulative = 0.0;
                    let mut start_idx = 0;
                    let mut end_idx = units.len();

                    for (i, h) in estimates.iter().enumerate() {
                        if cumulative + h >= scroll_y && start_idx == 0 {
                            start_idx = i.saturating_sub(3);
                        }
                        cumulative += h;
                        if cumulative >= scroll_y + available_height && end_idx == units.len() {
                            end_idx = (i + 3).min(units.len());
                            break;
                        }
                    }

                    if start_idx > 0 {
                        let top = estimates[..start_idx].iter().sum::<f32>();
                        ui.allocate_space(egui::vec2(ui.available_width(), top));
                    }

                    let mut msg_idx = 0;
                    for (i, unit) in units
                        .iter()
                        .enumerate()
                        .skip(start_idx)
                        .take(end_idx - start_idx)
                    {
                        match unit {
                            RenderUnit::User(_) => {
                                while msg_idx < session.messages.len()
                                    && session.messages[msg_idx].role != Role::User
                                {
                                    msg_idx += 1;
                                }
                                if msg_idx < session.messages.len() {
                                    let actual = ui::render::message_bubble(
                                        ui,
                                        &session.messages[msg_idx],
                                        &theme,
                                        true,
                                    );
                                    session.messages[msg_idx].cached_height = Some(actual);
                                    msg_idx += 1;
                                }
                            }
                            RenderUnit::AgentTurn(msgs) => {
                                let mut turn =
                                    crate::components::agent_turn::AgentTurn::from_messages(msgs);
                                let actual = if agent_turn_glass {
                                    crate::render::turn_renderer::render_agent_turn_glass(
                                        ui, &mut turn, &theme,
                                    )
                                } else {
                                    crate::render::turn_renderer::render_agent_turn(
                                        ui, &mut turn, &theme,
                                    )
                                };
                                session.turn_heights[i] = Some(actual);
                                while msg_idx < session.messages.len()
                                    && session.messages[msg_idx].role == Role::Agent
                                {
                                    msg_idx += 1;
                                }
                            }
                        }
                    }

                    if end_idx < units.len() {
                        let bottom = estimates[end_idx..].iter().sum::<f32>();
                        ui.allocate_space(egui::vec2(ui.available_width(), bottom));
                    }

                    if is_loading
                        && session.messages.last().is_none_or(|m| m.role == Role::User)
                        && app.chat_store.tool_calls.is_empty()
                    {
                        ui::render::typing_indicator(ui, &theme);
                    }
                } else {
                    // --- Legacy virtualized message list ---
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
                        let show_header = if session.messages[i].role == Role::Agent {
                            i == 0 || session.messages[i - 1].role != Role::Agent
                        } else {
                            true
                        };
                        let actual = ui::render::message_bubble(
                            ui,
                            &session.messages[i],
                            &theme,
                            show_header,
                        );
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

// ============================================================================
// Turn aggregation
// ============================================================================

enum RenderUnit {
    User(ui::types::Message),
    AgentTurn(Vec<ui::types::Message>),
}

fn aggregate_turns(messages: &[ui::types::Message]) -> Vec<RenderUnit> {
    let mut units = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        if messages[i].role == Role::User {
            units.push(RenderUnit::User(messages[i].clone()));
            i += 1;
        } else {
            let mut turn = Vec::new();
            while i < messages.len() && messages[i].role == Role::Agent {
                turn.push(messages[i].clone());
                i += 1;
            }
            units.push(RenderUnit::AgentTurn(turn));
        }
    }
    units
}
