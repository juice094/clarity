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
                .fill(app.ui_store.theme.bg_accent)
                .inner_margin(egui::Margin::same(4)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Clarity")
                        .size(app.ui_store.theme.text_xl)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("⬅").size(app.ui_store.theme.text_base))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.ui_store.sidebar_collapsed = true;
                    }
                });
            });
            ui.add_space(app.ui_store.theme.space_16);

            // ── Fixed category list (vertical) ──
            let categories = [("emotion", app.t("Emotion")), ("knowledge", app.t("Knowledge")), ("engineering", app.t("Engineering"))];
            for (cat, label) in categories {
                let is_active = app.session_store.active_category == cat;
                let bg = if is_active {
                    app.ui_store.theme.surface
                } else {
                    app.ui_store.theme.bg_accent
                };
                let text_color = if is_active {
                    app.ui_store.theme.text
                } else {
                    app.ui_store.theme.text_dim
                };
                let stroke = if is_active {
                    egui::Stroke::new(2.0, app.ui_store.theme.accent)
                } else {
                    egui::Stroke::NONE
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(label).size(app.ui_store.theme.text_base).color(text_color),
                        )
                        .fill(bg)
                        .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                        .stroke(stroke)
                        .min_size(egui::vec2(ui.available_width(), 36.0)),
                    )
                    .clicked()
                {
                    app.switch_category(cat);
                }
            }
            ui.add_space(app.ui_store.theme.space_12);

            // ── Session list for active category ──
            ui.label(
                egui::RichText::new("Sessions")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim)
                    .weak(),
            );
            ui.add_space(app.ui_store.theme.space_4);
            let session_list_height = (ui.available_height() * 0.25).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("session_list_scroll")
                .max_height(session_list_height)
                .show(ui, |ui| {
                    let active_cat = app.session_store.active_category.clone();
                    let category_sessions: Vec<(String, String)> = app
                        .session_store
                        .sessions
                        .iter()
                        .filter(|s| s.category == active_cat)
                        .map(|s| (s.id.clone(), s.title.clone()))
                        .collect();
                    if category_sessions.is_empty() {
                        ui.label(
                            egui::RichText::new("No sessions")
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim),
                        );
                    } else {
                        for (id, title) in &category_sessions {
                            let is_active = id == &app.session_store.active_session_id;
                            let text_color = if is_active {
                                app.ui_store.theme.text_strong
                            } else {
                                app.ui_store.theme.text_dim
                            };
                            let bg = if is_active {
                                app.ui_store.theme.surface
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let session_id = id.clone();
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(title)
                                            .size(app.ui_store.theme.text_sm)
                                            .color(text_color),
                                    )
                                    .fill(bg)
                                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                                    .min_size(egui::vec2(ui.available_width(), 28.0)),
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
                                app.session_store.active_session_id = session_id.clone();
                                app.chat_store.input = app.session_store.drafts.remove(&session_id).unwrap_or_default();
                            }
                        }
                    }
                    // New session quick button
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("+ New Session")
                                    .size(app.ui_store.theme.text_xs)
                                    .color(app.ui_store.theme.accent),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                            .min_size(egui::vec2(ui.available_width(), 24.0)),
                        )
                        .clicked()
                    {
                        app.new_session();
                    }
                });

            ui.add_space(app.ui_store.theme.space_16);
            ui.label(
                egui::RichText::new("Files")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim)
                    .weak(),
            );
            ui.add_space(app.ui_store.theme.space_4);
            let mut clicked_file: Option<std::path::PathBuf> = None;
            let files_height = (ui.available_height() * 0.25).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("file_tree_scroll")
                .max_height(files_height)
                .show(ui, |ui| {
                    if let Ok(cwd) = std::env::current_dir() {
                        crate::ui::file_browser::render_file_tree(
                            ui,
                            &cwd,
                            &app.ui_store.theme,
                            0,
                            &mut |path| {
                                clicked_file = Some(path.to_path_buf());
                            },
                        );
                    }
                });
            if let Some(path) = clicked_file {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let content = std::fs::read_to_string(&path).ok();
                app.ui_store.preview_file = content.map(|c| (name, c));
            }

            // File preview panel
            if let Some((ref name, ref content)) = app.ui_store.preview_file {
                let preview_name = name.clone();
                let preview_content = content.clone();
                ui.add_space(app.ui_store.theme.space_12);
                ui.separator();
                ui.add_space(app.ui_store.theme.space_8);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Preview")
                            .size(app.ui_store.theme.text_sm)
                            .color(app.ui_store.theme.text_dim)
                            .weak(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
                            app.ui_store.preview_file = None;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(&preview_name)
                        .size(app.ui_store.theme.text_sm)
                        .color(app.ui_store.theme.text)
                        .monospace(),
                );
                ui.add_space(app.ui_store.theme.space_4);
                let mut preview_text = if preview_content.chars().count() > 2000 {
                    let truncated: String = preview_content.chars().take(2000).collect();
                    format!(
                        "{}…\n\n[Preview truncated: {} total characters]",
                        truncated,
                        preview_content.len()
                    )
                } else {
                    preview_content
                };
                egui::ScrollArea::vertical()
                    .id_salt("preview_scroll")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        ui.add_sized(
                            egui::vec2(ui.available_width(), 180.0),
                            egui::TextEdit::multiline(&mut preview_text)
                                .desired_rows(10)
                                .font(egui::TextStyle::Monospace)
                                .text_color(app.ui_store.theme.text_dim)
                                .margin(egui::vec2(8.0, 6.0)),
                        );
                    });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(app.ui_store.theme.space_8);
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
                        #[cfg(debug_assertions)]
                        ui.label(
                            egui::RichText::new(format!("FPS: {:.0}", app.ui_store.fps))
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim),
                        );
                    });
                });
            });
        });
}
