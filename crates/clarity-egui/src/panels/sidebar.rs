use crate::{App, SIDEBAR_WIDTH};

pub fn render_sidebar(app: &mut App, ctx: &egui::Context) {
    if app.sidebar_collapsed {
        return;
    }
    egui::SidePanel::left("sidebar")
        .default_width(SIDEBAR_WIDTH)
        .min_width(220.0)
        .max_width(360.0)
        .resizable(true)
        .frame(
            egui::Frame::new()
                .fill(app.theme.bg_accent)
                .inner_margin(egui::Margin::same(4)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Clarity")
                        .size(18.0)
                        .strong()
                        .color(app.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("⬅").size(14.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.sidebar_collapsed = true;
                    }
                });
            });
            ui.add_space(16.0);

            // ── Fixed category tabs ──
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let categories = [("emotion", "情感"), ("knowledge", "知识"), ("engineering", "工程")];
                for (cat, label) in categories {
                    let is_active = app.active_category == cat;
                    let bg = if is_active {
                        app.theme.accent
                    } else {
                        app.theme.surface
                    };
                    let text_color = if is_active {
                        app.theme.text_strong
                    } else {
                        app.theme.text
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(label).size(13.0).color(text_color),
                            )
                            .fill(bg)
                            .corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))
                            .min_size(egui::vec2(70.0, 36.0)),
                        )
                        .clicked()
                    {
                        app.switch_category(cat);
                    }
                }
            });
            ui.add_space(12.0);

            // New-session button for the current category
            let new_label = match app.active_category.as_str() {
                "emotion" => "+ 新建情感",
                "knowledge" => "+ 新建知识",
                "engineering" => "+ 新建工程",
                _ => "+ New Chat",
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(new_label)
                            .size(13.0)
                            .color(app.theme.text),
                    )
                    .fill(app.theme.surface)
                    .corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))
                    .min_size(egui::vec2(ui.available_width(), 36.0)),
                )
                .clicked()
            {
                app.new_session();
            }
            ui.add_space(12.0);

            ui.add_space(16.0);
            ui.label(
                egui::RichText::new("Files")
                    .size(11.0)
                    .color(app.theme.text_dim)
                    .weak(),
            );
            ui.add_space(4.0);
            let mut clicked_file: Option<std::path::PathBuf> = None;
            let files_height = (ui.available_height() - 260.0).max(100.0);
            egui::ScrollArea::vertical()
                .id_salt("file_tree_scroll")
                .max_height(files_height)
                .show(ui, |ui| {
                    if let Ok(cwd) = std::env::current_dir() {
                        crate::ui::file_browser::render_file_tree(
                            ui,
                            &cwd,
                            &app.theme,
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
                app.preview_file = content.map(|c| (name, c));
            }

            // File preview panel
            if let Some((ref name, ref content)) = app.preview_file {
                let preview_name = name.clone();
                let preview_content = content.clone();
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Preview")
                            .size(11.0)
                            .color(app.theme.text_dim)
                            .weak(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
                            app.preview_file = None;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(&preview_name)
                        .size(12.0)
                        .color(app.theme.text)
                        .monospace(),
                );
                ui.add_space(4.0);
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
                                .text_color(app.theme.text_dim)
                                .margin(egui::vec2(8.0, 6.0)),
                        );
                    });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new("🛠 Skills")
                                .size(11.0)
                                .color(app.theme.text),
                        )
                        .clicked()
                    {
                        app.skill_panel_open = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some((_, _, t)) = app.last_usage {
                            ui.label(
                                egui::RichText::new(format!("{}∑", t))
                                    .size(10.0)
                                    .color(app.theme.text_dim)
                                    .monospace(),
                            );
                        }
                        #[cfg(debug_assertions)]
                        ui.label(
                            egui::RichText::new(format!("FPS: {:.0}", app.fps))
                                .size(10.0)
                                .color(app.theme.text_dim),
                        );
                    });
                });
            });
        });
}
