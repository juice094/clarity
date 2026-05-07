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
const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build", ".clarity"];

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
            let header = egui::CollapsingHeader::new(
                egui::RichText::new(format!("📁 {}", name))
                    .size(theme.text_sm)
                    .color(theme.text),
            )
            .id_salt(header_id)
            .default_open(depth < 1);
            let resp = header.show(ui, |ui| {
                render_file_tree(
                    ui,
                    &full_path,
                    theme,
                    depth + 1,
                    selected_path,
                    on_file_click,
                );
            });
            // Allow clicking the directory header itself to select it
            if resp.header_response.clicked() {
                on_file_click(&full_path);
            }
        } else {
            // ── Custom painter row for files ──
            // Using raw painter + interact gives us:
            //   - full-width hover background
            //   - 3px left accent bar on selection
            //   - depth-based indent
            let full_width = ui.available_width();
            let row_height = 20.0;
            let row_rect = ui.available_rect_before_wrap();
            let row_rect =
                egui::Rect::from_min_size(row_rect.min, egui::vec2(full_width, row_height));
            let response = ui.interact(row_rect, ui.id().with(&full_path), egui::Sense::click());
            let is_selected = selected_path.is_some_and(|sp| {
                // Normalise Windows back-slashes before comparison
                let a = full_path.to_string_lossy().replace('\\', "/");
                let b = std::path::Path::new(sp).to_string_lossy().replace('\\', "/");
                a == b
            });

            if ui.is_rect_visible(row_rect) {
                let painter = ui.painter_at(row_rect);
                if is_selected {
                    // Selected: full bg_hover + accent bar on left edge
                    painter.rect_filled(row_rect, egui::CornerRadius::same(4), theme.bg_hover);
                    let accent_bar = egui::Rect::from_min_max(
                        egui::pos2(row_rect.min.x, row_rect.min.y + 2.0),
                        egui::pos2(row_rect.min.x + 3.0, row_rect.max.y - 2.0),
                    );
                    painter.rect_filled(accent_bar, egui::CornerRadius::same(2), theme.accent);
                } else if response.hovered() {
                    // Hover: subtle bg_hover
                    painter.rect_filled(
                        row_rect,
                        egui::CornerRadius::same(4),
                        theme.bg_hover.linear_multiply(0.5),
                    );
                }
                let text_pos = row_rect.min + egui::vec2(4.0 * depth as f32 + 8.0, 3.0);
                let text_color = if is_selected {
                    theme.text
                } else {
                    theme.text_dim
                };
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    format!("📄 {}", name),
                    egui::FontId::new(theme.text_sm, egui::FontFamily::Proportional),
                    text_color,
                );
            }
            if response.clicked() {
                on_file_click(&full_path);
            }
            ui.allocate_space(egui::vec2(full_width, row_height));
        }
    }
}
