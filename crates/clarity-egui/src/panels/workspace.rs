//! Right-side Workspace panel — file browser + inline file preview.
//!
//! Replaces the legacy task panel (tasks now live exclusively in the sidebar
//! Tools section).  Files can be browsed and previewed here while the user
//! continues chatting in the central panel.

use crate::App;

pub fn render_workspace_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    egui::SidePanel::right("workspace_panel")
        .default_width(320.0)
        .min_width(240.0)
        .max_width(480.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(theme.bg)
                .stroke(egui::Stroke::new(1.0, theme.border))
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.add_space(theme.space_12);
            ui.label(
                egui::RichText::new("Workspace")
                    .size(theme.text_lg)
                    .strong()
                    .color(theme.text),
            );
            ui.add_space(theme.space_12);

            let work_dir = app.state.agent.config().working_dir.clone();
            let selected_path: Option<String> = app
                .ui_store
                .preview_item
                .as_ref()
                .and_then(|p| match p {
                    crate::ui::types::PreviewItem::File { path, .. } => Some(path.clone()),
                    _ => None,
                });
            let selected_path_ref = selected_path.as_deref();

            // ── File tree (top half, scrollable) ──
            egui::ScrollArea::vertical()
                .id_salt("workspace_file_tree")
                .max_height(ui.available_height() * 0.45)
                .show(ui, |ui| {
                    crate::ui::file_browser::render_file_tree(
                        ui,
                        &work_dir,
                        &theme,
                        0,
                        selected_path_ref,
                        &mut |path| {
                            if let Ok(content) = std::fs::read_to_string(path) {
                                app.ui_store.preview_item =
                                    Some(crate::ui::types::PreviewItem::File {
                                        name: path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_default(),
                                        content,
                                        path: path.display().to_string(),
                                    });
                            }
                        },
                    );
                });

            ui.add_space(theme.space_12);

            // ── File preview (bottom half, inline) ──
            if let Some(ref preview) = app.ui_store.preview_item {
                let (title, content, is_web) = match preview {
                    crate::ui::types::PreviewItem::File { name, content, .. } => {
                        (name.clone(), content.clone(), false)
                    }
                    crate::ui::types::PreviewItem::WebPage {
                        title, content, ..
                    } => (title.clone(), content.clone(), true),
                };

                ui.horizontal(|ui| {
                    let icon = if is_web { "🌐" } else { crate::theme::ICON_PAPERCLIP };
                    ui.label(
                        egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)),
                    );
                    ui.label(
                        egui::RichText::new(&title)
                            .size(theme.text_sm)
                            .strong()
                            .color(theme.text)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(crate::theme::ICON_X)
                                        .font(theme.font_icon(theme.text_base)),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(
                                    theme.radius_sm as u8,
                                )),
                            )
                            .clicked()
                        {
                            app.ui_store.preview_item = None;
                        }
                    });
                });

                ui.add_space(theme.space_8);

                egui::Frame::new()
                    .fill(theme.code_block_bg)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("workspace_preview_scroll")
                            .show(ui, |ui| {
                                let parsed = crate::ui::markdown::parse_markdown(&content);
                                crate::ui::markdown::render_blocks(
                                    ui,
                                    &parsed,
                                    &theme,
                                    theme.chat_text,
                                );
                            });
                    });
            }
        });
}
