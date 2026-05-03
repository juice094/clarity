use crate::App;

pub fn render_task_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::right("task_panel")
        .default_width(320.0)
        .min_width(240.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(app.ui_store.theme.bg)
                .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Workspace")
                        .size(app.ui_store.theme.text_lg)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
            });

            // ---- Files tree (full height, preview moves to main chat area) ----
            ui.add_space(app.ui_store.theme.space_12);
            let mut clicked_file: Option<std::path::PathBuf> = None;
            let tree_max_h = (ui.available_height() - 40.0).max(120.0);
            egui::ScrollArea::vertical()
                .id_salt("file_tree_scroll_right")
                .max_height(tree_max_h)
                .show(ui, |ui| {
                    if let Ok(cwd) = std::env::current_dir() {
                        let selected_path = app.ui_store.preview_item.as_ref().and_then(|p| {
                            match p {
                                crate::ui::types::PreviewItem::File { path, .. } => Some(path.as_str()),
                                _ => None,
                            }
                        });
                        crate::ui::file_browser::render_file_tree(
                            ui,
                            &cwd,
                            &app.ui_store.theme,
                            0,
                            selected_path,
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
                app.ui_store.preview_item = content.map(|c| crate::ui::types::PreviewItem::File {
                    name: name.clone(),
                    content: c,
                    path: path.to_string_lossy().to_string(),
                });
            }
        });
}
