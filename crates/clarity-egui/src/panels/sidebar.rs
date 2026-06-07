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

pub fn render_sidebar(app: &mut App, ctx: &egui::Context) {
    if app.ui_store.sidebar_collapsed {
        return;
    }
    let frame_fill = app.ui_store.theme.bg;
    let default_w = app
        .settings_store
        .settings_edit
        .sidebar_width
        .unwrap_or(app.ui_store.theme.size_sidebar);

    let panel = egui::SidePanel::left("sidebar")
        .default_width(default_w)
        .min_width(180.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(frame_fill)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("sidebar_scroll")
                .auto_shrink([true, false])
                .show(ui, |ui| {
                    let theme = app.ui_store.theme.clone();
                    // ── Top toolbar: collapse + new session ──
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // Collapse sidebar
                        if crate::widgets::icon_button_toolbar(
                            ui,
                            crate::theme::ICON_ARROW_LEFT,
                            app.ui_store.theme.text_base,
                            &app.ui_store.theme,
                        )
                        .on_hover_text("Collapse sidebar")
                        .clicked()
                        {
                            app.ui_store.sidebar_collapsed = true;
                        }

                        // New session button (Kimi-style prominent action)
                        let new_session_btn = egui::Button::new(
                            egui::RichText::new("+ 新建会话")
                                .size(app.ui_store.theme.text_sm)
                                .color(app.ui_store.theme.text),
                        )
                        .fill(theme.bg_hover)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                        if ui.add(new_session_btn).on_hover_text("New session (Ctrl+N)").clicked() {
                            if !app.chat_store.is_loading {
                                app.new_session();
                            }
                        }

                        // Right: settings only
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::widgets::icon_button_toolbar(
                                ui,
                                crate::theme::ICON_SETTINGS,
                                app.ui_store.theme.text_base,
                                &app.ui_store.theme,
                            )
                            .on_hover_text("Settings")
                            .clicked()
                            {
                                app.settings_store.settings_open = true;
                                app.settings_store.settings_edit = {
                                    let guard = app.state.cached_settings.lock();
                                    guard.clone()
                                };
                            }
                        });
                    });
                    ui.add_space(app.ui_store.theme.space_12);

                    // Helper for group headers — Kimi-style: no ALL CAPS, no separator
                    let group_header = |ui: &mut egui::Ui, text: &str| {
                        ui.add_space(theme.space_8);
                        ui.label(
                            egui::RichText::new(text)
                                .size(theme.text_xs)
                                .color(theme.text_dim)
                                .strong(),
                        );
                        ui.add_space(theme.space_4);
                    };

                    // Helper for clickable sidebar rows
                    let clickable_row =
                        |ui: &mut egui::Ui,
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
                            let text_color = if *is_open { theme.text } else { theme.text_dim };

                            let resp =
                                crate::widgets::interactive_row(ui, *is_open, &theme, |ui| {
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
                                });
                            if resp.response.clicked() {
                                *is_open = !*is_open;
                            }
                        };

                    // ── 角色 ──
                    group_header(ui, "角色");

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

                    // ── 实时 ──
                    group_header(ui, "实时");

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

                    // ── 工作区 ──
                    group_header(ui, "工作区");

                    // ── Tools / Tasks ──
                    crate::components::tools_section::render_tools_section(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Teams ──
                    let team_count = app.team_store.teams.len();
                    let mut team_open = matches!(
                        app.view_state.right,
                        Some(clarity_core::ui::SidePanel::Team)
                    );
                    clickable_row(ui, "Teams", Some(team_count), &mut team_open);
                    if team_open
                        != matches!(
                            app.view_state.right,
                            Some(clarity_core::ui::SidePanel::Team)
                        )
                    {
                        app.view_state
                            .toggle_right(clarity_core::ui::SidePanel::Team);
                    }

                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Cron Jobs ──
                    crate::panels::cron::render_cron_section(app, ui);
                    ui.add_space(app.ui_store.theme.space_12);

                    // ── 分析 ──
                    group_header(ui, "分析");

                    // ── Dashboard ──
                    let mut dashboard_open =
                        app.view_state.main == clarity_core::ui::AppView::Dashboard;
                    clickable_row(ui, "Dashboard", None, &mut dashboard_open);
                    if dashboard_open
                        != (app.view_state.main == clarity_core::ui::AppView::Dashboard)
                    {
                        app.view_state.main =
                            if app.view_state.main == clarity_core::ui::AppView::Dashboard {
                                clarity_core::ui::AppView::Chat
                            } else {
                                clarity_core::ui::AppView::Dashboard
                            };
                    }

                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Plan Timeline ──
                    clickable_row(
                        ui,
                        "Plan Timeline",
                        None,
                        &mut app.ui_store.gantt_panel_open,
                    );

                    // ── 底部用户区 (Kimi-style) ──
                    ui.add_space(theme.space_16);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        // Avatar placeholder
                        let avatar_size = 28.0;
                        let (avatar_rect, _avatar_resp) = ui.allocate_exact_size(
                            egui::vec2(avatar_size, avatar_size),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(
                            avatar_rect.center(),
                            avatar_size * 0.5,
                            theme.accent,
                        );
                        let initial = ui.fonts(|f| {
                            f.layout(
                                "U".to_string(),
                                theme.font_bold(theme.text_sm),
                                egui::Color32::WHITE,
                                f32::INFINITY,
                            )
                        });
                        let label_pos = avatar_rect.center() - initial.rect.size() * 0.5;
                        ui.painter().galley(label_pos, initial, egui::Color32::WHITE);

                        // User name + model badge
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("User")
                                    .size(theme.text_sm)
                                    .strong()
                                    .color(theme.text),
                            );
                            let model = app.settings_store.settings_edit.model.trim();
                            if !model.is_empty() {
                                ui.label(
                                    egui::RichText::new(model)
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                );
                            }
                        });
                    });
                });
        });

    let actual_w = panel.response.rect.width();
    let stored_w = app
        .settings_store
        .settings_edit
        .sidebar_width
        .unwrap_or(0.0);
    if (actual_w - stored_w).abs() > 1.0 {
        app.settings_store.settings_edit.sidebar_width = Some(actual_w);
        if ctx.input(|i| i.pointer.any_released()) {
            let _ = app.settings_store.settings_edit.save();
        }
    }
}
