//! Files panel — local project file browser for the right IDE rail.
//!
//! Renders a recursive directory tree rooted at the workspace directory
//! with file click-to-preview, right-click context menus, and recent-file
//! tracking.  Extension points for future GitHub integration are rendered
//! as disabled UI elements with "Coming soon" tooltips.

use crate::App;
use std::path::Path;

/// Render the files panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    // --- path bar ---
    let root = app.files_store.workspace_root.clone();
    let root_label = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| app.t("Workspace").to_string());

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} {}", crate::theme::ICON_FOLDER_OPEN, root_label))
                .size(theme.text_sm)
                .strong()
                .color(theme.text_strong),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_button_toolbar(
                ui,
                crate::theme::ICON_REFRESH,
                theme.text_sm,
                &theme,
            )
            .on_hover_text(app.t("Refresh file tree"))
            .clicked()
            {
                // Force a fresh read of the directory listing.
                app.files_store.expanded_dirs.clear();
            }
        });

        // GitHub URL badge — extension point.
        if let Some(ref repo_url) = app.files_store.repo_url.clone() {
            ui.add_space(theme.space_8);
            let badge = egui::Frame::new()
                .fill(theme.accent_subtle)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .inner_margin(egui::Margin::symmetric(
                    theme.space_8 as i8,
                    theme.space_4 as i8,
                ))
                .show(ui, |ui| {
                    let short = repo_url
                        .trim_start_matches("https://github.com/")
                        .trim_end_matches(".git");
                    ui.label(
                        egui::RichText::new(short)
                            .size(theme.text_xs)
                            .color(theme.accent),
                    );
                });
            if badge.response.clicked() {
                let _ = webbrowser::open(repo_url);
            }
        }
    });

    // --- git status bar (extension point) ---
    if let Some(ref git) = app.files_store.git_status {
        ui.add_space(theme.space_4);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("⎇ {}", git.branch))
                    .size(theme.text_xs)
                    .color(theme.accent),
            );
            if !git.modified.is_empty() {
                ui.label(
                    egui::RichText::new(format!("M:{}", git.modified.len()))
                        .size(theme.text_xs)
                        .color(theme.warn),
                );
            }
            if !git.staged.is_empty() {
                ui.label(
                    egui::RichText::new(format!("A:{}", git.staged.len()))
                        .size(theme.text_xs)
                        .color(theme.ok),
                );
            }
            if !git.untracked.is_empty() {
                ui.label(
                    egui::RichText::new(format!("U:{}", git.untracked.len()))
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
            }
        });
    }

    ui.add_space(theme.space_4);

    // --- dir tree ---
    let selected = app
        .files_store
        .selected_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());

    let root_path = app.files_store.workspace_root.clone();
    let mut secondary_click = app.files_store.selected_path.as_ref().map(|p| p.clone());

    crate::ui::file_browser::render_file_tree(
        ui,
        &root_path,
        &theme,
        0,
        selected.as_deref(),
        &mut |path: &Path| {
            app.files_store.touch_recent(path.to_path_buf());
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            let content = std::fs::read_to_string(path).unwrap_or_default();
            app.ui_store.preview_item = Some(crate::ui::types::PreviewItem::File {
                name: file_name,
                content,
                path: path.to_string_lossy().into_owned(),
            });
            app.files_store.selected_path = Some(path.to_path_buf());
        },
        &mut |path: &Path| {
            // Right-click: set as secondary click target for context menu.
            secondary_click = Some(path.to_path_buf());
        },
        false,
        None, // name_filter
    );

    // --- recent files ---
    if !app.files_store.recent_files.is_empty() {
        ui.add_space(theme.space_12);
        ui.label(
            egui::RichText::new(app.t("Recent"))
                .size(theme.text_xs)
                .strong()
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_4);

        let recent: Vec<_> = app
            .files_store
            .recent_files
            .iter()
            .take(10)
            .cloned()
            .collect();
        for path in recent {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            let is_selected = app
                .files_store
                .selected_path
                .as_ref()
                .map_or(false, |p| p == &path);
            let row_resp = crate::widgets::interactive_row(ui, is_selected, &theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} {}", crate::theme::ICON_FILE, name))
                            .size(theme.text_sm)
                            .color(theme.text),
                    );
                });
            });
            if row_resp.response.clicked() {
                app.files_store.selected_path = Some(path.clone());
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                app.ui_store.preview_item = Some(crate::ui::types::PreviewItem::File {
                    name,
                    content,
                    path: path.to_string_lossy().into_owned(),
                });
            }
        }
    }

    // --- context menu ---
    if let Some(ref ctx_path) = secondary_click {
        let name = ctx_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ctx_path.to_string_lossy().into_owned());
        render_context_menu(app, ui, ctx_path, &name, &theme);
    }
}

fn render_context_menu(
    app: &mut App,
    ui: &mut egui::Ui,
    path: &Path,
    name: &str,
    theme: &crate::theme::Theme,
) {
    let path_buf = path.to_path_buf();
    ui.menu_button(
        egui::RichText::new(format!("{} {}", crate::theme::ICON_FILE, name))
            .size(theme.text_xs)
            .color(theme.text),
        |ui| {
            if ui.button(app.t("Preview").to_string()).clicked() {
                let content = std::fs::read_to_string(&path_buf).unwrap_or_default();
                app.ui_store.preview_item = Some(crate::ui::types::PreviewItem::File {
                    name: name.to_string(),
                    content,
                    path: path_buf.to_string_lossy().into_owned(),
                });
                ui.close_menu();
            }
            if ui.button(app.t("Open in Editor").to_string()).clicked() {
                let file_url = format!("file:///{}", path_buf.to_string_lossy().replace('\\', "/"));
                let _ = webbrowser::open(&file_url);
                ui.close_menu();
            }
            if ui.button(app.t("Add to Chat").to_string()).clicked() {
                app.chat_store
                    .attachments
                    .push(crate::ui::types::Attachment {
                        path: path_buf.clone(),
                        name: name.to_string(),
                    });
                ui.close_menu();
            }
            if ui.button(app.t("Copy Path").to_string()).clicked() {
                ui.ctx().copy_text(path_buf.to_string_lossy().into_owned());
                ui.close_menu();
            }
            ui.separator();
            ui.add_enabled_ui(false, |ui| {
                if ui
                    .button(app.t("Create PR with changes").to_string())
                    .clicked()
                {
                    ui.close_menu();
                }
            })
            .response
            .on_disabled_hover_text(app.t("GitHub integration coming soon"));
        },
    );
}
