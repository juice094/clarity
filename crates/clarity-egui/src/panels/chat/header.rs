//! Chat area header.
//!
//! S6 Phase C: the header now hosts the bot identity on the left and the
//! right-rail context toggles + drawer expand icon on the right.  Session tabs
//! remain available for callers that need them (e.g. the custom titlebar).

use crate::App;
use crate::theme::{ICON_CHAT, ICON_FILE, ICON_SETTINGS, ICON_WRENCH};
use crate::ui::types::Session;
use clarity_core::ui::RightRailContext;

/// Compute which drawer contexts are available for the active session.
///
/// The current heuristic is intentionally simple:
/// - Every session gets the plain `Session` context.
/// - Sessions whose category/title hints at Claw also get `Claw`.
/// - Sessions whose category/title hints at a project/workspace also get `Project`.
///
/// As the workspace model matures, this should be driven by explicit session
/// metadata rather than string matching.
pub fn available_contexts(session: Option<&Session>) -> Vec<RightRailContext> {
    let mut contexts = vec![RightRailContext::Session];
    let marker = session.map(|s| format!("{} {}", s.category, s.title).to_lowercase());
    if let Some(ref m) = marker {
        if m.contains("claw") {
            contexts.push(RightRailContext::Claw);
        }
        if m.contains("project") || m.contains("workspace") {
            contexts.push(RightRailContext::Project);
        }
    }
    contexts
}

fn context_icon(ctx: RightRailContext) -> &'static str {
    match ctx {
        RightRailContext::Session => ICON_CHAT,
        RightRailContext::Claw => ICON_WRENCH,
        RightRailContext::Project => ICON_FILE,
    }
}

fn context_tooltip(ctx: RightRailContext) -> &'static str {
    match ctx {
        RightRailContext::Session => "会话上下文",
        RightRailContext::Claw => "Claw",
        RightRailContext::Project => "项目资源",
    }
}

/// Renders the session tabs UI.
pub fn render_session_tabs(app: &mut App, ui: &mut egui::Ui) {
    // All categories render tabs uniformly — no special-casing for emotion.
    // Emotion with a single session shows one tab, same visual weight as others.
    let category_sessions: Vec<(String, String, bool, String)> = app
        .session_store
        .sessions
        .iter()
        .filter(|s| s.category == app.session_store.active_category)
        .map(|s| {
            (
                s.id.clone(),
                s.title.clone(),
                s.id == app.session_store.active_session_id,
                s.category.clone(),
            )
        })
        .collect();

    let theme = &app.ui_store.theme;
    ui.spacing_mut().item_spacing.x = theme.space_4;
    let mut rename_commit: Option<(String, String)> = None;
    let mut tab_to_close: Option<String> = None;

    // Browser-style auto-width tabs
    let reserved_for_plus: f32 = theme.size_new_tab_btn_w;
    let tab_count = category_sessions.len();
    let spacing = ui.spacing().item_spacing.x;
    let total_spacing = if tab_count > 1 {
        (tab_count - 1) as f32 * spacing
    } else {
        0.0
    };
    let total_available = (ui.available_width() - reserved_for_plus - total_spacing).max(0.0);
    let tab_min = theme.size_tab_min_w;
    let tab_hard_min = theme.size_tab_min_w;
    let tab_max = theme.size_tab_max_w;
    let raw_width = if tab_count == 0 {
        0.0
    } else {
        total_available / tab_count as f32
    };
    // When space is too tight even for tab_min, shrink proportionally
    // rather than clamping — this prevents the tab bar from overflowing
    // its allocated zone and being visually truncated.
    let mut tab_width = if raw_width < tab_min {
        raw_width.max(tab_hard_min)
    } else {
        raw_width.clamp(tab_min, tab_max)
    };
    // Fix 2: 防溢出 — 确保所有 tab + spacing 不超过可用空间
    let actual_total = tab_width * tab_count as f32 + total_spacing;
    if actual_total > total_available && tab_count > 0 {
        tab_width = ((total_available - total_spacing) / tab_count as f32).max(theme.space_4);
    }

    for (id, title, is_active, _category) in &category_sessions {
        let editing = app.ui_store.editing_session_id.as_ref() == Some(id);
        if editing {
            // Inline rename TextEdit
            let mut buf = app.ui_store.editing_title.clone();
            let edit_w = tab_width.clamp(80.0, 180.0);
            let resp = ui.add_sized(
                egui::vec2(edit_w, 28.0),
                egui::TextEdit::singleline(&mut buf)
                    .id(ui.id().with(("rename", id)))
                    .font(egui::FontId::proportional(app.ui_store.theme.text_md))
                    .margin(egui::vec2(6.0, 4.0)),
            );
            app.ui_store.editing_title = buf;
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.ui_store.editing_session_id = None;
                app.ui_store.editing_title.clear();
            } else if resp.lost_focus() {
                rename_commit = Some((id.clone(), app.ui_store.editing_title.clone()));
            }
            if resp.changed() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                rename_commit = Some((id.clone(), app.ui_store.editing_title.clone()));
            }
        } else {
            let tab =
                crate::widgets::tab_button(ui, title, *is_active, &app.ui_store.theme, tab_width);
            let tab_response = if tab.response.hovered() {
                tab.response.on_hover_text(title.as_str())
            } else {
                tab.response
            };
            if tab.close_clicked {
                tab_to_close = Some(id.clone());
            } else if tab.double_clicked {
                app.ui_store.editing_session_id = Some(id.clone());
                app.ui_store.editing_title = title.clone();
            } else if tab_response.clicked() {
                app.save_current_session();
                let old_id = app.session_store.active_session_id.clone();
                if !app.chat_store.input.trim().is_empty() {
                    app.session_store
                        .drafts
                        .insert(old_id, app.chat_store.input.clone());
                } else {
                    app.session_store.drafts.remove(&old_id);
                }
                app.session_store.active_session_id = id.clone();
                app.chat_store.input = app.session_store.drafts.remove(id).unwrap_or_default();
                app.chat_store.tool_calls = app
                    .session_store
                    .sessions
                    .iter()
                    .find(|s| s.id == *id)
                    .map(|s| crate::stores::rebuild_tool_calls(&s.messages))
                    .unwrap_or_default();
            }
        }
    }
    if let Some((sid, new_title)) = rename_commit {
        if let Some(session) = app.session_store.sessions.iter_mut().find(|s| s.id == sid) {
            session.title = new_title;
            let _ = crate::session::save_session_internal(session);
        }
        app.ui_store.editing_session_id = None;
        app.ui_store.editing_title.clear();
    }
    // Handle tab close
    if let Some(close_id) = tab_to_close {
        if let Some(session) = app.session_store.sessions.iter().find(|s| s.id == close_id) {
            let _ = crate::session::save_session_internal(session);
        }
        let was_active = app.session_store.active_session_id == close_id;
        app.session_store.sessions.retain(|s| s.id != close_id);
        if was_active {
            let category = app.session_store.active_category.clone();
            if let Some(next) = app
                .session_store
                .sessions
                .iter()
                .find(|s| s.category == category)
            {
                let next_id = next.id.clone();
                app.session_store.active_session_id = next_id.clone();
                app.chat_store.input = app
                    .session_store
                    .drafts
                    .remove(&next_id)
                    .unwrap_or_default();
            } else {
                app.session_store.active_session_id.clear();
                app.chat_store.input.clear();
                app.chat_store.tool_calls.clear();
            }
        }
    }
    // New-tab button (browser style)
    ui.add_space(4.0);
    if ui
        .add(
            egui::Button::new(egui::RichText::new("+").size(app.ui_store.theme.text_base))
                .fill(egui::Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
        )
        .clicked()
    {
        app.new_session();
    }
}

/// Renders the header UI.
pub fn render_header(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let active_session = app.session_store.active_session().cloned();
    let contexts = available_contexts(active_session.as_ref());

    // Header row: bot identity on the left, right-rail controls on the right.
    // The frame adds horizontal padding so the row doesn't touch the side rails.
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_8;

                // ── Left: bot avatar + name ──
                let bot_name = app
                    .settings_store
                    .settings_edit
                    .active_persona_id
                    .as_deref()
                    .unwrap_or("Clarity");
                let initial = bot_name.chars().next().unwrap_or('C').to_string();
                crate::widgets::avatar::avatar(
                    ui,
                    &initial,
                    &theme,
                    Some(theme.accent.linear_multiply(0.25)),
                    Some(theme.accent),
                );
                ui.label(
                    egui::RichText::new(bot_name)
                        .size(theme.text_base)
                        .strong()
                        .color(theme.text_strong),
                );

                // ── Right-rail context toggles pushed to the far right ──
                // S6-C3: this now lives in the full-width chat column, so right
                // alignment is reliable (it was clipped when rendered inside the
                // narrower centered-content Ui).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = theme.space_4;

                    // Drawer expand/collapse icon (rightmost).
                    if crate::widgets::icon_button(
                        ui,
                        ICON_SETTINGS,
                        theme.text_md,
                        theme.accent.linear_multiply(0.25),
                        egui::CornerRadius::same(theme.radius_sm as u8),
                        &theme,
                    )
                    .on_hover_text("展开/折叠右栏")
                    .clicked()
                    {
                        app.view_state.right_rail_visible = !app.view_state.right_rail_visible;
                        app.persist_layout_settings();
                    }

                    // Context switcher: one icon per available context.
                    for ctx in &contexts {
                        let is_active = app.view_state.right_rail_context == *ctx
                            && app.view_state.right_rail_visible;
                        let icon = context_icon(*ctx);
                        let fill = if is_active {
                            theme.accent.linear_multiply(0.2)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        if crate::widgets::icon_button(
                            ui,
                            icon,
                            theme.text_md,
                            fill,
                            egui::CornerRadius::same(theme.radius_sm as u8),
                            &theme,
                        )
                        .on_hover_text(context_tooltip(*ctx))
                        .clicked()
                        {
                            if is_active {
                                app.view_state.right_rail_visible = false;
                            } else {
                                app.view_state.set_right_rail_context(*ctx);
                            }
                            app.persist_layout_settings();
                        }
                    }
                });
            });
        });
    ui.add_space(theme.space_4);

    if let Some(banner) = app.ui_store.network_banner.clone() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(banner)
                    .size(theme.text_sm)
                    .color(theme.status_busy),
            );
            if crate::widgets::icon_button_toolbar(ui, crate::theme::ICON_X, theme.text_sm, &theme)
                .clicked()
            {
                app.ui_store.network_banner = None;
            }
        });
        ui.separator();
    }

    if app.view_state.turn == clarity_core::ui::TurnState::Compacting {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Compacting conversation history…")
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
        });
        ui.separator();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(category: &str, title: &str) -> Session {
        Session {
            id: "s-1".into(),
            title: title.into(),
            category: category.into(),
            messages: vec![],
            updated_at: 0,
            turn_heights: vec![],
        }
    }

    #[test]
    fn plain_session_has_only_session_context() {
        let s = make_session("engineering", "general");
        let ctxs = available_contexts(Some(&s));
        assert_eq!(ctxs, vec![RightRailContext::Session]);
    }

    #[test]
    fn claw_session_adds_claw_context() {
        let s = make_session("claw", "remote");
        let ctxs = available_contexts(Some(&s));
        assert_eq!(
            ctxs,
            vec![RightRailContext::Session, RightRailContext::Claw]
        );
    }

    #[test]
    fn project_session_adds_project_context() {
        let s = make_session("project", "ui refactor");
        let ctxs = available_contexts(Some(&s));
        assert_eq!(
            ctxs,
            vec![RightRailContext::Session, RightRailContext::Project]
        );
    }

    #[test]
    fn workspace_in_title_adds_project_context() {
        let s = make_session("engineering", "workspace onboarding");
        let ctxs = available_contexts(Some(&s));
        assert!(ctxs.contains(&RightRailContext::Project));
    }

    #[test]
    fn no_session_has_only_session_context() {
        let ctxs = available_contexts(None);
        assert_eq!(ctxs, vec![RightRailContext::Session]);
    }
}
