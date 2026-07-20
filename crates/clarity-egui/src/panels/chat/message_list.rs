use crate::App;
use crate::design_system;
use crate::stores::SessionStore;
use crate::theme::Theme;
use crate::ui;
use crate::ui::types::Role;
use clarity_ui::widgets::icon_button::icon_button_toolbar_colored;
use clarity_ui::widgets::text_input::TextInput;

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

/// Result of computing the virtual-list window: which turn units to render.
struct VirtualWindow {
    start_idx: usize,
    end_idx: usize,
}

impl VirtualWindow {
    /// Compute which turn units are visible given the current scroll offset and
    /// available height. Includes a small overscan (3 units) above and below.
    fn compute(estimates: &[f32], scroll_y: f32, available_height: f32) -> Self {
        let mut cumulative = 0.0;
        let mut start_idx = 0;
        let mut end_idx = estimates.len();

        for (i, h) in estimates.iter().enumerate() {
            if cumulative + h >= scroll_y && start_idx == 0 {
                start_idx = i.saturating_sub(3);
            }
            cumulative += h;
            if cumulative >= scroll_y + available_height && end_idx == estimates.len() {
                end_idx = (i + 3).min(estimates.len());
                break;
            }
        }
        VirtualWindow { start_idx, end_idx }
    }

    /// Height of all units before the visible window.
    fn top_spacer(&self, estimates: &[f32]) -> f32 {
        if self.start_idx > 0 {
            estimates[..self.start_idx].iter().sum::<f32>()
        } else {
            0.0
        }
    }

    /// Height of all units after the visible window.
    fn bottom_spacer(&self, estimates: &[f32]) -> f32 {
        if self.end_idx < estimates.len() {
            estimates[self.end_idx..].iter().sum::<f32>()
        } else {
            0.0
        }
    }
}

/// Compute per-unit height estimates for the virtual list.
///
/// Takes individual slices rather than `&Session` so the caller can split
/// immutable (`messages`) and mutable (`turn_heights`) borrows across slices.
#[allow(clippy::too_many_arguments)]
fn compute_unit_estimates(
    messages: &[crate::ui::types::Message],
    units: &[RenderUnit],
    editing_idx: Option<usize>,
    max_w: f32,
    theme: &Theme,
    metrics: &crate::pretext::EguiFontMetrics,
    turn_heights: &mut [Option<f32>],
    turn_cache: &mut [Option<crate::components::agent_turn::AgentTurn>],
    out: &mut Vec<f32>,
) {
    // Keep this in sync with `estimate_total_height` so the virtual window and
    // the ScrollArea stick-to-bottom math agree.
    let action_gap_h = theme.space_24 + theme.space_8;
    out.clear();
    out.extend(units.iter().enumerate().map(|(i, u)| {
        let editing = editing_idx == Some(u.start);
        if u.is_user {
            let bubble_h = messages[u.start].cached_height.unwrap_or_else(|| {
                crate::ui::render::estimate_height(&messages[u.start], max_w, theme, metrics)
            });
            if editing {
                bubble_h + 80.0
            } else {
                bubble_h + action_gap_h
            }
        } else {
            let cached = turn_heights.get(i).copied().flatten();
            cached.unwrap_or_else(|| {
                let turn = turn_cache[i].get_or_insert_with(|| {
                    crate::components::agent_turn::AgentTurn::from_messages(
                        &messages[u.start..u.end],
                    )
                });
                turn.estimate_height(max_w, theme, metrics)
            }) + action_gap_h
        }
    }));
}

/// Renders the message list UI.
///
/// `theme` is passed by reference so the hot path avoids cloning the whole
/// `Theme` every frame.
pub fn render_message_list(app: &mut App, ui: &mut egui::Ui, theme: &Theme) {
    let turn_state = app.view_state.turn;
    let is_loading = turn_state == clarity_core::ui::TurnState::Loading;
    let max_w = ui.available_width();
    let active_id = app.context.session_store.active_session_id.clone();
    let pretext_enabled = app.context.ui_store.pretext_estimate_enabled;
    let metrics = app.pretext_metrics.clone();
    let render_metrics = pretext_enabled.then_some(&metrics);
    #[cfg(feature = "line-mode")]
    let line_cursor_selected = app.context.ui_store.line_cursor_selected;
    #[cfg(not(feature = "line-mode"))]
    let line_cursor_selected: Option<usize> = None;

    let available_height = ui.available_height();
    let scroll_y = app.context.ui_store.last_scroll_offset;
    let mut pending = PendingActions::default();

    {
        let (session_store, chat_store) = app.chat_session_both_mut();
        let SessionStore {
            sessions,
            turn_cache,
            ..
        } = session_store;
        let cache = turn_cache.entry(active_id.clone()).or_default();
        if let Some(session) = sessions.iter_mut().find(|s| s.id == active_id) {
            #[cfg(feature = "line-mode")]
            {
                session.line_offset_buffer.clear();
                let mut acc = 0;
                for msg in &session.messages {
                    session.line_offset_buffer.push(acc);
                    acc += msg.lines.len();
                }
                app.context.ui_store.line_cursor_total_lines = acc;
            }
            #[cfg(not(feature = "line-mode"))]
            session.line_offset_buffer.clear();

            // Phase 1 pretext PoC: pre-populate cached heights with pretext
            // measurements so the virtual list and stick-to-bottom use stable
            // first-frame estimates.
            for m in &mut session.messages {
                if m.cached_height.is_none() {
                    m.cached_height = Some(crate::ui::render::estimate_height(
                        m, max_w, theme, &metrics,
                    ));
                }
            }

            if session.messages.is_empty() && !is_loading {
                // Empty state is rendered by `ChatApp::render` so the composer is
                // centered vertically instead of pinned to the bottom.
                return;
            }

            let units = aggregate_turns(&session.messages);
            if session.turn_heights.len() < units.len() {
                session.turn_heights.resize(units.len(), None);
            }
            cache.resize(units.len(), None);
            if !units.is_empty() {
                // Only invalidate the last agent turn while a turn is actively
                // streaming/generating. When idle the content is stable, so
                // keeping the cached AgentTurn avoids rebuilding it every frame.
                let last_idx = units.len() - 1;
                let turn_active = !matches!(turn_state, clarity_core::ui::TurnState::Idle);
                if turn_active && !units[last_idx].is_user {
                    cache[last_idx] = None;
                }
            }

            let editing_idx = chat_store.editing_message_idx;
            compute_unit_estimates(
                &session.messages,
                &units,
                editing_idx,
                max_w,
                theme,
                &metrics,
                &mut session.turn_heights,
                cache,
                &mut session.estimate_buffer,
            );
            let win = VirtualWindow::compute(&session.estimate_buffer, scroll_y, available_height);

            let top = win.top_spacer(&session.estimate_buffer);
            if top > 0.0 {
                ui.allocate_space(egui::vec2(ui.available_width(), top));
            }

            let tool_calls_empty = chat_store.tool_calls.is_empty();
            let edit_buffer = &mut chat_store.edit_buffer;
            let status_message = chat_store.status_message.as_deref().unwrap_or("");

            for (i, unit) in units
                .iter()
                .enumerate()
                .skip(win.start_idx)
                .take(win.end_idx - win.start_idx)
            {
                if unit.is_user && session.messages[unit.start].content.trim().is_empty() {
                    continue;
                }
                render_unit(
                    ui,
                    session,
                    unit,
                    i,
                    theme,
                    editing_idx,
                    edit_buffer,
                    line_cursor_selected,
                    render_metrics,
                    cache,
                    &mut pending,
                );
            }

            let bottom = win.bottom_spacer(&session.estimate_buffer);
            if bottom > 0.0 {
                ui.allocate_space(egui::vec2(ui.available_width(), bottom));
            }

            if is_loading && !status_message.is_empty() {
                crate::widgets::status_message::status_message(ui, theme, status_message);
            }

            if is_loading
                && session.messages.last().is_none_or(|m| m.role == Role::User)
                && tool_calls_empty
            {
                let rendered = crate::widgets::draft_indicator::draft_indicator(
                    ui,
                    theme,
                    &chat_store.draft_status,
                );
                if !rendered {
                    ui::render::typing_indicator(ui, theme);
                }
            }
        }
    }

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
        app.navigate(clarity_core::ui::AppView::Settings.into());
    }
    if pending.copy_content.is_some() {
        app.push_toast("Copied to clipboard", crate::ui::types::ToastLevel::Info);
    }
}

/// Estimate the total rendered height of the message list content, including
/// inter-unit spacing and the typing indicator, without doing any actual layout.
///
/// This is used by `ChatApp::render` to decide whether the conversation is
/// tall enough to need a scrollable area. It intentionally uses only the
/// current-width estimate so that cached heights from previous frames do not
/// inflate the value after a resize.
pub fn estimate_total_height(app: &mut crate::App, content_max_width: f32, theme: &Theme) -> f32 {
    let max_w = content_max_width;
    let metrics = &app.pretext_metrics;
    let is_loading = app.view_state.turn == clarity_core::ui::TurnState::Loading;
    let active_id = app.context.session_store.active_session_id.clone();
    let editing_idx = app.chat_store().editing_message_idx;
    let tool_calls_empty = app.chat_store().tool_calls.is_empty();

    let session_store = &mut app.context.session_store;
    let SessionStore {
        sessions,
        turn_cache,
        ..
    } = session_store;
    let cache = turn_cache.entry(active_id.clone()).or_default();
    let Some(session) = sessions.iter_mut().find(|s| s.id == active_id) else {
        return 0.0;
    };
    if session.messages.is_empty() && !is_loading {
        return 0.0;
    }

    let units = aggregate_turns(&session.messages);
    cache.resize(units.len(), None);
    if !units.is_empty() {
        let last_idx = units.len() - 1;
        let turn_active = !matches!(app.view_state.turn, clarity_core::ui::TurnState::Idle);
        if turn_active && !units[last_idx].is_user {
            cache[last_idx] = None;
        }
    }

    // ponytail: cache the total height estimate across frames when the
    // conversation is idle. Streaming/content changes invalidate via the key.
    let turn_state = app.view_state.turn;
    let estimate_key = (units.len(), max_w, editing_idx, turn_state);
    if matches!(turn_state, clarity_core::ui::TurnState::Idle)
        && session.estimate_key == Some(estimate_key)
    {
        return session.cached_total_height.unwrap_or(0.0);
    }

    let mut total = 0.0_f32;
    // ponytail: prefer cached rendered heights. The cold-path estimator
    // systematically overestimates (action-bar + inter-unit spacing), which
    // inflates max_scroll and scrolls the conversation above the viewport.
    let action_gap_h = theme.space_24 + theme.space_8;
    for (i, u) in units.iter().enumerate() {
        let editing = editing_idx == Some(u.start);
        let cached = if u.is_user {
            session.messages[u.start].cached_height
        } else {
            session.turn_heights.get(i).copied().flatten()
        };
        let h = if let Some(bubble_h) = cached {
            if editing {
                bubble_h + 80.0
            } else {
                bubble_h + action_gap_h
            }
        } else if u.is_user {
            let bubble_h = crate::ui::render::estimate_height(
                &session.messages[u.start],
                max_w,
                theme,
                metrics,
            );
            if editing {
                bubble_h + 80.0
            } else {
                bubble_h + action_gap_h
            }
        } else {
            let turn = cache[i].get_or_insert_with(|| {
                crate::components::agent_turn::AgentTurn::from_messages(
                    &session.messages[u.start..u.end],
                )
            });
            let turn_h = turn.estimate_height(max_w, theme, metrics);
            turn_h + action_gap_h
        };
        total += h;
    }

    if is_loading
        && session.messages.last().is_none_or(|m| m.role == Role::User)
        && tool_calls_empty
    {
        total += 60.0;
    }

    if matches!(turn_state, clarity_core::ui::TurnState::Idle) {
        session.estimate_key = Some(estimate_key);
        session.cached_total_height = Some(total);
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

/// Render a single aggregated unit inside the virtual list.
#[allow(clippy::too_many_arguments)]
fn render_unit(
    ui: &mut egui::Ui,
    session: &mut crate::ui::types::Session,
    unit: &RenderUnit,
    unit_index: usize,
    theme: &Theme,
    editing_idx: Option<usize>,
    edit_buffer: &mut String,
    line_cursor_selected: Option<usize>,
    render_metrics: Option<&crate::pretext::EguiFontMetrics>,
    turn_cache: &mut [Option<crate::components::agent_turn::AgentTurn>],
    pending: &mut PendingActions,
) {
    let editing = editing_idx == Some(unit.start);
    if unit.is_user {
        let bubble_h = if editing {
            let (h, save, cancel) = render_edit_bubble(ui, edit_buffer, theme);
            if save {
                pending.save_edit = true;
            }
            if cancel {
                pending.cancel_edit = true;
            }
            h
        } else {
            let sel = line_cursor_selected.and_then(|g| {
                let start = session.line_offset_buffer[unit.start];
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
                theme,
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
            render_message_actions(ui, theme, unit, session, pending, true);
        }
    } else {
        let bubble_h = {
            let turn = turn_cache[unit_index].get_or_insert_with(|| {
                crate::components::agent_turn::AgentTurn::from_messages(
                    &session.messages[unit.start..unit.end],
                )
            });
            crate::render::turn_renderer::render_agent_turn(ui, turn, theme, unit_index)
        };
        session.turn_heights[unit_index] = Some(bubble_h);

        render_message_actions(ui, theme, unit, session, pending, false);
    }
}

/// Render a lightweight action/meta row beneath a message.
///
/// User messages get [Edit | Copy]; agent messages get [Copy | Regenerate].
/// Timestamp is shown only when the message is older than a minute, in the
/// smallest/dimmest caption style so it does not compete with content.
fn render_message_actions(
    ui: &mut egui::Ui,
    theme: &Theme,
    unit: &RenderUnit,
    session: &crate::ui::types::Session,
    pending: &mut PendingActions,
    is_user: bool,
) {
    let row_id = ui.id().with(unit.start).with("msg_actions");
    let hovered = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(row_id))
        .unwrap_or(false);

    let row_response = ui.horizontal(|ui| {
        ui.set_min_height(theme.space_20);
        // Timestamp on the left, very subtle.
        if let Some(msg) = session.messages.get(unit.start) {
            if let Some(ts) = format_relative_time(msg.timestamp) {
                design_system::text_with_size_color(ui, ts, theme.text_xs, theme.text_dim);
            } else {
                // Occupies the same vertical space so the row height is stable.
                ui.add_space(theme.text_xs);
            }
        }

        // Action icons on the right; only visible on hover to keep the
        // conversation clean. They remain allocated so text does not jump.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let icon_alpha = if hovered { 1.0 } else { 0.0 };
            let action_color = theme.text_dim.linear_multiply(icon_alpha);
            if is_user {
                if icon_button_toolbar_colored(
                    ui,
                    crate::theme::ICON_COPY,
                    theme.text_xs,
                    action_color,
                    theme,
                )
                .on_hover_text("Copy")
                .clicked()
                {
                    if let Some(msg) = session.messages.get(unit.start) {
                        ui.ctx().copy_text(msg.content.clone());
                        pending.copy_content = Some(msg.content.clone());
                    }
                }
                ui.add_space(4.0);
                if icon_button_toolbar_colored(
                    ui,
                    crate::theme::ICON_EDIT,
                    theme.text_xs,
                    action_color,
                    theme,
                )
                .on_hover_text("Edit")
                .clicked()
                {
                    pending.edit_idx = Some(unit.start);
                }
            } else {
                if icon_button_toolbar_colored(
                    ui,
                    crate::theme::ICON_REFRESH,
                    theme.text_xs,
                    action_color,
                    theme,
                )
                .on_hover_text("Regenerate")
                .clicked()
                {
                    pending.regenerate_idx = Some(unit.start);
                }
                ui.add_space(4.0);
                if icon_button_toolbar_colored(
                    ui,
                    crate::theme::ICON_COPY,
                    theme.text_xs,
                    action_color,
                    theme,
                )
                .on_hover_text("Copy")
                .clicked()
                {
                    let content: String = session.messages[unit.start..unit.end]
                        .iter()
                        .map(|m| m.content.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    ui.ctx().copy_text(content.clone());
                    pending.copy_content = Some(content);
                }
            }
        });
    });

    // Persist hover state for the row so the icons fade in/out smoothly.
    // (The row itself spans the full width, so hovering anywhere over the
    // meta row reveals the actions.)
    let is_hovered = row_response.response.hovered();
    ui.ctx().data_mut(|d| d.insert_temp(row_id, is_hovered));

    design_system::gap(ui, design_system::Space::S1);
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
        clarity_ui::design_system::Elevation::Elevated
            .frame(theme)
            .fill(theme.user_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.set_min_width(48.0);
                    ui.add(
                        TextInput::multiline(buffer)
                            .transparent()
                            .width(ui.available_width())
                            .desired_rows(3),
                    );
                });
            });
    });
    design_system::gap(ui, design_system::Space::S1);

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
    design_system::gap(ui, design_system::Space::S3);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::{ContentBlock, Message, Role, SessionContext};

    fn make_message(role: Role, content: &str) -> Message {
        let mut msg = Message {
            role,
            content: content.to_string(),
            blocks: vec![ContentBlock::Text {
                text: content.to_string(),
            }],
            timestamp: std::time::Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: vec![],
        };
        msg.prepare();
        msg
    }

    #[test]
    fn estimate_total_height_caches_when_idle() {
        let egui_ctx = egui::Context::default();
        let mut app = crate::apps::test_app(&egui_ctx);
        let mut session = crate::session::new_session(0, SessionContext::Chat);
        session.id = "cache-test".to_string();
        session.messages.push(make_message(
            Role::Agent,
            "This is a stable agent turn used for cache validation.",
        ));
        app.context.session_store.sessions.push(session);
        app.context.session_store.active_session_id = "cache-test".to_string();
        app.view_state.turn = clarity_core::ui::TurnState::Idle;

        let theme = app.context.ui_store.theme.clone();
        let mut first = 0.0_f32;
        let _ = egui_ctx.run_ui(egui::RawInput::default(), |_ctx| {
            first = estimate_total_height(&mut app, 600.0, &theme);
        });
        assert!(first > 0.0, "height should be positive");

        let session = app
            .context
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == "cache-test")
            .expect("session exists");
        assert!(
            session.estimate_key.is_some(),
            "estimate key should be populated after idle estimate"
        );
        assert_eq!(
            session.cached_total_height,
            Some(first),
            "cached total height should match first estimate"
        );

        let mut second = 0.0_f32;
        let _ = egui_ctx.run_ui(egui::RawInput::default(), |_ctx| {
            second = estimate_total_height(&mut app, 600.0, &theme);
        });
        assert_eq!(
            first, second,
            "second estimate should return the cached value"
        );
    }

    #[test]
    fn estimate_total_height_invalidates_cache_when_turn_active() {
        let egui_ctx = egui::Context::default();
        let mut app = crate::apps::test_app(&egui_ctx);
        let mut session = crate::session::new_session(0, SessionContext::Chat);
        session.id = "active-test".to_string();
        session.messages.push(make_message(
            Role::Agent,
            "This turn is considered active and should not be cached.",
        ));
        app.context.session_store.sessions.push(session);
        app.context.session_store.active_session_id = "active-test".to_string();
        app.view_state.turn = clarity_core::ui::TurnState::Loading;

        let theme = app.context.ui_store.theme.clone();
        let mut first = 0.0_f32;
        let _ = egui_ctx.run_ui(egui::RawInput::default(), |_ctx| {
            first = estimate_total_height(&mut app, 600.0, &theme);
        });
        assert!(first > 0.0);

        let session = app
            .context
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == "active-test")
            .expect("session exists");
        assert!(
            session.estimate_key.is_none(),
            "estimate key should not be cached while turn is active"
        );
    }
}
