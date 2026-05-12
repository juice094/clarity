//! File browser — recursive directory tree for the workspace panel.
//!
//! IS-1 Sprint 32: restored from git history and wired into `render_sidebar`.
//! Sprint 39: Windows path normalization, skip-list for large dirs,
//!            directory click support, hidden-file whitelist.
//!
//! Design notes:
//! - `MAX_DEPTH` prevents infinite recursion on circular symlinks.
//! - Custom painter-based file rows (instead of `selectable_label`) give us
//!   per-pixel control over hover / selected / accent-bar visuals.
//! - `is_rect_visible` culling keeps the tree cheap even for large dirs.
//! - `SKIP_DIRS` skips build artifacts (`target`, `node_modules`, etc.) to
//!   keep the tree responsive on real projects.

use crate::theme::Theme;
use std::path::Path;

/// Maximum recursion depth for the directory tree.
const MAX_DEPTH: usize = 6;

/// Directories that are skipped entirely to avoid performance cliffs
/// on typical development workspaces.
const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    "dist",
    "build",
    ".clarity",
];

/// Render a directory tree starting at `path`.
///
/// `on_file_click` is called when a file (not directory) is clicked.
/// `selected_path` highlights the matching file row with an accent indicator.
pub fn render_file_tree(
    ui: &mut egui::Ui,
    path: &Path,
    theme: &Theme,
    depth: usize,
    selected_path: Option<&str>,
    on_file_click: &mut dyn FnMut(&Path),
    compact: bool,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut entries: Vec<_> = match std::fs::read_dir(path) {
        Ok(it) => it.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match b_dir.cmp(&a_dir) {
            std::cmp::Ordering::Equal => a.file_name().cmp(&b.file_name()),
            other => other,
        }
    });

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        // Skip known large / irrelevant directories
        if is_dir && SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }

        let full_path = entry.path();

        if is_dir {
            let header_id = ui.id().with(&full_path);
            let dir_label = if compact {
                let prefix: String = name.chars().take(5).collect();
                prefix
            } else {
                name.clone()
            };
            let header = egui::CollapsingHeader::new(
                egui::RichText::new(dir_label)
                    .size(theme.text_sm)
                    .color(theme.text),
            )
            .id_salt(header_id)
            .default_open(depth < 1);
            let resp = header.show(ui, |ui| {
                ui.spacing_mut().indent = 16.0;
                render_file_tree(
                    ui,
                    &full_path,
                    theme,
                    depth + 1,
                    selected_path,
                    on_file_click,
                    compact,
                );
            });
            // Allow clicking the directory header itself to select it
            if resp.header_response.clicked() {
                on_file_click(&full_path);
            }
        } else {
            // ── File row (interactive_row + decorator painter) ──
            let is_selected = selected_path.is_some_and(|sp| {
                // Normalise Windows back-slashes before comparison
                let a = full_path.to_string_lossy().replace('\\', "/");
                let b = std::path::Path::new(sp)
                    .to_string_lossy()
                    .replace('\\', "/");
                a == b
            });
            let text_color = if is_selected {
                theme.text
            } else {
                theme.text_dim
            };
            let indent = if compact {
                4.0
            } else {
                8.0 * depth as f32 + 12.0
            };

            let resp = crate::widgets::interactive_row(
                ui,
                is_selected,
                theme,
                |ui| {
                    if is_selected {
                        let accent_bar = egui::Rect::from_min_max(
                            egui::pos2(ui.min_rect().min.x, ui.min_rect().min.y + 2.0),
                            egui::pos2(ui.min_rect().min.x + 3.0, ui.min_rect().max.y - 2.0),
                        );
                        ui.painter().rect_filled(
                            accent_bar,
                            egui::CornerRadius::same(2),
                            theme.accent,
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(indent);

                        // Icon (decorative painter — allowed per RULE 2)
                        let icon_size = if compact { 10.0 } else { 14.0 };
                        let icon_resp = ui.allocate_exact_size(
                            egui::vec2(icon_size, icon_size),
                            egui::Sense::hover(),
                        );
                        let icon_rect = icon_resp.1.rect;
                        if ui.is_rect_visible(icon_rect) {
                            let painter = ui.painter_at(icon_rect);
                            crate::ui::icons::paint_file(&painter, icon_rect, text_color);
                            if let Some(ext) = full_path.extension().and_then(|e| e.to_str()) {
                                let badge = match ext {
                                    "rs" => Some("R"),
                                    "md" => Some("M"),
                                    "toml" => Some("≡"),
                                    _ => None,
                                };
                                if let Some(b) = badge {
                                    crate::ui::icons::paint_file_badge(
                                        &painter,
                                        icon_rect,
                                        b,
                                        text_color,
                                        if compact { 5.0 } else { 6.0 },
                                    );
                                }
                            }
                        }

                        ui.add_space(4.0);

                        // Filename
                        let label = if compact {
                            let prefix: String = name.chars().take(5).collect();
                            prefix
                        } else {
                            name.clone()
                        };
                        ui.label(
                            egui::RichText::new(label)
                                .size(theme.text_sm)
                                .color(text_color),
                        );
                    });
                },
            );
            if resp.response.clicked() {
                on_file_click(&full_path);
            }
        }
    }
}
