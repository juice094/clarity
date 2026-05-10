//! Right-side Workspace panel — file browser + drawer-style file preview.
//!
//! Drawer design (Kimi-style):
//!   - Default: full-width file tree (280px)
//!   - Click file: file tree shrinks to 60px icon strip, preview drawer slides in
//!   - Click drawer ✕: drawer closes, file tree restores full width

use crate::App;

pub fn render_workspace_panel(app: &mut App, ctx: &egui::Context) {
    // ── Clone theme to free app borrow for closures ──
    let theme = app.ui_store.theme.clone();
    let bg = theme.bg;
    let text_xs = theme.text_xs;
    let text_dim = theme.text_dim;
    let space_4 = theme.space_4;

    // Auto-expand plan section when a plan becomes active; auto-fold when plan ends.
    let plan_active =
        app.chat_store.pending_plan.is_some() || app.chat_store.plan_tracker.is_some();
    if plan_active && !app.ui_store.workspace_plan_manually_collapsed {
        app.ui_store.workspace_plan_expanded = true;
    } else if !plan_active {
        app.ui_store.workspace_plan_expanded = false;
        app.ui_store.workspace_plan_manually_collapsed = false;
    }

    let has_preview = app.ui_store.preview_item.is_some() && app.ui_store.preview_drawer_open;

    egui::SidePanel::right("workspace_panel")
        .default_width(280.0)
        .min_width(200.0)
        .max_width(480.0)
        .resizable(true)
        .frame(
            egui::Frame::new()
                .fill(bg)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            // ── Workspace header ──
            ui.label(
                egui::RichText::new("WORKSPACE")
                    .size(text_xs)
                    .color(text_dim),
            );
            ui.separator();
            ui.add_space(space_4);

            let work_dir = app.state.agent.config().working_dir.clone();
            let selected_path: Option<String> =
                app.ui_store.preview_item.as_ref().and_then(|p| match p {
                    crate::ui::types::PreviewItem::File { path, .. } => Some(path.clone()),
                    _ => None,
                });
            let selected_path_ref = selected_path.as_deref();

            // ── Horizontal split: file tree + preview drawer ──
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                // 1. File tree (full width or compact 80px icon strip)
                let tree_width = if has_preview {
                    80.0
                } else {
                    ui.available_width()
                };
                ui.allocate_ui_with_layout(
                    egui::vec2(tree_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let has_plan = plan_active
                            && app.ui_store.workspace_plan_expanded
                            && !has_preview;
                        let max_tree_h = if has_plan {
                            ui.available_height() * 0.55
                        } else {
                            ui.available_height()
                        };
                        egui::ScrollArea::vertical()
                            .id_salt(ui.id().with("file_tree"))
                            .auto_shrink([false, true])
                            .max_height(max_tree_h)
                            .show(ui, |ui| {
                                let ctx = ui.ctx().clone();
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
                                                    path: path.to_string_lossy().to_string(),
                                                });
                                            app.ui_store.preview_drawer_open = true;
                                            ctx.request_repaint();
                                        } else {
                                            app.push_toast(
                                                format!("Failed to read {}", path.to_string_lossy()),
                                                crate::ui::types::ToastLevel::Error,
                                            );
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

    let (title, is_web, file_path) = match preview {
        crate::ui::types::PreviewItem::File { name, path, .. } => {
            (name.clone(), false, Some(path.clone()))
        }
        crate::ui::types::PreviewItem::WebPage { title, .. } => (title.clone(), true, None),
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
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm.round() as u8)),
                )
                .clicked()
            {
                app.ui_store.preview_drawer_open = false;
                app.ui_store.preview_item = None;
            }

            if !is_web {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_CHECK)
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm.round() as u8))
                        .frame(false),
                    )
                    .on_hover_text("Save & git add")
                    .clicked()
                {
                    if let crate::ui::types::PreviewItem::File { content, path, .. } =
                        app.ui_store.preview_item.as_ref().unwrap()
                    {
                        let work_dir = app.state.agent.config().working_dir.clone();
                        match std::fs::write(path, content) {
                            Ok(_) => {
                                let _ = std::process::Command::new("git")
                                    .args(["add", path])
                                    .current_dir(&work_dir)
                                    .status();
                                app.push_toast(
                                    format!("Saved {}", path),
                                    crate::ui::types::ToastLevel::Info,
                                );
                            }
                            Err(e) => {
                                app.push_toast(
                                    format!("Save failed: {}", e),
                                    crate::ui::types::ToastLevel::Error,
                                );
                            }
                        }
                    }
                }
            }
        });
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Content area ──
    let is_code_file = !is_web && {
        let ext = file_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).extension())
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

    if let Some(preview) = app.ui_store.preview_item.as_mut() {
        egui::Frame::new()
            .fill(theme.code_block_bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm.round() as u8))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                match preview {
                    crate::ui::types::PreviewItem::File { content, .. } => {
                        if is_code_file {
                            ui.add_sized(
                                ui.available_size(),
                                egui::TextEdit::multiline(content)
                                    .font(egui::FontId::monospace(theme.text_sm))
                                    .lock_focus(true)
                                    .frame(false),
                            );
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt(ui.id().with("preview_scroll"))
                                .auto_shrink([false; 2])
                                .scroll_bar_visibility(
                                    egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible,
                                )
                                .show(ui, |ui| {
                                    let parsed = crate::ui::markdown::parse_markdown(content);
                                    crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
                                });
                        }
                    }
                    crate::ui::types::PreviewItem::WebPage { content, .. } => {
                        egui::ScrollArea::vertical()
                            .id_salt(ui.id().with("preview_scroll"))
                            .auto_shrink([false; 2])
                            .scroll_bar_visibility(
                                egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .show(ui, |ui| {
                                let parsed = crate::ui::markdown::parse_markdown(content);
                                crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
                            });
                    }
                }
            });
    }
}
