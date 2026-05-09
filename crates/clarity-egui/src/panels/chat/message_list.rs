use crate::theme::Theme;
use crate::ui;
use crate::ui::types::Role;
use crate::App;

/// Actions detected during the render pass that must be applied after the
/// `session` mutable borrow is released.
#[derive(Default)]
struct PendingActions {
    copy_content: Option<String>,
    edit_idx: Option<usize>,
    regenerate_idx: Option<usize>,
    save_edit: bool,
    cancel_edit: bool,
}

pub fn render_message_list(app: &mut App, ui: &mut egui::Ui) {
    let available_height = ui.available_height() - 70.0;
    let is_loading = app.chat_store.is_loading;
    let theme = app.ui_store.theme.clone();
    let max_w = app.ui_store.content_max_width;
    let active_id = app.session_store.active_session_id.clone();
    let scroll_y = app.ui_store.last_scroll_offset;
    let mut configure_clicked = false;
    let agent_turn_style = app.ui_store.agent_turn_style;
    let agent_turn_glass = app.ui_store.agent_turn_glass;

    // Pre-calculate content height to avoid stick-to-bottom when messages are short
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
                    .map(|(i, u)| {
                        let editing = app.chat_store.editing_message_idx == Some(u.start);
                        if u.is_user {
                            let bubble_h =
                                session.messages[u.start].cached_height.unwrap_or_else(|| {
                                    ui::render::estimate_height(&session.messages[u.start], max_w, &theme)
                                });
                            if editing {
                                bubble_h + 80.0 // approximate edit controls height
                            } else {
                                bubble_h + 28.0 // action bar
                            }
                        } else {
                            session
                                .turn_heights
                                .get(i)
                                .copied()
                                .flatten()
                                .unwrap_or_else(|| {
                                    let turn =
                                        crate::components::agent_turn::AgentTurn::from_messages(
                                            &session.messages[u.start..u.end],
                                        );
                                    turn.estimate_height(max_w, &theme)
                                })
                                + 28.0 // action bar
                        }
                    })
                    .collect();
                estimates.iter().sum::<f32>() + typing_h
            } else {
                session
                    .messages
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let editing = app.chat_store.editing_message_idx == Some(i);
                        let bubble_h = m
                            .cached_height
                            .unwrap_or_else(|| crate::ui::render::estimate_height(m, max_w, &theme));
                        if editing && m.role == Role::User {
                            bubble_h + 80.0
                        } else {
                            bubble_h + 28.0
                        }
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
        .scroll_bar_visibility(
            egui::containers::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
        )
        .stick_to_bottom(should_stick)
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            let mut pending = PendingActions::default();

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
                        .map(|(i, u)| {
                            let editing = app.chat_store.editing_message_idx == Some(u.start);
                            if u.is_user {
                                let bubble_h =
                                    session.messages[u.start].cached_height.unwrap_or_else(|| {
                                        ui::render::estimate_height(&session.messages[u.start], max_w, &theme)
                                    });
                                if editing {
                                    bubble_h + 80.0
                                } else {
                                    bubble_h + 28.0
                                }
                            } else {
                                session
                                    .turn_heights
                                    .get(i)
                                    .copied()
                                    .flatten()
                                    .unwrap_or_else(|| {
                                        let turn =
                                            crate::components::agent_turn::AgentTurn::from_messages(
                                                &session.messages[u.start..u.end],
                                            );
                                        turn.estimate_height(max_w, &theme)
                                    })
                                    + 28.0
                            }
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

                    for (i, unit) in units
                        .iter()
                        .enumerate()
                        .skip(start_idx)
                        .take(end_idx - start_idx)
                    {
                        if unit.is_user {
                            let editing = app.chat_store.editing_message_idx == Some(unit.start);
                            let bubble_h = if editing {
                                let (h, save, cancel) =
                                    render_edit_bubble(ui, &mut app.chat_store.edit_buffer, &theme);
                                if save {
                                    pending.save_edit = true;
                                }
                                if cancel {
                                    pending.cancel_edit = true;
                                }
                                h
                            } else {
                                ui::render::message_bubble(
                                    ui,
                                    &session.messages[unit.start],
                                    &theme,
                                    true,
                                )
                            };
                            session.messages[unit.start].cached_height = Some(bubble_h);

                            if !editing {
                                ui.horizontal(|ui| {
                                    if let Some(msg) = session.messages.get(unit.start) {
                                        ui.label(
                                            egui::RichText::new(format_relative_time(
                                                msg.timestamp,
                                            ))
                                            .size(theme.text_xs)
                                            .color(theme.text_dim),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(theme.space_8);
                                            let edit_btn = egui::Button::new(
                                                egui::RichText::new(crate::theme::ICON_EDIT)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(theme.text_muted),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ));
                                            if ui.add(edit_btn).on_hover_text("Edit").clicked() {
                                                pending.edit_idx = Some(unit.start);
                                            }
                                            ui.add_space(4.0);
                                            let copy_btn = egui::Button::new(
                                                egui::RichText::new(crate::theme::ICON_COPY)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(theme.text_muted),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ));
                                            if ui.add(copy_btn).on_hover_text("Copy").clicked() {
                                                if let Some(msg) = session.messages.get(unit.start)
                                                {
                                                    ui.ctx().copy_text(msg.content.clone());
                                                    pending.copy_content =
                                                        Some(msg.content.clone());
                                                }
                                            }
                                        },
                                    );
                                });
                                ui.add_space(theme.space_8);
                            }
                        } else {
                            let mut turn = crate::components::agent_turn::AgentTurn::from_messages(
                                &session.messages[unit.start..unit.end],
                            );
                            let bubble_h = if agent_turn_glass {
                                crate::render::turn_renderer::render_agent_turn_glass(
                                    ui, &mut turn, &theme, i,
                                )
                            } else {
                                crate::render::turn_renderer::render_agent_turn(
                                    ui, &mut turn, &theme, i,
                                )
                            };
                            session.turn_heights[i] = Some(bubble_h);

                            ui.horizontal(|ui| {
                                if let Some(msg) = session.messages.get(unit.start) {
                                    ui.label(
                                        egui::RichText::new(format_relative_time(msg.timestamp))
                                            .size(theme.text_xs)
                                            .color(theme.text_dim),
                                    );
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(theme.space_8);
                                        let regen_btn = egui::Button::new(
                                            egui::RichText::new(crate::theme::ICON_REFRESH)
                                                .font(theme.font_icon(theme.text_sm))
                                                .color(theme.text_muted),
                                        )
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(
                                            theme.radius_sm as u8,
                                        ));
                                        if ui.add(regen_btn).on_hover_text("Regenerate").clicked() {
                                            pending.regenerate_idx = Some(unit.start);
                                        }
                                        ui.add_space(4.0);
                                        let copy_btn = egui::Button::new(
                                            egui::RichText::new(crate::theme::ICON_COPY)
                                                .font(theme.font_icon(theme.text_sm))
                                                .color(theme.text_muted),
                                        )
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(
                                            theme.radius_sm as u8,
                                        ));
                                        if ui.add(copy_btn).on_hover_text("Copy").clicked() {
                                            let content: String = session.messages
                                                [unit.start..unit.end]
                                                .iter()
                                                .map(|m| m.content.as_str())
                                                .collect::<Vec<_>>()
                                                .join("\n\n");
                                            ui.ctx().copy_text(content.clone());
                                            pending.copy_content = Some(content);
                                        }
                                    },
                                );
                            });
                            ui.add_space(theme.space_8);
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
                        .enumerate()
                        .map(|(i, m)| {
                            let editing = app.chat_store.editing_message_idx == Some(i);
                            let bubble_h = m
                                .cached_height
                                .unwrap_or_else(|| crate::ui::render::estimate_height(m, max_w, &theme));
                            if editing && m.role == Role::User {
                                bubble_h + 80.0
                            } else {
                                bubble_h + 28.0
                            }
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
                        let editing = app.chat_store.editing_message_idx == Some(i);
                        let show_header = if session.messages[i].role == Role::Agent {
                            i == 0 || session.messages[i - 1].role != Role::Agent
                        } else {
                            true
                        };
                        let bubble_h = if editing && session.messages[i].role == Role::User {
                            let (h, save, cancel) =
                                render_edit_bubble(ui, &mut app.chat_store.edit_buffer, &theme);
                            if save {
                                pending.save_edit = true;
                            }
                            if cancel {
                                pending.cancel_edit = true;
                            }
                            h
                        } else {
                            ui::render::message_bubble(
                                ui,
                                &session.messages[i],
                                &theme,
                                show_header,
                            )
                        };
                        session.messages[i].cached_height = Some(bubble_h);

                        if !editing {
                            if session.messages[i].role == Role::User {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format_relative_time(
                                            session.messages[i].timestamp,
                                        ))
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(theme.space_8);
                                            let edit_btn = egui::Button::new(
                                                egui::RichText::new(crate::theme::ICON_EDIT)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(theme.text_muted),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ));
                                            if ui.add(edit_btn).on_hover_text("Edit").clicked() {
                                                pending.edit_idx = Some(i);
                                            }
                                            ui.add_space(4.0);
                                            let copy_btn = egui::Button::new(
                                                egui::RichText::new(crate::theme::ICON_COPY)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(theme.text_muted),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ));
                                            if ui.add(copy_btn).on_hover_text("Copy").clicked() {
                                                ui.ctx()
                                                    .copy_text(session.messages[i].content.clone());
                                                pending.copy_content =
                                                    Some(session.messages[i].content.clone());
                                            }
                                        },
                                    );
                                });
                                ui.add_space(theme.space_8);
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format_relative_time(
                                            session.messages[i].timestamp,
                                        ))
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(theme.space_8);
                                            let regen_btn = egui::Button::new(
                                                egui::RichText::new(crate::theme::ICON_REFRESH)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(theme.text_muted),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ));
                                            if ui
                                                .add(regen_btn)
                                                .on_hover_text("Regenerate")
                                                .clicked()
                                            {
                                                pending.regenerate_idx = Some(i);
                                            }
                                            ui.add_space(4.0);
                                            let copy_btn = egui::Button::new(
                                                egui::RichText::new(crate::theme::ICON_COPY)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(theme.text_muted),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ));
                                            if ui.add(copy_btn).on_hover_text("Copy").clicked() {
                                                ui.ctx()
                                                    .copy_text(session.messages[i].content.clone());
                                                pending.copy_content =
                                                    Some(session.messages[i].content.clone());
                                            }
                                        },
                                    );
                                });
                                ui.add_space(theme.space_8);
                            }
                        }
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
            pending
        });

    let pending = output.inner;
    if pending.save_edit {
        app.commit_edit();
    }
    if pending.cancel_edit {
        app.cancel_edit();
    }
    if let Some(idx) = pending.edit_idx {
        app.start_edit(idx);
    }
    if let Some(idx) = pending.regenerate_idx {
        app.regenerate(idx);
    }
    if pending.copy_content.is_some() {
        app.push_toast("Copied to clipboard", crate::ui::types::ToastLevel::Info);
    }

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

/// Lightweight index-based turn descriptor — avoids cloning every Message per frame.
struct RenderUnit {
    start: usize,
    end: usize,
    is_user: bool,
}

fn aggregate_turns(messages: &[ui::types::Message]) -> Vec<RenderUnit> {
    let mut units = Vec::new();
    let mut i = 0;
    let n = messages.len();
    while i < n {
        if messages[i].role == Role::User {
            units.push(RenderUnit {
                start: i,
                end: i + 1,
                is_user: true,
            });
            i += 1;
        } else {
            let start = i;
            while i < n && messages[i].role == Role::Agent {
                i += 1;
            }
            units.push(RenderUnit {
                start,
                end: i,
                is_user: false,
            });
        }
    }
    units
}

// ============================================================================
// Inline edit bubble
// ============================================================================

/// Render an editable user bubble with Save / Cancel controls.
/// Returns `(total_height, save_clicked, cancel_clicked)`.
fn render_edit_bubble(ui: &mut egui::Ui, buffer: &mut String, theme: &Theme) -> (f32, bool, bool) {
    let start_y = ui.cursor().min.y;
    let max_width = (ui.available_width() * 0.72).max(280.0);

    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.set_max_width(max_width);
        egui::Frame::new()
            .fill(theme.user_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.set_min_width(48.0);
                    ui.add(
                        egui::TextEdit::multiline(buffer)
                            .desired_width(ui.available_width())
                            .desired_rows(3)
                            .font(theme.font(theme.text_base))
                            .text_color(theme.text_strong),
                    );
                });
            });
    });
    ui.add_space(theme.space_8);

    let mut save_clicked = false;
    let mut cancel_clicked = false;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add(theme.primary_button("Save")).clicked() {
            save_clicked = true;
        }
        if ui.add(theme.ghost_button("Cancel")).clicked() {
            cancel_clicked = true;
        }
    });
    ui.add_space(theme.space_16);

    (ui.cursor().min.y - start_y, save_clicked, cancel_clicked)
}

// ============================================================================
// Relative time formatting
// ============================================================================

/// Format an `Instant` as a human-readable relative string (e.g. "2m ago").
fn format_relative_time(instant: std::time::Instant) -> String {
    let elapsed = instant.elapsed().as_secs();
    if elapsed < 60 {
        "just now".to_string()
    } else if elapsed < 3600 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 86400 {
        format!("{}h ago", elapsed / 3600)
    } else {
        format!("{}d ago", elapsed / 86400)
    }
}
