use crate::theme::Theme;
use std::path::Path;

// ============================================================================
// File Browser — Recursive directory tree for sidebar
// ============================================================================

const MAX_DEPTH: usize = 4;

/// Render a directory tree starting at `path`.
/// `on_file_click` is called when a file (not directory) is clicked.
pub fn render_file_tree(
    ui: &mut egui::Ui,
    path: &Path,
    theme: &Theme,
    depth: usize,
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
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let full_path = entry.path();

        if is_dir {
            let header = egui::CollapsingHeader::new(
                egui::RichText::new(format!("📁 {}", name))
                    .size(theme.text_sm)
                    .color(theme.text),
            )
            .id_salt(full_path.to_string_lossy().to_string())
            .default_open(depth < 1);
            header.show(ui, |ui| {
                render_file_tree(ui, &full_path, theme, depth + 1, on_file_click);
            });
        } else {
            let full_width = ui.available_width();
            let row_height = 20.0;
            let row_rect = ui.available_rect_before_wrap();
            let row_rect =
                egui::Rect::from_min_size(row_rect.min, egui::vec2(full_width, row_height));
            let response = ui.interact(row_rect, ui.id().with(&full_path), egui::Sense::click());
            if ui.is_rect_visible(row_rect) {
                let painter = ui.painter_at(row_rect);
                let text_pos = row_rect.min + egui::vec2(4.0 * depth as f32 + 4.0, 3.0);
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    format!("📄 {}", name),
                    egui::FontId::new(theme.text_sm, egui::FontFamily::Proportional),
                    theme.text_dim,
                );
                if response.hovered() {
                    painter.rect_filled(row_rect, egui::CornerRadius::same(4), theme.bg_hover);
                }
            }
            if response.clicked() {
                on_file_click(&full_path);
            }
            ui.allocate_space(egui::vec2(full_width, row_height));
        }
    }
}
