use crate::ui::types::AgentStatus;
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
    egui::SidePanel::left("sidebar")
        .default_width(SIDEBAR_WIDTH)
        .min_width(220.0)
        .max_width(360.0)
        .resizable(true)
        .frame(
            egui::Frame::new()
                .fill(app.ui_store.theme.bg)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("sidebar_scroll")
                .show(ui, |ui| {
                    // ── Top toolbar: collapse + global controls ──
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        // Collapse sidebar (left)
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(crate::theme::ICON_ARROW_LEFT)
                                        .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_base)),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(
                                    app.ui_store.theme.radius_md as u8,
                                )),
                            )
                            .clicked()
                        {
                            app.ui_store.sidebar_collapsed = true;
                        }

                        // Global controls (right-aligned)
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
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

                                // MCP
                                let mcp_count = app
                                    .mcp_store
                                    .mcp_config
                                    .as_ref()
                                    .map_or(0, |c| c.servers.len());
                                let mcp_icon = if mcp_count > 0 {
                                    format!("🔌{}", mcp_count)
                                } else {
                                    "🔌".to_string()
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(&mcp_icon)
                                                .size(app.ui_store.theme.text_sm),
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

                                // Skills
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("🛠")
                                                .size(app.ui_store.theme.text_sm),
                                        )
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(
                                            app.ui_store.theme.radius_sm as u8,
                                        )),
                                    )
                                    .clicked()
                                {
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
                                        .corner_radius(egui::CornerRadius::same(
                                            app.ui_store.theme.radius_sm as u8,
                                        )),
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

                                // Online status
                                let (status_color, status_label) =
                                    match app.chat_store.agent_status {
                                        AgentStatus::Online => {
                                            (app.ui_store.theme.status_online, "Online")
                                        }
                                        AgentStatus::Busy => {
                                            (app.ui_store.theme.status_busy, "Busy")
                                        }
                                        AgentStatus::Unconfigured => {
                                            (app.ui_store.theme.status_offline, "Unconfigured")
                                        }
                                        AgentStatus::Offline => {
                                            (app.ui_store.theme.status_offline, "Offline")
                                        }
                                    };
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(8.0, 8.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(rect.center(), 4.0, status_color);
                                ui.label(
                                    egui::RichText::new(status_label)
                                        .size(app.ui_store.theme.text_xs)
                                        .color(app.ui_store.theme.text_dim),
                                );
                            },
                        );
                    });
                    ui.add_space(app.ui_store.theme.space_12);

                    // ── Category navigation ──
                    let categories = [("emotion", app.t("Emotion")), ("knowledge", app.t("Knowledge")), ("engineering", app.t("Engineering"))];
                    for (cat, label) in categories {
                        let is_active = app.session_store.active_category == cat;
                        let stroke = if is_active {
                            egui::Stroke::new(1.5, app.ui_store.theme.accent)
                        } else {
                            egui::Stroke::NONE
                        };
                        let btn_resp = ui.add(
                            egui::Button::new("")
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                                .stroke(stroke)
                                .min_size(egui::vec2(ui.available_width(), 32.0)),
                        );
                        let text_color = if is_active || btn_resp.hovered() {
                            app.ui_store.theme.text
                        } else {
                            app.ui_store.theme.text_dim
                        };
                        ui.painter().text(
                            btn_resp.rect.center(),
                            egui::Align2::CENTER_CENTER,
                            label,
                            app.ui_store.theme.font(app.ui_store.theme.text_base),
                            text_color,
                        );
                        if btn_resp.clicked() {
                            app.switch_category(cat);
                        }
                    }
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Web Tabs ──
                    crate::components::web_tabs::render_web_tabs(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Tools / Tasks (migrated from right panel) ──
                    crate::components::tools_section::render_tools_section(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);

                    // ── Thinking Log ──
                    crate::components::thinking_log::render_thinking_log(app, ui);
                    ui.add_space(app.ui_store.theme.space_16);


                });
        });
}
