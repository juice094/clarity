//! Context picker — `#` quick-add popup for injecting file, folder,
//! terminal, and web context into the chat input.
//!
//! Activated when the user types `#` in the chat composer.  Presents a
//! menu of context sources; selecting one opens a secondary chooser.
//! Selected items render as accent-colored chips in the input area.

use crate::theme::Theme;
use crate::ui::types::{ContextItem, ContextSource};
use std::path::PathBuf;

/// State for the `#` context picker popup.
#[derive(Clone, Default)]
pub struct ContextPickerState {
    /// Whether the picker is currently visible.
    pub open: bool,
    /// The raw text after `#` that the user is typing (filter query).
    pub filter: String,
    /// Current working directory for file/folder resolution.
    pub cwd: PathBuf,
}

/// Render the context picker popup. Returns `Some(ContextItem)` when the
/// user confirms a selection, or `None` while the picker is open.
///
/// Callers should invoke this immediately after the chat input widget when
/// `state.open` is true, positioning it as an anchored popup.
pub fn render_context_picker(
    ui: &mut egui::Ui,
    state: &mut ContextPickerState,
    theme: &Theme,
    available_items: &[ContextSource],
) -> Option<ContextItem> {
    let mut result = None;

    egui::Frame::popup(ui.style())
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            ui.set_min_width(200.0);

            // Filter input.
            let hint = "Filter…";
            ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text(hint)
                    .font(theme.font_mono(theme.text_sm)),
            );
            ui.add_space(theme.space_4);

            // Source list with inline selection.
            for src in available_items {
                let (label, icon, desc) = source_info(src);
                let matches_filter = state.filter.is_empty()
                    || label
                        .to_lowercase()
                        .contains(&state.filter.to_lowercase());

                if !matches_filter {
                    continue;
                }

                let row = egui::Frame::new()
                    .fill(theme.surface)
                    .inner_margin(egui::Margin::symmetric(theme.space_8 as i8, 4))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(icon)
                                    .font(theme.font_icon(theme.text_sm))
                                    .color(theme.accent),
                            );
                            ui.label(
                                egui::RichText::new(label)
                                    .size(theme.text_sm)
                                    .color(theme.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(desc)
                                            .size(theme.text_xs)
                                            .color(theme.text_dim),
                                    );
                                },
                            );
                        });
                    });

                if row.response.clicked() {
                    let item = build_item(src, &state.cwd);
                    result = Some(item);
                    state.open = false;
                    state.filter.clear();
                }
            }

            if result.is_none() {
                // Cancelled by clicking outside — handled by caller checking
                // whether the mouse clicked outside the popup area.
            }
        });

    result
}

fn source_info(src: &ContextSource) -> (&'static str, &'static str, &'static str) {
    match src {
        ContextSource::File { .. } => ("File", crate::theme::ICON_FILE, "Select a file"),
        ContextSource::Code { .. } => ("Code Symbol", crate::theme::ICON_FILE_CODE, "Function/class"),
        ContextSource::Folder { .. } => ("Folder", crate::theme::ICON_FOLDER_OPEN, "All files in dir"),
        ContextSource::Terminal { .. } => ("Terminal", crate::theme::ICON_TERMINAL, "Command output"),
        ContextSource::Web { .. } => ("Web", crate::theme::ICON_GLOBE, "Fetch URL"),
        ContextSource::Documentation { .. } => ("Docs", crate::theme::ICON_BOOK, "Documentation"),
        ContextSource::Codebase { .. } => ("Codebase", crate::theme::ICON_LAYERS, "Semantic search"),
        ContextSource::GitDiff { .. } => ("Git Diff", crate::theme::ICON_MINUS, "Branch diff"),
    }
}

fn build_item(src: &ContextSource, cwd: &PathBuf) -> ContextItem {
    match src {
        ContextSource::File { path, start_line, end_line } => {
            let display = match (start_line, end_line) {
                (Some(s), Some(e)) => format!("{}:{}-{}", file_name(path), s, e),
                _ => file_name(path),
            };
            let full_path = if PathBuf::from(path).is_relative() {
                cwd.join(path)
            } else {
                PathBuf::from(path)
            };
            let payload = std::fs::read_to_string(&full_path).unwrap_or_default();
            ContextItem {
                source: src.clone(),
                display,
                payload,
            }
        }
        ContextSource::Code { symbol, file } => ContextItem {
            source: src.clone(),
            display: format!("{}::{}", file_name(file), symbol),
            payload: format!("// {}: {}", file, symbol),
        },
        ContextSource::Folder { path } => {
            let full_path = if PathBuf::from(path).is_relative() {
                cwd.join(path)
            } else {
                PathBuf::from(path)
            };
            let mut payload = String::new();
            if let Ok(entries) = std::fs::read_dir(&full_path) {
                for e in entries.flatten() {
                    payload.push_str(&format!("{}\n", e.path().display()));
                }
            }
            ContextItem {
                source: src.clone(),
                display: file_name(path),
                payload,
            }
        }
        ContextSource::Terminal { command } => ContextItem {
            source: src.clone(),
            display: format!("$ {}", command),
            payload: format!("Command: {}\n", command),
        },
        ContextSource::Web { url } => ContextItem {
            source: src.clone(),
            display: url.clone(),
            payload: url.clone(),
        },
        // Extension points — return placeholder items.
        ContextSource::Documentation { url } => ContextItem {
            source: src.clone(),
            display: format!("Docs: {}", url),
            payload: String::new(),
        },
        ContextSource::Codebase { query } => ContextItem {
            source: src.clone(),
            display: format!("Search: {}", query),
            payload: String::new(),
        },
        ContextSource::GitDiff { base_branch } => ContextItem {
            source: src.clone(),
            display: format!("Diff: {}", base_branch),
            payload: String::new(),
        },
    }
}

fn file_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}
