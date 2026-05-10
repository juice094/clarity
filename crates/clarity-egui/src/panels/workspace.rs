//! Right-side Workspace panel — file browser + drawer-style file preview.
//!
//! Drawer design (Kimi-style):
//!   - Default: full-width file tree (280px)
//!   - Click file: file tree shrinks to 60px icon strip, preview drawer slides in
//!   - Click drawer ✕: drawer closes, file tree restores full width

use crate::App;

pub fn render_workspace_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    // Auto-expand plan section when a plan becomes active (unless user manually collapsed)
    let plan_active =
        app.chat_store.pending_plan.is_some() || app.chat_store.plan_tracker.is_some();
    if plan_active && !app.ui_store.workspace_plan_manually_collapsed {
        app.ui_store.workspace_plan_expanded = true;
    }

    let has_preview = app.ui_store.preview_item.is_some() && app.ui_store.preview_drawer_open;

    egui::SidePanel::right("workspace_panel")
        .default_width(280.0)
        .min_width(200.0)
        .max_width(480.0)
        .resizable(true)
        .frame(
            egui::Frame::new()
                .fill(theme.bg)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            // ── Workspace header ──
            ui.label(
                egui::RichText::new("WORKSPACE")
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            );
            ui.separator();
            ui.add_space(theme.space_4);

            let work_dir = app.state.agent.config().working_dir.clone();
            let selected_path: Option<String> =
                app.ui_store.preview_item.as_ref().and_then(|p| match p {
                    crate::ui::types::PreviewItem::File { path, .. } => Some(path.clone()),
                    _ => None,
                });
            let selected_path_ref = selected_path.as_deref();

            // ── Horizontal split: file tree + preview drawer ──
            ui.horizontal(|ui| {
                // 1. File tree (full width or compact 80px icon strip)
                let tree_width = if has_preview {
                    80.0
                } else {
                    ui.available_width()
                };
                ui.allocate_ui_with_layout(
                    egui::vec2(tree_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let has_plan = plan_active && app.ui_store.workspace_plan_expanded;
                        let max_tree_h = if has_plan {
                            ui.available_height() * 0.55
                        } else {
                            ui.available_height()
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("workspace_file_tree")
                            .auto_shrink([true, true])
                            .max_height(max_tree_h)
                            .show(ui, |ui| {
                            crate::ui::file_browser::render_file_tree(
                                ui,
                                &work_dir,
                                &theme,
                                0,
                                selected_path_ref,
                                &mut |path| {
                                    app.state
                                        .agent
                                        .set_active_file_paths(vec![path.to_path_buf()]);
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
                                        app.ui_store.preview_drawer_open = true;
                                    }
                                },
                                has_preview,
                            );
                        });

                        // Plan section at bottom (only in full-width mode)
                        if !has_preview {
                            crate::panels::workspace_plan::render_workspace_plan(app, ui);
                        }
                    },
                );

                // 2. Preview drawer
                if has_preview {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            render_preview_drawer(app, ui, &theme);
                        },
                    );
                }
            });
        });
}

/// Render the preview drawer inside the workspace panel.
fn render_preview_drawer(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let Some(preview) = app.ui_store.preview_item.as_ref() else {
        return;
    };

    let (title, is_web) = match preview {
        crate::ui::types::PreviewItem::File { name, .. } => (name.clone(), false),
        crate::ui::types::PreviewItem::WebPage { title, .. } => (title.clone(), true),
    };

    // ── Drawer header ──
    ui.horizontal(|ui| {
        if is_web {
            let (globe_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            if ui.is_rect_visible(globe_rect) {
                crate::ui::icons::paint_globe(&ui.painter_at(globe_rect), globe_rect.center(), theme.text);
            }
            ui.add_space(4.0);
        } else {
            ui.label(
                egui::RichText::new(crate::theme::ICON_PAPERCLIP)
                    .font(theme.font_icon(theme.text_sm))
                    .color(theme.text),
            );
            ui.add_space(4.0);
        }
        ui.label(
            egui::RichText::new(&title)
                .size(theme.text_sm)
                .color(theme.text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(crate::theme::ICON_X)
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                )
                .clicked()
            {
                app.ui_store.preview_drawer_open = false;
            }
        });
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Content area ──
    let is_code_file = !is_web && {
        let ext = std::path::Path::new(&title)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        matches!(
            ext,
            "rs" | "toml"
                | "json" | "yaml" | "yml" | "py" | "js" | "ts" | "sh" | "ps1"
                | "cpp" | "c" | "h" | "hpp" | "go" | "java" | "kt" | "swift"
                | "rb" | "php" | "html" | "css" | "scss" | "xml" | "sql"
        )
    };

    let preview = app.ui_store.preview_item.as_mut().unwrap();
    egui::Frame::new()
        .fill(theme.code_block_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("workspace_preview_drawer_scroll")
                .auto_shrink([false; 2])
                .scroll_bar_visibility(
                    egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible,
                )
                .show(ui, |ui| {
                    match preview {
                        crate::ui::types::PreviewItem::File { content, .. } => {
                            if is_code_file {
                                ui.add(
                                    egui::TextEdit::multiline(content)
                                        .font(egui::FontId::monospace(theme.text_sm))
                                        .desired_rows(20)
                                        .lock_focus(true)
                                        .frame(false),
                                );
                            } else {
                                let parsed = crate::ui::markdown::parse_markdown(content);
                                crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
                            }
                        }
                        crate::ui::types::PreviewItem::WebPage { content, .. } => {
                            let parsed = crate::ui::markdown::parse_markdown(content);
                            crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
                        }
                    }
                });
        });
}
