use crate::App;

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

pub fn render_header(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if app.ui_store.sidebar_collapsed
            && crate::widgets::icon_button_toolbar(
                ui,
                crate::theme::ICON_SEND,
                app.ui_store.theme.text_base,
                &app.ui_store.theme,
            )
            .clicked()
        {
            app.ui_store.sidebar_collapsed = false;
        }
    });
    ui.add_space(app.ui_store.theme.space_4);
    // Separator removed: active tab now "connects" to content area
    // via matching bg color and square bottom corners.

    if let Some(banner) = app.ui_store.network_banner.clone() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(banner)
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.status_busy),
            );
            if crate::widgets::icon_button_toolbar(
                ui,
                crate::theme::ICON_X,
                app.ui_store.theme.text_sm,
                &app.ui_store.theme,
            )
            .clicked()
            {
                app.ui_store.network_banner = None;
            }
        });
        ui.separator();
    }

    if app.chat_store.compacting {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Compacting conversation history…")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
            );
        });
        ui.separator();
    }
}
