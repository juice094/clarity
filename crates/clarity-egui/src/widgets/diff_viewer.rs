//! Unified diff viewer — reusable widget for rendering `DiffHunk` slices.
//!
//! Consumes the canonical diff types from `clarity_core::diff` and renders
//! a color-coded single-column view with optional line numbers, hunk
//! folding for large unchanged blocks, and accept/reject affordances.
//!
//! Extension points for future features (side-by-side mode, line comments)
//! are declared as reserved fields that degrade gracefully.

use crate::theme::Theme;
use clarity_core::diff::{DiffHunk, DiffLine};

// ── Configuration ────────────────────────────────────────────────────────

/// Configuration for the unified diff viewer.
pub struct DiffViewConfig {
    /// Show line numbers (old + new) alongside each line.
    pub show_line_numbers: bool,
    /// Show the `---` / `+++` file header above the first hunk.
    pub show_file_header: bool,
    /// Collapse long runs of unchanged context lines.
    pub collapse_unchanged: bool,
    /// Minimum number of *consecutive* context lines before a fold is inserted.
    pub collapse_threshold: usize,
    /// Compact mode: reduced line height and font size.
    pub compact: bool,
    /// Maximum height of the diff viewport (scrolls if taller).
    pub max_height: Option<f32>,
    /// Render accept/reject buttons at the bottom. Disable when the
    /// diff is embedded inside an approval modal that provides its own
    /// action buttons.
    pub show_actions: bool,
    // === Extension points (reserved) ===
    /// Side-by-side mode — reserved, always `false` for now.
    #[allow(dead_code)]
    pub side_by_side: bool,
}

impl Default for DiffViewConfig {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            show_file_header: true,
            collapse_unchanged: false,
            collapse_threshold: 6,
            compact: false,
            max_height: None,
            show_actions: true,
            side_by_side: false,
        }
    }
}

/// Compact config suitable for embedding inside an approval modal.
pub fn approval_diff_config() -> DiffViewConfig {
    DiffViewConfig {
        show_line_numbers: true,
        show_file_header: false,
        collapse_unchanged: true,
        collapse_threshold: 4,
        compact: true,
        max_height: Some(250.0),
        show_actions: false,
        side_by_side: false,
    }
}

/// Result returned by `render_diff_view`.
pub struct DiffViewResponse {
    pub accepted: bool,
    pub rejected: bool,
    /// 0-based global line index currently hovered.
    pub hovered_line: Option<usize>,
    // === Extension points (reserved) ===
    /// Future: comments left on specific lines during review.
    #[allow(dead_code)]
    pub line_comments: Vec<(usize, String)>,
}

// ── Public API ───────────────────────────────────────────────────────────

/// Render a unified diff view from pre-parsed hunks.
///
/// Returns `DiffViewResponse` indicating user action (accept / reject) as
/// well as which line is hovered for optional tooltip rendering by the
/// caller.
pub fn render_diff_view(
    ui: &mut egui::Ui,
    hunks: &[DiffHunk],
    theme: &Theme,
    config: &DiffViewConfig,
) -> DiffViewResponse {
    let mut resp = DiffViewResponse {
        accepted: false,
        rejected: false,
        hovered_line: None,
        line_comments: Vec::new(),
    };

    let font_size = if config.compact {
        theme.text_xs
    } else {
        theme.text_sm
    };

    // Determine line-number gutter width for alignment.
    let ln_width = if config.show_line_numbers {
        let max_old = hunks
            .iter()
            .map(|h| h.old_start + h.lines.len().saturating_sub(1))
            .max()
            .unwrap_or(1);
        let max_new = hunks
            .iter()
            .map(|h| h.new_start + h.lines.len().saturating_sub(1))
            .max()
            .unwrap_or(1);
        let digits = max_old.max(max_new).to_string().len().max(3);
        (digits as f32) * 8.0 + theme.space_12
    } else {
        0.0
    };

    let inner = egui::ScrollArea::vertical()
        .id_salt("diff_view_scroll")
        .auto_shrink([false; 2])
        .max_height(config.max_height.unwrap_or(f32::INFINITY));

    inner.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;

        if config.show_file_header {
            ui.label(
                egui::RichText::new("Unified diff")
                    .size(theme.text_xs)
                    .color(theme.text_dim)
                    .italics(),
            );
        }

        let mut global_line: usize = 0;
        let mut last_was_context: usize = 0; // consecutive context count
        let mut folded_until: Option<usize> = None; // skip lines while folded

        for (hunk_idx, hunk) in hunks.iter().enumerate() {
            // Hunk header: `@@ -old_start,N +new_start,N @@`
            let header = format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start,
                hunk.lines.len(),
                hunk.new_start,
                hunk.lines.len(),
            );

            if config.collapse_unchanged {
                // Count leading context lines.
                let lead_ctx = hunk
                    .lines
                    .iter()
                    .take_while(|l| matches!(l, DiffLine::Context(_)))
                    .count();
                if lead_ctx > config.collapse_threshold && last_was_context > 0 {
                    // This hunk starts with context AND previous hunk ended with
                    // context. Fold the gap.
                    let fold_text =
                        format!("⋮  {} unchanged lines  ⋮", lead_ctx + last_was_context);
                    let fold_btn = ui.button(
                        egui::RichText::new(&fold_text)
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                    if fold_btn.clicked() {
                        // Unfold: toggle collapse_unchanged or expand inline.
                        folded_until = None;
                    }
                }
            }

            // Hunk header row.
            ui.horizontal(|ui| {
                if config.show_line_numbers {
                    ui.add_sized(
                        [ln_width, font_size],
                        egui::Label::new(
                            egui::RichText::new("")
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        ),
                    );
                }
                ui.label(
                    egui::RichText::new(&header)
                        .size(theme.text_xs)
                        .color(theme.accent)
                        .strong(),
                );
            });

            for line in &hunk.lines {
                global_line += 1;

                // Honour folding.
                if let Some(end) = folded_until {
                    if global_line < end {
                        last_was_context = if matches!(line, DiffLine::Context(_)) {
                            last_was_context + 1
                        } else {
                            0
                        };
                        continue;
                    }
                }

                match line {
                    DiffLine::Context(content) => {
                        last_was_context += 1;
                        render_diff_line(
                            ui,
                            content,
                            theme.text_dim,
                            egui::Color32::TRANSPARENT,
                            ln_width,
                            font_size,
                            Some(hunk.old_start + global_line.saturating_sub(1)),
                            Some(hunk.new_start + global_line.saturating_sub(1)),
                            config,
                            theme,
                            &mut resp,
                            global_line,
                        );
                    }
                    DiffLine::Added(content) => {
                        last_was_context = 0;
                        render_diff_line(
                            ui,
                            content,
                            theme.diff_added_text,
                            theme.diff_added_bg,
                            ln_width,
                            font_size,
                            None, // no old-line number for added lines
                            Some(hunk.new_start + global_line.saturating_sub(1)),
                            config,
                            theme,
                            &mut resp,
                            global_line,
                        );
                    }
                    DiffLine::Removed(content) => {
                        last_was_context = 0;
                        render_diff_line(
                            ui,
                            content,
                            theme.diff_removed_text,
                            theme.diff_removed_bg,
                            ln_width,
                            font_size,
                            Some(hunk.old_start + global_line.saturating_sub(1)),
                            None, // no new-line number for removed lines
                            config,
                            theme,
                            &mut resp,
                            global_line,
                        );
                    }
                }
            }

            // Collapse trailing context beyond threshold.
            if config.collapse_unchanged && last_was_context > config.collapse_threshold {
                let fold_text = format!("⋮  {} unchanged lines  ⋮", last_was_context);
                let fold_btn = ui.button(
                    egui::RichText::new(&fold_text)
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
                if fold_btn.clicked() {
                    folded_until = None;
                }
                last_was_context = 0;
            }

            // Separator between hunks.
            if hunk_idx + 1 < hunks.len() {
                ui.add_space(theme.space_4);
            }
        }

        // Accept / Reject bar at the bottom (only when configured).
        if config.show_actions {
            ui.add_space(theme.space_12);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [120.0, theme.size_input],
                        egui::Button::new(
                            egui::RichText::new("✓ Accept")
                                .size(theme.text_sm)
                                .color(theme.text_strong),
                        )
                        .fill(theme.ok),
                    )
                    .clicked()
                {
                    resp.accepted = true;
                }
                if ui
                    .add_sized(
                        [120.0, theme.size_input],
                        egui::Button::new(
                            egui::RichText::new("✗ Reject")
                                .size(theme.text_sm)
                                .color(theme.text_strong),
                        )
                        .fill(theme.danger),
                    )
                    .clicked()
                {
                    resp.rejected = true;
                }
            });
        }
    });

    resp
}

// ── Single-line render helper ────────────────────────────────────────────

fn render_diff_line(
    ui: &mut egui::Ui,
    content: &str,
    text_color: egui::Color32,
    bg_color: egui::Color32,
    ln_width: f32,
    font_size: f32,
    old_ln: Option<usize>,
    new_ln: Option<usize>,
    config: &DiffViewConfig,
    theme: &Theme,
    resp: &mut DiffViewResponse,
    global_line: usize,
) {
    let prefix = match (config.show_line_numbers, old_ln, new_ln) {
        (true, Some(o), Some(n)) => format!("{:>4} {:>4} ", o, n),
        (true, Some(o), None) => format!("{:>4}      ", o),
        (true, None, Some(n)) => format!("      {:>4} ", n),
        _ => String::new(),
    };

    let line_rect = egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(2, 0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if config.show_line_numbers {
                    ui.add_sized(
                        [ln_width, font_size],
                        egui::Label::new(
                            egui::RichText::new(prefix)
                                .size(theme.text_xs)
                                .color(theme.text_dim)
                                .monospace(),
                        ),
                    );
                }
                ui.label(
                    egui::RichText::new(content.to_string())
                        .size(font_size)
                        .color(text_color)
                        .monospace(),
                );
            });
        });

    if line_rect.response.hovered() {
        resp.hovered_line = Some(global_line);
    }
}

// ── Tool-result extraction helper ────────────────────────────────────────

/// Attempt to extract a `_diff_preview` field from a JSON tool result
/// (e.g. the output of `FileEditTool`), parse it as a unified diff, and
/// return the structured hunks.
pub fn extract_diff_from_tool_result(result_json: &str) -> Option<Vec<DiffHunk>> {
    let v: serde_json::Value = serde_json::from_str(result_json).ok()?;
    let diff_str = v.get("_diff_preview")?.as_str()?;
    if diff_str.is_empty() {
        return None;
    }
    Some(clarity_core::diff::parse_unified_diff(diff_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_diff_from_file_edit_tool_output() {
        // Simulated FileEditTool output with _diff_preview (no trailing newline).
        let json = r#"{
            "path": "/tmp/test.rs",
            "_diff_preview": "--- a/test.rs\n+++ b/test.rs\n@@ -1,2 +1,2 @@\n-fn main() {\n+fn run() {\n"
        }"#;
        let hunks = extract_diff_from_tool_result(json).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 1);
        // 2 context lines (the @@ header counts the line range, not content count;
        // parse_unified_diff returns 2 lines: -old and +new).
        assert_eq!(hunks[0].lines.len(), 2);
        assert!(matches!(
            hunks[0].lines[0],
            clarity_core::diff::DiffLine::Removed(_)
        ));
        assert!(matches!(
            hunks[0].lines[1],
            clarity_core::diff::DiffLine::Added(_)
        ));
    }

    #[test]
    fn extract_diff_returns_none_when_no_preview() {
        let json = r#"{"path": "/tmp/test.rs", "result": "ok"}"#;
        assert!(extract_diff_from_tool_result(json).is_none());
    }

    #[test]
    fn config_default_is_sane() {
        let c = DiffViewConfig::default();
        assert!(c.show_line_numbers);
        assert!(!c.collapse_unchanged);
        assert!(!c.side_by_side);
        assert_eq!(c.collapse_threshold, 6);
    }
}
