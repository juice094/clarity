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
                    .size(12.0)
                    .color(theme.text),
            )
            .id_salt(full_path.to_string_lossy().to_string())
            .default_open(depth < 1);
            header.show(ui, |ui| {
                render_file_tree(ui, &full_path, theme, depth + 1, on_file_click);
            });
        } else {
            let response = ui.horizontal(|ui| {
                ui.add_space(4.0 * depth as f32);
                ui.label(
                    egui::RichText::new(format!("📄 {}", name))
                        .size(12.0)
                        .color(theme.text_dim),
                )
            }).response.interact(egui::Sense::click());
            if response.clicked() {
                on_file_click(&full_path);
            }
        }
    }
}
