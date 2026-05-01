use crate::App;
use crate::ui::types::AgentStatus;

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
                .map(|s| (s.id.clone(), s.title.clone(), s.id == app.session_store.active_session_id))
                .collect();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (id, title, is_active) in &category_sessions {
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
                    let tab_id = id.clone();
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(title).size(app.ui_store.theme.text_sm).color(text_color),
                            )
                            .fill(bg)
                            .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                            .stroke(stroke)
                            .min_size(egui::vec2(60.0, 28.0)),
                        )
                        .clicked()
                    {
                        app.save_current_session();
                        let old_id = app.session_store.active_session_id.clone();
                        if !app.chat_store.input.trim().is_empty() {
                            app.session_store.drafts.insert(old_id, app.chat_store.input.clone());
                        } else {
                            app.session_store.drafts.remove(&old_id);
                        }
                        app.session_store.active_session_id = tab_id.clone();
                        app.chat_store.input = app.session_store.drafts.remove(&tab_id).unwrap_or_default();
                    }
                }
                // New-tab button (browser style)
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
            });
        }
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
            // Tasks
            let active_tasks = app.task_store.tasks.iter().filter(|t| !t.status.is_terminal()).count();
            let task_btn = if active_tasks > 0 {
                format!("📝 {}", active_tasks)
            } else {
                "📝".to_string()
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(&task_btn).size(app.ui_store.theme.text_sm))
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                )
                .clicked()
            {
                app.task_store.task_panel_open = !app.task_store.task_panel_open;
                if app.task_store.task_panel_open {
                    app.refresh_tasks();
                }
            }
            // MCP
            let mcp_count = app.mcp_store.mcp_config.as_ref().map_or(0, |c| c.servers.len());
            let mcp_btn = if mcp_count > 0 {
                format!("🔌 {}", mcp_count)
            } else {
                "🔌".to_string()
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(&mcp_btn).size(app.ui_store.theme.text_sm))
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                )
                .clicked()
            {
                app.mcp_store.mcp_panel_open = !app.mcp_store.mcp_panel_open;
            }
            // Status
            let (status_color, status_label) = match app.chat_store.agent_status {
                AgentStatus::Online => (app.ui_store.theme.status_online, "Online"),
                AgentStatus::Busy => (app.ui_store.theme.status_busy, "Busy"),
                AgentStatus::Unconfigured => (app.ui_store.theme.status_offline, "Unconfigured"),
                AgentStatus::Offline => (app.ui_store.theme.status_offline, "Offline"),
            };
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, status_color);
            ui.label(
                egui::RichText::new(status_label)
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
            );
            // Token usage (session cumulative)
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
