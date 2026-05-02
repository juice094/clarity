use crate::App;

pub fn render_task_panel(app: &mut App, ctx: &egui::Context) {
    if !app.task_store.task_panel_open {
        return;
    }
    egui::SidePanel::right("task_panel")
        .default_width(260.0)
        .min_width(200.0)
        .max_width(320.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(app.ui_store.theme.bg_accent)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border)),
        )
        .show(ctx, |ui| {
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tools")
                        .size(app.ui_store.theme.text_lg)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
            });

            // ---- Regular task list ----
            let action = crate::ui::task_panel::render_task_panel(ui, &app.task_store.tasks, &app.ui_store.theme);
            if let crate::ui::task_panel::TaskPanelAction::Cancel(task_id) = action {
                let store = app.state.task_store.clone();
                app.runtime.spawn(async move {
                    if let Err(e) = store
                        .update_status(&task_id, clarity_core::background::TaskStatus::Cancelled)
                        .await
                    {
                        tracing::warn!("Failed to cancel task {}: {}", task_id, e);
                    }
                });
            }

            // ---- Create task button ----
            ui.add_space(app.ui_store.theme.space_8);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("+ Create Task")
                            .size(app.ui_store.theme.text_base)
                            .color(app.ui_store.theme.accent),
                    )
                    .fill(app.ui_store.theme.accent_subtle)
                    .stroke(egui::Stroke::new(1.0, app.ui_store.theme.accent))
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                    .min_size(egui::vec2(ui.available_width(), 36.0)),
                )
                .clicked()
            {
                app.task_store.task_create_modal_open = true;
            }

            // ---- SubAgent parallel progress ----
            ui.add_space(app.ui_store.theme.space_12);
            egui::Frame::group(ui.style())
                .fill(app.ui_store.theme.bg)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    crate::panels::subagent_progress::render_subagent_progress(app, ui);
                });

            // ---- Files (moved from sidebar) ----
            ui.add_space(app.ui_store.theme.space_16);
            ui.label(
                egui::RichText::new("Files")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim)
                    .weak(),
            );
            ui.add_space(app.ui_store.theme.space_4);
            let mut clicked_file: Option<std::path::PathBuf> = None;
            let files_height = (ui.available_height() * 0.3).max(100.0);
            egui::ScrollArea::vertical()
                .id_salt("file_tree_scroll_right")
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
                        if ui.add(egui::Button::new(egui::RichText::new(crate::theme::ICON_X).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_xs))).small()).clicked() {
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
                    .id_salt("preview_scroll_right")
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
        });
}
