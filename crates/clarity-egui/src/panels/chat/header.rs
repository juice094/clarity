use crate::App;

pub fn render_session_tabs(app: &mut App, ui: &mut egui::Ui) {
    if app.session_store.active_category == "emotion" {
        return;
    }
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
    let tab_max = (ui.available_width() - 8.0).max(200.0);
    ui.allocate_ui_with_layout(
        egui::vec2(tab_max, 28.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("session_tabs_scroll")
                .scroll_bar_visibility(
                    egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden,
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let mut rename_commit: Option<(String, String)> = None;
                        let mut tab_to_close: Option<String> = None;
                        for (id, title, is_active, category) in &category_sessions {
                            let editing =
                                app.ui_store.editing_session_id.as_ref() == Some(id);
                            if editing {
                                // Inline rename TextEdit
                                let mut buf = app.ui_store.editing_title.clone();
                                let resp = ui.add_sized(
                                    egui::vec2(120.0, 28.0),
                                    egui::TextEdit::singleline(&mut buf)
                                        .font(egui::FontId::proportional(
                                            app.ui_store.theme.text_sm,
                                        ))
                                        .margin(egui::vec2(6.0, 4.0)),
                                );
                                app.ui_store.editing_title = buf;
                                if resp.lost_focus() {
                                    rename_commit = Some((
                                        id.clone(),
                                        app.ui_store.editing_title.clone(),
                                    ));
                                }
                                if resp.changed()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    rename_commit = Some((
                                        id.clone(),
                                        app.ui_store.editing_title.clone(),
                                    ));
                                }
                            } else {
                                // Active tab bg = content bg for "connected" look;
                                // inactive tabs float above with elevated bg.
                                let bg = if *is_active {
                                    app.ui_store.theme.bg_hover
                                } else {
                                    app.ui_store.theme.bg_elevated
                                };
                                let text_color = if *is_active {
                                    app.ui_store.theme.text_strong
                                } else {
                                    app.ui_store.theme.text_dim
                                };
                                // Precise text width measurement via egui galley.
                                let font_id =
                                    app.ui_store.theme.font(app.ui_store.theme.text_sm);
                                let text_galley = ui.painter().layout_no_wrap(
                                    title.clone(),
                                    font_id.clone(),
                                    egui::Color32::PLACEHOLDER,
                                );
                                let text_width = text_galley.rect.width();
                                const TAB_MIN: f32 = 80.0;
                                const TAB_MAX: f32 = 200.0;
                                const TAB_PAD: f32 = 36.0;
                                let tab_count = category_sessions.len();
                                let use_compressed = tab_count > 8;
                                let tab_width = if use_compressed {
                                    (tab_max / tab_count as f32).max(60.0).min(160.0)
                                } else {
                                    (text_width + TAB_PAD).clamp(TAB_MIN, TAB_MAX)
                                };
                                let (tab_rect, tab_resp) = ui.allocate_exact_size(
                                    egui::vec2(tab_width, 28.0),
                                    egui::Sense::click(),
                                );
                                // Active tab: bottom corners square (connect to content).
                                // Inactive tab: all corners rounded.
                                let cr = if *is_active {
                                    egui::CornerRadius {
                                        nw: app.ui_store.theme.radius_sm as u8,
                                        ne: app.ui_store.theme.radius_sm as u8,
                                        sw: 0,
                                        se: 0,
                                    }
                                } else {
                                    egui::CornerRadius::same(
                                        app.ui_store.theme.radius_sm as u8,
                                    )
                                };
                                ui.painter().rect_filled(tab_rect, cr, bg);
                                // Category indicator dot
                                let dot_color = match category.as_str() {
                                    "emotion" => app.ui_store.theme.status_busy,
                                    "knowledge" => app.ui_store.theme.status_online,
                                    "engineering" => app.ui_store.theme.accent,
                                    _ => app.ui_store.theme.text_dim,
                                };
                                let dot_center = egui::pos2(tab_rect.min.x + 8.0, tab_rect.center().y);
                                ui.painter().circle_filled(dot_center, 3.0, dot_color);
                                // Active tab: 2px accent line at the top.
                                if *is_active {
                                    let accent_line = egui::Rect::from_min_max(
                                        egui::pos2(tab_rect.min.x, tab_rect.min.y),
                                        egui::pos2(tab_rect.max.x, tab_rect.min.y + 2.0),
                                    );
                                    ui.painter().rect_filled(
                                        accent_line,
                                        egui::CornerRadius::ZERO,
                                        app.ui_store.theme.accent,
                                    );
                                }
                                // Title text — clipped to tab interior so long titles
                                // don't bleed into adjacent tabs or the close button.
                                let text_left = tab_rect.min.x + 18.0;
                                let text_clip = egui::Rect::from_min_max(
                                    egui::pos2(tab_rect.min.x + 14.0, tab_rect.min.y),
                                    egui::pos2(tab_rect.max.x - 22.0, tab_rect.max.y),
                                );
                                let max_text_w = tab_width - TAB_PAD;
                                let display_title = if text_width > max_text_w {
                                    let ellipsis_galley = ui.painter().layout_no_wrap(
                                        "…".to_string(),
                                        font_id.clone(),
                                        egui::Color32::PLACEHOLDER,
                                    );
                                    let ellipsis_w = ellipsis_galley.rect.width();
                                    // Collect valid UTF-8 char boundaries to avoid
                                    // slicing inside multi-byte chars (e.g. CJK).
                                    let boundaries: Vec<usize> = std::iter::once(0)
                                        .chain(title.char_indices().map(|(i, c)| i + c.len_utf8()))
                                        .collect();
                                    let mut lo = 0usize;
                                    let mut hi = boundaries.len().saturating_sub(1);
                                    while lo < hi {
                                        let mid = (lo + hi + 1) / 2;
                                        let substr = &title[..boundaries[mid]];
                                        let g = ui.painter().layout_no_wrap(
                                            substr.to_string(),
                                            font_id.clone(),
                                            egui::Color32::PLACEHOLDER,
                                        );
                                        if g.rect.width() + ellipsis_w <= max_text_w {
                                            lo = mid;
                                        } else {
                                            hi = mid - 1;
                                        }
                                    }
                                    let safe_len = boundaries[lo];
                                    if safe_len == 0 {
                                        "…".to_string()
                                    } else {
                                        format!("{}…", &title[..safe_len])
                                    }
                                } else {
                                    title.clone()
                                };
                                ui.painter_at(text_clip).text(
                                    egui::pos2(text_left, tab_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &display_title,
                                    font_id.clone(),
                                    text_color,
                                );
                                // Close button: always show for active tab;
                                // only show on hover for inactive tabs.
                                let show_close = *is_active || tab_resp.hovered();
                                let mut close_clicked = false;
                                if show_close {
                                    let close_rect = egui::Rect::from_min_max(
                                        egui::pos2(
                                            tab_rect.max.x - 20.0,
                                            tab_rect.min.y + 2.0,
                                        ),
                                        egui::pos2(
                                            tab_rect.max.x - 2.0,
                                            tab_rect.max.y - 2.0,
                                        ),
                                    );
                                    let close_id = egui::Id::new(("tab_close", id.clone()));
                                    let close_resp = ui.interact(
                                        close_rect,
                                        close_id,
                                        egui::Sense::click(),
                                    );
                                    let close_col = if close_resp.hovered() {
                                        app.ui_store.theme.danger
                                    } else {
                                        text_color
                                    };
                                    ui.painter().text(
                                        close_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        crate::theme::ICON_X,
                                        app.ui_store
                                            .theme
                                            .font_icon(app.ui_store.theme.text_xs),
                                        close_col,
                                    );
                                    close_clicked = close_resp.clicked();
                                }
                                if use_compressed && tab_resp.hovered() {
                                    let _ = tab_resp.clone().on_hover_text(title.as_str());
                                }

                                if close_clicked {
                                    tab_to_close = Some(id.clone());
                                } else if tab_resp.double_clicked() {
                                    app.ui_store.editing_session_id = Some(id.clone());
                                    app.ui_store.editing_title = title.clone();
                                } else if tab_resp.clicked() {
                                    if let Some(pos) = tab_resp.interact_pointer_pos() {
                                        let close_rect = egui::Rect::from_min_max(
                                            egui::pos2(
                                                tab_rect.max.x - 20.0,
                                                tab_rect.min.y + 2.0,
                                            ),
                                            egui::pos2(
                                                tab_rect.max.x - 2.0,
                                                tab_rect.max.y - 2.0,
                                            ),
                                        );
                                        if !close_rect.contains(pos) {
                                            app.save_current_session();
                                            let old_id =
                                                app.session_store.active_session_id.clone();
                                            if !app.chat_store.input.trim().is_empty() {
                                                app.session_store.drafts.insert(
                                                    old_id,
                                                    app.chat_store.input.clone(),
                                                );
                                            } else {
                                                app.session_store.drafts.remove(&old_id);
                                            }
                                            app.session_store.active_session_id =
                                                id.clone();
                                            app.chat_store.input = app
                                                .session_store
                                                .drafts
                                                .remove(id)
                                                .unwrap_or_default();
                                            app.chat_store.tool_calls = app
                                                .session_store
                                                .sessions
                                                .iter()
                                                .find(|s| s.id == *id)
                                                .map(|s| {
                                                    crate::stores::rebuild_tool_calls(
                                                        &s.messages,
                                                    )
                                                })
                                                .unwrap_or_default();
                                        }
                                    }
                                }
                            }
                        }
                        if let Some((sid, new_title)) = rename_commit {
                            if let Some(session) =
                                app.session_store.sessions.iter_mut().find(|s| s.id == sid)
                            {
                                session.title = new_title;
                                let _ = crate::session::save_session_internal(session);
                            }
                            app.ui_store.editing_session_id = None;
                            app.ui_store.editing_title.clear();
                        }
                        // Handle tab close
                        if let Some(close_id) = tab_to_close {
                            if let Some(session) =
                                app.session_store.sessions.iter().find(|s| s.id == close_id)
                            {
                                let _ = crate::session::save_session_internal(session);
                            }
                            let was_active =
                                app.session_store.active_session_id == close_id;
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
                                    app.new_session();
                                }
                            }
                        }
                        // New-tab button (browser style)
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("+")
                                        .size(app.ui_store.theme.text_base),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(
                                    egui::CornerRadius::same(
                                        app.ui_store.theme.radius_sm as u8,
                                    ),
                                ),
                            )
                            .clicked()
                        {
                            app.new_session();
                        }
                    });
                });
        },
    );
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

    let banner_text = app.ui_store.network_banner.clone();
    if let Some(banner) = banner_text {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&banner)
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
