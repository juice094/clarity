use crate::ui::types::ToastLevel;
use crate::{App, SIDEBAR_WIDTH};

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

pub fn render_sidebar(app: &mut App, ctx: &egui::Context) {
    if app.ui_store.sidebar_collapsed {
        return;
    }
    let frame_fill = app.ui_store.theme.bg;
    egui::SidePanel::left("sidebar")
        .default_width(SIDEBAR_WIDTH)
        .min_width(220.0)
        .max_width(360.0)
        .resizable(true)
        .frame(
            egui::Frame::new()
                .fill(frame_fill)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("sidebar_scroll")
                .auto_shrink([true, false])
                .show(ui, |ui| {
                    let theme = app.ui_store.theme.clone();
                    // ── Top toolbar: collapse + global controls ──
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        // Collapse sidebar (left)
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(crate::theme::ICON_ARROW_LEFT).font(
                                        app.ui_store.theme.font_icon(app.ui_store.theme.text_base),
                                    ),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(
                                    egui::CornerRadius::same(app.ui_store.theme.radius_md as u8),
                                ),
                            )
                            .clicked()
                        {
                            app.ui_store.sidebar_collapsed = true;
                        }

                        // Global controls (right-aligned)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;

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

                            // Export current session
                            if let Some(session) = app.session_store.active_session() {
                                if crate::widgets::icon_button_toolbar(
                                    ui,
                                    crate::theme::ICON_EXPORT,
                                    app.ui_store.theme.text_sm,
                                    &app.ui_store.theme,
                                )
                                .on_hover_text("Export session")
                                .clicked()
                                {
                                    let file_name = format!("{}-session.json", session.title);
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("JSON", &["json"])
                                        .set_file_name(&file_name)
                                        .save_file()
                                    {
                                        if let Err(e) =
                                            crate::session::export_session(session, &path)
                                        {
                                            app.push_toast(
                                                format!("Export failed: {}", e),
                                                ToastLevel::Error,
                                            );
                                        } else {
                                            app.push_toast("Session exported", ToastLevel::Info);
                                        }
                                    }
                                }
                            }

                            // Import session
                            if crate::widgets::icon_button_toolbar(
                                ui,
                                crate::theme::ICON_IMPORT,
                                app.ui_store.theme.text_sm,
                                &app.ui_store.theme,
                            )
                            .on_hover_text("Import session")
                            .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("JSON", &["json"])
                                    .pick_file()
                                {
                                    match crate::session::import_session(&path) {
                                        Ok(session) => {
                                            let id = session.id.clone();
                                            app.session_store.sessions.push(session);
                                            app.session_store.active_session_id = id.clone();
                                            app.chat_store.input = String::new();
                                            app.chat_store.last_usage = None;
                                            if let Some(s) = app
                                                .session_store
                                                .sessions
                                                .iter()
                                                .find(|s| s.id == id)
                                            {
                                                app.chat_store.tool_calls =
                                                    crate::stores::rebuild_tool_calls(&s.messages);
                                            }
                                            app.push_toast("Session imported", ToastLevel::Info);
                                        }
                                        Err(e) => {
                                            app.push_toast(
                                                format!("Import failed: {}", e),
                                                ToastLevel::Error,
                                            );
                                        }
                                    }
                                }
                            }

                            // MCP
                            let mcp_count = app
                                .mcp_store
                                .mcp_config
                                .as_ref()
                                .map_or(0, |c| c.servers.len());
                            let mcp_btn_w = if mcp_count > 0 { 36.0 } else { 20.0 };
                            let (mcp_rect, mcp_resp) = ui.allocate_exact_size(
                                egui::vec2(mcp_btn_w, 20.0),
                                egui::Sense::click(),
                            );
                            if ui.is_rect_visible(mcp_rect) {
                                let icon_color = if mcp_resp.hovered() {
                                    app.ui_store.theme.text
                                } else {
                                    app.ui_store.theme.text_dim
                                };
                                ui.allocate_new_ui(
                                    egui::UiBuilder::new().max_rect(mcp_rect),
                                    |ui| {
                                        ui.horizontal_centered(|ui| {
                                            ui.label(
                                                egui::RichText::new(crate::theme::ICON_PLUG)
                                                    .font(
                                                        app.ui_store
                                                            .theme
                                                            .font_icon(app.ui_store.theme.text_base),
                                                    )
                                                    .color(icon_color),
                                            );
                                            if mcp_count > 0 {
                                                ui.label(
                                                    egui::RichText::new(format!("{}", mcp_count))
                                                        .size(app.ui_store.theme.text_xs)
                                                        .color(icon_color),
                                                );
                                            }
                                        });
                                    },
                                );
                            }
                            if mcp_resp.clicked() {
                                app.mcp_store.mcp_panel_open = !app.mcp_store.mcp_panel_open;
                            }

                            // Skills
                            let skills_resp = crate::widgets::icon_button_toolbar(
                                ui,
                                crate::theme::ICON_PUZZLE,
                                app.ui_store.theme.text_base,
                                &app.ui_store.theme,
                            );
                            if skills_resp.clicked() {
                                app.ui_store.skill_panel_open = true;
                            }

                            // Locale toggle
                            let locale_label = match app.ui_store.locale {
                                crate::i18n::Locale::EnUS => "EN",
                                crate::i18n::Locale::ZhCN => "中",
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(locale_label)
                                            .size(app.ui_store.theme.text_xs)
                                            .color(app.ui_store.theme.text_dim)
                                            .monospace(),
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
                                app.ui_store.locale = match app.ui_store.locale {
                                    crate::i18n::Locale::EnUS => crate::i18n::Locale::ZhCN,
                                    crate::i18n::Locale::ZhCN => crate::i18n::Locale::EnUS,
                                };
                            }

                            // Token usage (compact)
                            if let Some((_, _, t)) = app.chat_store.last_usage {
                                ui.label(
                                    egui::RichText::new(format!("{}∑", format_thousands(t)))
                                        .size(app.ui_store.theme.text_xs)
                                        .color(app.ui_store.theme.text_dim)
                                        .monospace(),
                                );
                            }
                        });
                    });
                    ui.add_space(app.ui_store.theme.space_12);

                    // Helper for group headers
                    let group_header = |ui: &mut egui::Ui, text: &str| {
                        ui.label(
                            egui::RichText::new(text)
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                        ui.separator();
                        ui.add_space(theme.space_4);
                    };

                    // Helper for clickable sidebar rows
                    let clickable_row = |ui: &mut egui::Ui,
                                         id: egui::Id,
                                         label: &str,
                                         count: Option<usize>,
                                         is_open: &mut bool| {
                        let caret_icon = if *is_open {
                            crate::theme::ICON_CARET_DOWN
                        } else {
                            crate::theme::ICON_CARET_RIGHT
                        };
                        let caret_color = if *is_open {
                            theme.accent
                        } else {
                            theme.text_dim
                        };
                        let text_color = if *is_open {
                            theme.text
                        } else {
                            theme.text_dim
                        };

                        let resp = crate::widgets::interactive_row(
                            ui,
                            id,
                            *is_open,
                            &theme,
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        egui::RichText::new(label)
                                            .size(theme.text_sm)
                                            .strong()
                                            .color(text_color),
                                    );
                                    if let Some(c) = count {
                                        if c > 0 {
                                            ui.label(
                                                egui::RichText::new(format!("({})", c))
                                                    .size(theme.text_sm)
                                                    .color(theme.text_muted),
                                            );
                                        }
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(8.0);
                                            ui.label(
                                                egui::RichText::new(caret_icon)
                                                    .font(theme.font_icon(theme.text_sm))
                                                    .color(caret_color),
                                            );
                                        },
                                    );
                                });
                            },
                        );
                        if resp.response.clicked() {
                            *is_open = !*is_open;
                        }
                    };

                    // ── ROLES ──
                    group_header(ui, "ROLES");

                    // ── Category navigation ──
                    let categories = [
                        ("emotion", app.t("Emotion")),
                        ("knowledge", app.t("Knowledge")),
                        ("engineering", app.t("Engineering")),
                    ];
                    for (cat, label) in categories {
                        let is_active = app.session_store.active_category == cat;
                        let count = app
                            .session_store
                            .sessions
                            .iter()
                            .filter(|s| s.category == cat)
                            .count();
                        let latest = app
                            .session_store
                            .sessions
                            .iter()
                            .filter(|s| s.category == cat)
                            .max_by_key(|s| s.updated_at);

                        let role_icon = match cat {
                            "emotion" => crate::theme::ICON_CHAT,
                            "knowledge" => crate::theme::ICON_BOOK,
                            _ => crate::theme::ICON_WRENCH,
                        };

                        let subtitle_str = if count > 0 {
                            Some(format!(
                                "{} session{}",
                                count,
                                if count == 1 { "" } else { "s" }
                            ))
                        } else {
                            None
                        };
                        let badge_str = latest.map(|s| {
                            if s.title.chars().count() > 18 {
                                let truncated: String = s.title.chars().take(15).collect();
                                format!("{}...", truncated)
                            } else {
                                s.title.clone()
                            }
                        });

                        let resp = crate::widgets::sidebar_card(
                            ui,
                            role_icon,
                            label,
                            subtitle_str.as_deref(),
                            badge_str.as_deref(),
                            is_active,
                            &theme,
                        );
                        if resp.clicked() {
                            app.switch_category(cat);
                        }
                    }
                    ui.add_space(app.ui_store.theme.space_12);

                    // ── LIVE ──
                    group_header(ui, "LIVE");

                    // ── Web Tabs ──
                    crate::components::web_tabs::render_web_tabs(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Thinking Log ──
                    crate::components::thinking_log::render_thinking_log(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Subagents ──
                    let running_count = app.subagent_store.running_agents.len()
                        + app
                            .subagent_store
                            .parallel_batches
                            .iter()
                            .filter(|b| b.status == "Running")
                            .count();
                    let subagents_expanded = app.ui_store.subagents_expanded;

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Subagents")
                                .size(app.ui_store.theme.text_sm)
                                .strong()
                                .color(app.ui_store.theme.text),
                        );
                        if running_count > 0 {
                            ui.label(
                                egui::RichText::new(format!("({})", running_count))
                                    .size(app.ui_store.theme.text_sm)
                                    .color(app.ui_store.theme.text_muted),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let caret_icon = if subagents_expanded {
                                crate::theme::ICON_CARET_DOWN
                            } else {
                                crate::theme::ICON_CARET_RIGHT
                            };
                            let chevron_resp = crate::widgets::icon_button_toolbar(
                                ui,
                                caret_icon,
                                app.ui_store.theme.text_sm,
                                &app.ui_store.theme,
                            );
                            if chevron_resp.clicked() {
                                app.ui_store.subagents_expanded = !subagents_expanded;
                            }
                        });
                    });

                    if subagents_expanded {
                        ui.add_space(app.ui_store.theme.space_8);
                        crate::panels::subagent_progress::render_subagent_progress(app, ui);
                    }

                    ui.add_space(app.ui_store.theme.space_12);

                    // ── WORKSPACE ──
                    group_header(ui, "WORKSPACE");

                    // ── Tools / Tasks ──
                    crate::components::tools_section::render_tools_section(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Teams ──
                    let team_count = app.team_store.teams.len();
                    clickable_row(
                        ui,
                        ui.id().with("teams_row"),
                        "Teams",
                        Some(team_count),
                        &mut app.team_store.team_panel_open,
                    );

                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Cron Jobs ──
                    crate::panels::cron::render_cron_section(app, ui);
                    ui.add_space(app.ui_store.theme.space_12);

                    // ── ANALYTICS ──
                    group_header(ui, "ANALYTICS");

                    // ── Dashboard ──
                    clickable_row(
                        ui,
                        ui.id().with("dashboard_row"),
                        "Dashboard",
                        None,
                        &mut app.ui_store.dashboard_panel_open,
                    );

                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Plan Timeline ──
                    clickable_row(
                        ui,
                        ui.id().with("plan_timeline_row"),
                        "Plan Timeline",
                        None,
                        &mut app.ui_store.gantt_panel_open,
                    );


                    // Workspace has moved to the right-side panel (Sprint 34 refactor).
                });
        });
}
