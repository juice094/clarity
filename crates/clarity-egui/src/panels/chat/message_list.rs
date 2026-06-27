use crate::App;
use crate::theme::Theme;
use crate::ui;
use crate::ui::types::Role;

/// Actions detected during the render pass that must be applied after the
/// `session` mutable borrow is released.
#[derive(Default)]
struct PendingActions {
    copy_content: Option<String>,
    edit_idx: Option<usize>,
    regenerate_idx: Option<usize>,
    retry_error_idx: Option<usize>,
    switch_model: bool,
    save_edit: bool,
    cancel_edit: bool,
}

/// Renders the message list UI.
pub fn render_message_list(app: &mut App, ui: &mut egui::Ui) {
    let available_height = ui.available_height();
    let is_loading = app.view_state.turn == clarity_core::ui::TurnState::Loading;
    let theme = app.ui_store.theme.clone();
    // The parent (ScrollArea) already constrains the content column width.
    // Use the actual available width so cached heights and estimates stay
    // consistent when the window is narrower than `content_max_width`.
    let max_w = ui.available_width();
    let active_id = app.session_store.active_session_id.clone();
    let scroll_y = app.ui_store.last_scroll_offset;
    let pretext_enabled = app.ui_store.pretext_estimate_enabled;
    let pretext_metrics = app.pretext_metrics.clone();
    let metrics = &pretext_metrics;
    let render_metrics = pretext_enabled.then_some(metrics);
    #[cfg(feature = "line-mode")]
    let line_cursor_selected = app.ui_store.line_cursor_selected;
    #[cfg(not(feature = "line-mode"))]
    let line_cursor_selected: Option<usize> = None;

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
            let units = aggregate_turns(&session.messages);
            let estimates: Vec<f32> = units
                .iter()
                .enumerate()
                .map(|(i, u)| {
                    let editing = app.chat_store.editing_message_idx == Some(u.start);
                    if u.is_user {
                        let bubble_h =
                            session.messages[u.start].cached_height.unwrap_or_else(|| {
                                ui::render::estimate_height(
                                    &session.messages[u.start],
                                    max_w,
                                    &theme,
                                    metrics,
                                )
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
                                let turn = crate::components::agent_turn::AgentTurn::from_messages(
                                    &session.messages[u.start..u.end],
                                );
                                turn.estimate_height(max_w, &theme, metrics)
                            })
                            + 28.0 // action bar
                    }
                })
                .collect();
            estimates.iter().sum::<f32>() + typing_h
        }
    } else {
        0.0
    };
    let _should_stick = app.chat_store.stick_to_bottom && total_estimated >= available_height;

    let mut scroll_up = false;
    // The ScrollArea has been moved up to `chat/mod.rs::render_chat_area` so
    // the scrollbar rides the right edge of the full chat_content area (Kimi
    // style). This function receives the already-scrollable inner Ui.
    let mut pending = PendingActions::default();

    if let Some(session) = app
        .session_store
        .sessions
        .iter_mut()
        .find(|s| s.id == active_id)
    {
        #[cfg(feature = "line-mode")]
        let msg_line_offsets: Vec<usize> = {
            let mut v = Vec::with_capacity(session.messages.len());
            let mut acc = 0;
            for msg in &session.messages {
                v.push(acc);
                acc += msg.lines.len();
            }
            app.ui_store.line_cursor_total_lines = acc;
            v
        };
        #[cfg(not(feature = "line-mode"))]
        let msg_line_offsets: Vec<usize> = Vec::new();

        // Phase 1 pretext PoC: pre-populate cached heights with pretext
        // measurements so the virtual list and stick-to-bottom use stable
        // first-frame estimates.
        for m in &mut session.messages {
            if m.cached_height.is_none() {
                m.cached_height = Some(crate::ui::render::estimate_height(
                    m, max_w, &theme, metrics,
                ));
            }
        }

        if session.messages.is_empty() && !is_loading {
            // Empty state is rendered by `render_chat_area` so the composer is
            // centered vertically instead of pinned to the bottom.
            return;
        }

        // --- AgentTurn aggregation mode (single unified path) ---
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
                    let bubble_h = session.messages[u.start].cached_height.unwrap_or_else(|| {
                        ui::render::estimate_height(
                            &session.messages[u.start],
                            max_w,
                            &theme,
                            metrics,
                        )
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
                            let turn = crate::components::agent_turn::AgentTurn::from_messages(
                                &session.messages[u.start..u.end],
                            );
                            turn.estimate_height(max_w, &theme, metrics)
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
            if unit.is_user && session.messages[unit.start].content.trim().is_empty() {
                continue;
            }
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
                    let sel = line_cursor_selected.and_then(|g| {
                        let start = msg_line_offsets[unit.start];
                        let end = start + session.messages[unit.start].lines.len();
                        if g >= start && g < end {
                            Some(g - start)
                        } else {
                            None
                        }
                    });
                    ui::render::message_bubble(
                        ui,
                        &session.messages[unit.start],
                        &theme,
                        true,
                        unit.start,
                        &mut pending.retry_error_idx,
                        &mut pending.switch_model,
                        sel,
                        render_metrics,
                    )
                };
                session.messages[unit.start].cached_height = Some(bubble_h);

                if !editing {
                    ui.horizontal(|ui| {
                        if let Some(msg) = session.messages.get(unit.start) {
                            if let Some(ts) = format_relative_time(msg.timestamp) {
                                ui.label(
                                    egui::RichText::new(ts)
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                );
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(theme.space_8);
                            let edit_btn = egui::Button::new(
                                egui::RichText::new(crate::theme::ICON_EDIT)
                                    .font(theme.font_icon(theme.text_sm))
                                    .color(theme.text_muted),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
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
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                            if ui.add(copy_btn).on_hover_text("Copy").clicked() {
                                if let Some(msg) = session.messages.get(unit.start) {
                                    ui.ctx().copy_text(msg.content.clone());
                                    pending.copy_content = Some(msg.content.clone());
                                }
                            }
                        });
                    });
                    ui.add_space(theme.space_8);
                }
            } else {
                let mut turn = crate::components::agent_turn::AgentTurn::from_messages(
                    &session.messages[unit.start..unit.end],
                );
                let bubble_h =
                    crate::render::turn_renderer::render_agent_turn(ui, &mut turn, &theme, i);
                session.turn_heights[i] = Some(bubble_h);

                ui.horizontal(|ui| {
                    if let Some(msg) = session.messages.get(unit.start) {
                        if let Some(ts) = format_relative_time(msg.timestamp) {
                            ui.label(
                                egui::RichText::new(ts)
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            );
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(theme.space_8);
                        let regen_btn = egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_REFRESH)
                                .font(theme.font_icon(theme.text_sm))
                                .color(theme.text_muted),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
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
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                        if ui.add(copy_btn).on_hover_text("Copy").clicked() {
                            let content: String = session.messages[unit.start..unit.end]
                                .iter()
                                .map(|m| m.content.as_str())
                                .collect::<Vec<_>>()
                                .join("\n\n");
                            ui.ctx().copy_text(content.clone());
                            pending.copy_content = Some(content);
                        }
                    });
                });
                ui.add_space(theme.space_8);
            }
        }

        if end_idx < units.len() {
            let bottom = estimates[end_idx..].iter().sum::<f32>();
            ui.allocate_space(egui::vec2(ui.available_width(), bottom));
        }

        // Always show a transient status message while the agent is working,
        // even when tool calls are in flight (StepBegin / StatusUpdate).
        if is_loading && app.chat_store.status_message.is_some() {
            crate::widgets::status_message::status_message(
                ui,
                &theme,
                app.chat_store.status_message.as_deref().unwrap_or(""),
            );
        }

        if is_loading
            && session.messages.last().is_none_or(|m| m.role == Role::User)
            && app.chat_store.tool_calls.is_empty()
        {
            // Prefer the new draft indicator when the backend has emitted one.
            // Falls back to the legacy typing indicator so the UI never goes blank.
            let rendered = crate::widgets::draft_indicator::draft_indicator(
                ui,
                &theme,
                &app.chat_store.draft_status,
            );
            if !rendered {
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
    if let Some(idx) = pending.retry_error_idx {
        app.regenerate(idx);
    }
    if pending.switch_model {
        app.view_state.main = clarity_core::ui::AppView::Settings;
    }
    if pending.copy_content.is_some() {
        app.push_toast("Copied to clipboard", crate::ui::types::ToastLevel::Info);
    }

    if scroll_up {
        app.chat_store.stick_to_bottom = false;
    }
}

/// Estimate the total rendered height of the message list content, including
/// inter-unit spacing and the typing indicator, without doing any actual layout.
///
/// This is used by `render_chat_area` to decide whether the conversation is
/// tall enough to need a scrollable area. It intentionally uses only the
/// current-width estimate so that cached heights from previous frames do not
/// inflate the value after a resize.
pub fn estimate_total_height(app: &crate::App, content_max_width: f32) -> f32 {
    let max_w = content_max_width;
    let theme = app.ui_store.theme.clone();
    let metrics = &app.pretext_metrics;
    let is_loading = app.view_state.turn == clarity_core::ui::TurnState::Loading;
    let active_id = app.session_store.active_session_id.clone();

    let Some(session) = app
        .session_store
        .sessions
        .iter()
        .find(|s| s.id == active_id)
    else {
        return 0.0;
    };
    if session.messages.is_empty() && !is_loading {
        return 0.0;
    }

    let units = aggregate_turns(&session.messages);
    let mut total = 0.0_f32;
    for u in units.iter() {
        let editing = app.chat_store.editing_message_idx == Some(u.start);
        let h = if u.is_user {
            let bubble_h = crate::ui::render::estimate_height(
                &session.messages[u.start],
                max_w,
                &theme,
                metrics,
            );
            if editing {
                bubble_h + 80.0
            } else {
                bubble_h + 28.0
            }
        } else {
            let turn = crate::components::agent_turn::AgentTurn::from_messages(
                &session.messages[u.start..u.end],
            );
            let turn_h = turn.estimate_height(max_w, &theme, metrics);
            turn_h + 28.0
        };
        total += h;
    }
    // Each unit adds `space_8` before the action bar and `space_8` after it.
    // The +28 action-bar height is already included in the per-unit estimate.
    total += units.len() as f32 * theme.space_8 * 2.0;

    if is_loading
        && session.messages.last().is_none_or(|m| m.role == Role::User)
        && app.chat_store.tool_calls.is_empty()
    {
        total += 60.0;
    }
    total
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
/// Returns `None` for messages less than 60 seconds old to avoid visual clutter.
fn format_relative_time(instant: std::time::Instant) -> Option<String> {
    let elapsed = instant.elapsed().as_secs();
    if elapsed < 60 {
        None
    } else if elapsed < 3600 {
        Some(format!("{}m ago", elapsed / 60))
    } else if elapsed < 86400 {
        Some(format!("{}h ago", elapsed / 3600))
    } else {
        Some(format!("{}d ago", elapsed / 86400))
    }
}
