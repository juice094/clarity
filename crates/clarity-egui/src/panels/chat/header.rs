use crate::ui::types::AgentStatus;
use crate::App;

fn format_thousands(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
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
        // ── Category instance tabs (hidden for emotion) ──
        if app.session_store.active_category != "emotion" {
            let category_sessions: Vec<(String, String, bool)> = app
                .session_store
                .sessions
                .iter()
                .filter(|s| s.category == app.session_store.active_category)
                .map(|s| {
                    (
                        s.id.clone(),
                        s.title.clone(),
                        s.id == app.session_store.active_session_id,
                    )
                })
                .collect();
            let right_reserve = 220.0;
            let tab_max = (ui.available_width() - right_reserve - 8.0).max(200.0);
            ui.allocate_ui_with_layout(
                egui::vec2(tab_max, 28.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("session_tabs_scroll")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let mut rename_commit: Option<(String, String)> = None;
                                let mut tab_to_close: Option<String> = None;
                                for (id, title, is_active) in &category_sessions {
                            let editing = app.ui_store.editing_session_id.as_ref() == Some(id);
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
                                    rename_commit =
                                        Some((id.clone(), app.ui_store.editing_title.clone()));
                                }
                                if resp.changed() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    rename_commit =
                                        Some((id.clone(), app.ui_store.editing_title.clone()));
                                }
                            } else {
                                let bg = if *is_active {
                                    app.ui_store.theme.surface
                                } else {
                                    app.ui_store.theme.bg_elevated
                                };
                                let text_color = if *is_active {
                                    app.ui_store.theme.text_strong
                                } else {
                                    app.ui_store.theme.text_dim
                                };
                                let stroke = if *is_active {
                                    egui::Stroke::new(1.5, app.ui_store.theme.accent)
                                } else {
                                    egui::Stroke::new(1.0, app.ui_store.theme.border)
                                };
                                let tab_width =
                                    (title.chars().count() as f32 * 7.5 + 32.0).clamp(60.0, 160.0);
                                let (tab_rect, tab_resp) = ui.allocate_exact_size(
                                    egui::vec2(tab_width, 28.0),
                                    egui::Sense::click(),
                                );
                                let cr =
                                    egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8);
                                ui.painter().rect_filled(tab_rect, cr, bg);
                                if stroke.width > 0.0 {
                                    ui.painter().rect_stroke(
                                        tab_rect,
                                        cr,
                                        stroke,
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                // Title text
                                ui.painter().text(
                                    egui::pos2(tab_rect.min.x + 10.0, tab_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    title.as_str(),
                                    app.ui_store.theme.font(app.ui_store.theme.text_sm),
                                    text_color,
                                );
                                // Close button area
                                let close_rect = egui::Rect::from_min_max(
                                    egui::pos2(tab_rect.max.x - 20.0, tab_rect.min.y + 2.0),
                                    egui::pos2(tab_rect.max.x - 2.0, tab_rect.max.y - 2.0),
                                );
                                let close_id = egui::Id::new(("tab_close", id.clone()));
                                let close_resp =
                                    ui.interact(close_rect, close_id, egui::Sense::click());
                                let close_col = if close_resp.hovered() {
                                    app.ui_store.theme.danger
                                } else {
                                    text_color
                                };
                                ui.painter().text(
                                    close_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    crate::theme::ICON_X,
                                    app.ui_store.theme.font_icon(app.ui_store.theme.text_xs),
                                    close_col,
                                );

                                if close_resp.clicked() {
                                    tab_to_close = Some(id.clone());
                                } else if tab_resp.double_clicked() {
                                    app.ui_store.editing_session_id = Some(id.clone());
                                    app.ui_store.editing_title = title.clone();
                                } else if tab_resp.clicked() {
                                    if let Some(pos) = tab_resp.interact_pointer_pos() {
                                        if !close_rect.contains(pos) {
                                            app.save_current_session();
                                            let old_id =
                                                app.session_store.active_session_id.clone();
                                            if !app.chat_store.input.trim().is_empty() {
                                                app.session_store
                                                    .drafts
                                                    .insert(old_id, app.chat_store.input.clone());
                                            } else {
                                                app.session_store.drafts.remove(&old_id);
                                            }
                                            app.session_store.active_session_id = id.clone();
                                            app.chat_store.input = app
                                                .session_store
                                                .drafts
                                                .remove(id)
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
                                    app.new_session();
                                }
                            }
                        }
                        // New-tab button (browser style)
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("+").size(app.ui_store.theme.text_base),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(
                                    egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8),
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
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // Settings
            if crate::widgets::icon_button_toolbar(
                ui,
                crate::theme::ICON_SETTINGS,
                app.ui_store.theme.text_base,
                &app.ui_store.theme,
            )
            .clicked()
            {
                app.settings_store.settings_open = true;
                app.settings_store.settings_edit = {
                    let guard = app.state.cached_settings.lock();
                    guard.clone()
                };
            }
            // MCP (hide if space is tight)
            if ui.available_width() > 60.0 {
                let mcp_count = app
                    .mcp_store
                    .mcp_config
                    .as_ref()
                    .map_or(0, |c| c.servers.len());
                let mcp_btn = if mcp_count > 0 {
                    format!("🔌 {}", mcp_count)
                } else {
                    "🔌".to_string()
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(&mcp_btn).size(app.ui_store.theme.text_sm),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(
                            app.ui_store.theme.radius_sm as u8,
                        )),
                    )
                    .clicked()
                {
                    app.mcp_store.mcp_panel_open = !app.mcp_store.mcp_panel_open;
                }
            }
            // Status
            let (status_color, status_label) = match app.chat_store.agent_status {
                AgentStatus::Online => (app.ui_store.theme.status_online, "Online"),
                AgentStatus::Busy => (app.ui_store.theme.status_busy, "Busy"),
                AgentStatus::Unconfigured => (app.ui_store.theme.status_offline, "Unconfigured"),
                AgentStatus::Offline => (app.ui_store.theme.status_offline, "Offline"),
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, status_color);
            ui.label(
                egui::RichText::new(status_label)
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
            );
            // Token usage (session cumulative) — hide when space is tight
            if ui.available_width() > 100.0 {
                if let Some((p, c, t)) = app.chat_store.last_usage {
                    ui.label(
                        egui::RichText::new(format!(
                            "Session: {}↑ {}↓ {}∑",
                            format_thousands(p),
                            format_thousands(c),
                            format_thousands(t)
                        ))
                        .size(app.ui_store.theme.text_sm)
                        .color(app.ui_store.theme.text_dim)
                        .monospace(),
                    );
                }
            }
        });
    });
    ui.add_space(app.ui_store.theme.space_4);
    ui.separator();

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
