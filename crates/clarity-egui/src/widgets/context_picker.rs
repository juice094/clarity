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
    /// When set, the picker is showing a file browser sub-view instead of
    /// the source list. `"file"` = file picker, `"folder"` = folder picker.
    pub browsing: Option<String>,
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
            ui.set_max_height(320.0);

            if let Some(ref mode) = state.browsing.clone() {
                // ── File browser sub-view ──
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [60.0, 20.0],
                            egui::Button::new(
                                egui::RichText::new("\u{2190} Back")
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            )
                            .fill(theme.surface),
                        )
                        .clicked()
                    {
                        state.browsing = None;
                        state.filter.clear();
                    }
                    ui.add_space(theme.space_4);
                    ui.label(
                        egui::RichText::new(if mode == "file" {
                            "Select a file"
                        } else {
                            "Select a folder"
                        })
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                    );
                });
                ui.add_space(theme.space_4);

                let root = state.cwd.clone();
                if root.exists() && root.is_dir() {
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .id_salt("ctx_picker_file_tree")
                        .show(ui, |ui| {
                            crate::ui::file_browser::render_file_tree(
                                ui,
                                &root,
                                theme,
                                0,
                                None,
                                &mut |path: &std::path::Path| {
                                    if path.is_file() || mode == "folder" {
                                        let resolved = if mode == "file" {
                                            ContextSource::File {
                                                path: path.to_string_lossy().into_owned(),
                                                start_line: None,
                                                end_line: None,
                                            }
                                        } else {
                                            ContextSource::Folder {
                                                path: path.to_string_lossy().into_owned(),
                                            }
                                        };
                                        let item = build_item(&resolved, &state.cwd);
                                        result = Some(item);
                                        state.open = false;
                                        state.browsing = None;
                                        state.filter.clear();
                                    }
                                },
                                &mut |_path: &std::path::Path| {},
                                true, // compact
                            );
                        });
                } else {
                    ui.label(
                        egui::RichText::new("Directory not accessible")
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                }
            } else {
                // ── Source list view ──
                let hint = "Filter…";
                ui.add(
                    egui::TextEdit::singleline(&mut state.filter)
                        .hint_text(hint)
                        .font(theme.font_mono(theme.text_sm)),
                );
                ui.add_space(theme.space_4);

                for src in available_items {
                    let (label, icon, desc) = source_info(src);
                    let matches_filter = state.filter.is_empty()
                        || label.to_lowercase().contains(&state.filter.to_lowercase());

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
                        match src {
                            ContextSource::File { .. } | ContextSource::Folder { .. } => {
                                // Open file browser sub-view.
                                state.browsing = Some(
                                    if matches!(src, ContextSource::File { .. }) {
                                        "file"
                                    } else {
                                        "folder"
                                    }
                                    .to_string(),
                                );
                            }
                            ContextSource::Web { .. } | ContextSource::Terminal { .. } => {
                                if state.filter.is_empty() {
                                    state.filter = String::new();
                                } else {
                                    let resolved_src = resolve_source(src, &state.filter);
                                    let item = build_item(&resolved_src, &state.cwd);
                                    result = Some(item);
                                    state.open = false;
                                    state.filter.clear();
                                }
                            }
                            _ => {
                                let resolved_src = resolve_source(src, &state.filter);
                                let item = build_item(&resolved_src, &state.cwd);
                                result = Some(item);
                                state.open = false;
                                state.filter.clear();
                            }
                        }
                    }
                }
            }
        });

    result
}

fn source_info(src: &ContextSource) -> (&'static str, &'static str, &'static str) {
    match src {
        ContextSource::File { .. } => ("File", crate::theme::ICON_FILE, "Select a file"),
        ContextSource::Code { .. } => (
            "Code Symbol",
            crate::theme::ICON_FILE_CODE,
            "Function/class",
        ),
        ContextSource::Folder { .. } => {
            ("Folder", crate::theme::ICON_FOLDER_OPEN, "All files in dir")
        }
        ContextSource::Terminal { .. } => {
            ("Terminal", crate::theme::ICON_TERMINAL, "Command output")
        }
        ContextSource::Web { .. } => ("Web", crate::theme::ICON_GLOBE, "Fetch URL"),
        ContextSource::Documentation { .. } => ("Docs", crate::theme::ICON_BOOK, "Documentation"),
        ContextSource::Codebase { .. } => {
            ("Codebase", crate::theme::ICON_LAYERS, "Semantic search")
        }
        ContextSource::GitDiff { .. } => ("Git Diff", crate::theme::ICON_MINUS, "Branch diff"),
    }
}

/// Resolve a source by using the filter text to refine paths or fill URL/command.
fn resolve_source(src: &ContextSource, filter: &str) -> ContextSource {
    match src {
        ContextSource::File {
            path,
            start_line,
            end_line,
        } => {
            let p = PathBuf::from(path);
            if p.is_dir() && !filter.is_empty() {
                let file_path = p.join(filter);
                ContextSource::File {
                    path: file_path.to_string_lossy().into_owned(),
                    start_line: *start_line,
                    end_line: *end_line,
                }
            } else {
                src.clone()
            }
        }
        ContextSource::Folder { path } => {
            let p = PathBuf::from(path);
            if !filter.is_empty() {
                let sub_path = p.join(filter);
                if sub_path.is_dir() {
                    return ContextSource::Folder {
                        path: sub_path.to_string_lossy().into_owned(),
                    };
                }
            }
            src.clone()
        }
        ContextSource::Web { .. } => {
            if !filter.is_empty() {
                ContextSource::Web {
                    url: filter.to_string(),
                }
            } else {
                src.clone()
            }
        }
        ContextSource::Terminal { .. } => {
            if !filter.is_empty() {
                ContextSource::Terminal {
                    command: filter.to_string(),
                }
            } else {
                src.clone()
            }
        }
        _ => src.clone(),
    }
}

fn build_item(src: &ContextSource, cwd: &PathBuf) -> ContextItem {
    match src {
        ContextSource::File {
            path,
            start_line,
            end_line,
        } => {
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
            display: if url.is_empty() {
                url.to_string()
            } else {
                format!("Web: {}", url)
            },
            payload: if url.is_empty() {
                String::new()
            } else {
                format!("Web content from: {}", url)
            },
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
