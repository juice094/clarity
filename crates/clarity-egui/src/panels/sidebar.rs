use crate::{App, SIDEBAR_WIDTH};

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
                    ui.add_space(app.ui_store.theme.space_12);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new(crate::theme::ICON_ARROW_LEFT).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_base)))
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8)),
                                )
                                .clicked()
                            {
                                app.ui_store.sidebar_collapsed = true;
                            }
                        });
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
                        let text_color = if is_active {
                            app.ui_store.theme.text
                        } else if btn_resp.hovered() {
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

                    // ── Bottom bar: Skills + Locale + Token usage ──
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                egui::RichText::new("🛠 Skills")
                                    .size(app.ui_store.theme.text_sm)
                                    .color(app.ui_store.theme.text),
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
                            .button(
                                egui::RichText::new(locale_label)
                                    .size(app.ui_store.theme.text_xs)
                                    .color(app.ui_store.theme.text_dim)
                                    .monospace(),
                            )
                            .clicked()
                        {
                            app.ui_store.locale = match app.ui_store.locale {
                                crate::i18n::Locale::EnUS => crate::i18n::Locale::ZhCN,
                                crate::i18n::Locale::ZhCN => crate::i18n::Locale::EnUS,
                            };
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some((_, _, t)) = app.chat_store.last_usage {
                                ui.label(
                                    egui::RichText::new(format!("{}∑", t))
                                        .size(app.ui_store.theme.text_xs)
                                        .color(app.ui_store.theme.text_dim)
                                        .monospace(),
                                );
                            }
                            // FPS debug display removed — use tracing instead
                        });
                    });
                    ui.add_space(app.ui_store.theme.space_8);
                });
        });
}
