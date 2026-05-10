//! Right-side Workspace panel — file browser + inline file preview.
//!
//! Replaces the legacy task panel (tasks now live exclusively in the sidebar
//! Tools section).  Files can be browsed and previewed here while the user
//! continues chatting in the central panel.

use crate::ui::types::UiEvent;
use crate::App;

pub fn render_workspace_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    // Auto-expand plan section when a plan becomes active (unless user manually collapsed)
    let plan_active = app.chat_store.pending_plan.is_some() || app.chat_store.plan_tracker.is_some();
    if plan_active && !app.ui_store.workspace_plan_manually_collapsed {
        app.ui_store.workspace_plan_expanded = true;
    }

    egui::SidePanel::right("workspace_panel")
        .default_width(280.0)
        .min_width(200.0)
        .max_width(480.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(theme.bg)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.add_space(theme.space_12);

            // ── Workspace title (minimal) ──
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Workspace")
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
            });
            ui.add_space(theme.space_8);

            let work_dir = app.state.agent.config().working_dir.clone();
            let selected_path: Option<String> =
                app.ui_store.preview_item.as_ref().and_then(|p| match p {
                    crate::ui::types::PreviewItem::File { path, .. } => Some(path.clone()),
                    _ => None,
                });
            let selected_path_ref = selected_path.as_deref();

            // ── File tree (scrollable, full height) ──
            let has_plan = plan_active && app.ui_store.workspace_plan_expanded;
            let mut scroll = egui::ScrollArea::vertical().id_salt("workspace_file_tree");
            if has_plan {
                scroll = scroll.max_height(ui.available_height() * 0.55);
            }
            scroll.show(ui, |ui| {
                crate::ui::file_browser::render_file_tree(
                    ui,
                    &work_dir,
                    &theme,
                    0,
                    selected_path_ref,
                    &mut |path| {
                        app.state.agent.set_active_file_paths(vec![path.to_path_buf()]);
                        if let Ok(content) = std::fs::read_to_string(path) {
                            app.ui_store.preview_item = Some(crate::ui::types::PreviewItem::File {
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

            // ── Plan foldable section (bottom) ──
            crate::panels::workspace_plan::render_workspace_plan(app, ui);
        });
}

/// Render file preview as a floating popup window instead of inline inside workspace.
/// This prevents the preview from squeezing the file tree and central chat area.
pub fn render_file_preview_window(app: &mut App, ctx: &egui::Context) {
    let Some(ref preview) = app.ui_store.preview_item else {
        return;
    };

    let (title, content, is_web) = match preview {
        crate::ui::types::PreviewItem::File { name, content, .. } => {
            (name.clone(), content.clone(), false)
        }
        crate::ui::types::PreviewItem::WebPage { title, content, .. } => {
            (title.clone(), content.clone(), true)
        }
    };

    let theme = app.ui_store.theme.clone();
    let mut open = true;

    let icon = if is_web { "🌐" } else { crate::theme::ICON_PAPERCLIP };
    let window_title = format!("{} {}", icon, title);

    egui::Window::new(&window_title)
        .id(egui::Id::new("file_preview_popup"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_pos(egui::pos2(
            ctx.screen_rect().max.x - 560.0,
            52.0, // below titlebar (36 + 16 padding)
        ))
        .default_size([520.0, 580.0])
        .min_size([320.0, 240.0])
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.bg_elevated)
                .stroke(egui::Stroke::NONE),
        )
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(theme.space_4);
                egui::Frame::new()
                    .fill(theme.code_block_bg)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("file_preview_popup_scroll")
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
            });
        });

    if !open {
        app.ui_store.preview_item = None;
    }
}
