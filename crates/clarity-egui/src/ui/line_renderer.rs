//! Line-atoms renderer for egui — S5 Phase 2B skeleton.
#![allow(dead_code)] // Skeleton: functions wired in S6 (Phase 2C ChatArea migration)
//!
//! Renders `clarity_core::ui::render_line::RenderLine` into egui widgets.
//! Virtual scrolling is handled by skipping invisible lines; the actual
//! scroll container is provided by the caller (usually `ScrollArea`).

use clarity_core::ui::render_line::{
    ApprovalOption, DiffKind, LineRole, RenderLine, Span, StatusKind, ToolStatus,
};

use crate::theme::Theme;

/// Height of a single text line in pixels (theme token).
///
/// Must match the font metrics used by `theme.font(theme.text_base)`.
pub const LINE_HEIGHT: f32 = 18.0;

/// Render a batch of lines with virtual-scroll culling.
///
/// `scroll_offset` and `viewport_height` are used to compute which lines are
/// visible; invisible lines are skipped to maintain 60 fps at 10K lines.
///
/// `selected_idx` is the `LineCursor::selected` value; the matching row draws
/// a subtle highlight background.
pub fn render_lines(
    ui: &mut egui::Ui,
    lines: &[RenderLine],
    theme: &Theme,
    scroll_offset: f32,
    viewport_height: f32,
    selected_idx: Option<usize>,
) {
    if lines.is_empty() {
        return;
    }

    let start_y = ui.cursor().min.y;
    let (start_idx, end_idx) =
        compute_visible_range(lines.len(), scroll_offset, viewport_height, LINE_HEIGHT);

    for (idx, line) in lines.iter().enumerate().take(end_idx).skip(start_idx) {
        let is_selected = selected_idx == Some(idx);
        let line_y = start_y + (idx as f32 * LINE_HEIGHT) - scroll_offset;

        // Skip if completely above or below the current clip rect.
        let clip_min = ui.clip_rect().min.y;
        let clip_max = ui.clip_rect().max.y;
        if line_y + LINE_HEIGHT < clip_min || line_y > clip_max {
            // Still advance the cursor so egui's layout remains consistent.
            ui.add_space(LINE_HEIGHT);
            continue;
        }

        // Selected-line highlight.
        if is_selected {
            let rect = egui::Rect::from_min_size(
                egui::pos2(ui.min_rect().min.x, line_y),
                egui::vec2(ui.available_width(), LINE_HEIGHT),
            );
            ui.painter().rect_filled(rect, 0, theme.bg_hover);
        }

        // Position the cursor at the correct Y for this line.
        let desired = egui::pos2(ui.cursor().min.x, line_y);
        let current = ui.cursor().min;
        if (desired.y - current.y).abs() > 0.5 {
            ui.add_space(desired.y - current.y);
        }

        render_line(ui, line, theme, is_selected);
    }

    // Reserve total height so the scroll area knows the full extent.
    let total_height = lines.len() as f32 * LINE_HEIGHT;
    let consumed = (end_idx - start_idx) as f32 * LINE_HEIGHT;
    if total_height > consumed {
        ui.add_space(total_height - consumed);
    }
}

/// Render a single `RenderLine`.
fn render_line(ui: &mut egui::Ui, line: &RenderLine, theme: &Theme, is_selected: bool) {
    match line {
        RenderLine::Text {
            spans,
            role,
            indent,
        } => {
            render_text_line(ui, spans, *role, *indent, theme, is_selected);
        }
        RenderLine::CodeLine {
            lang,
            content,
            line_no,
            diff,
        } => {
            render_code_line(ui, lang, content, *line_no, *diff, theme);
        }
        RenderLine::ToolCallHeader {
            name,
            status,
            expanded,
        } => {
            render_tool_call_header(ui, name, *status, *expanded, theme);
        }
        RenderLine::ToolCallArg { key, value } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{}: ", key))
                        .font(theme.font_mono(theme.text_sm))
                        .color(theme.text_muted),
                );
                ui.label(
                    egui::RichText::new(value.as_str())
                        .font(theme.font_mono(theme.text_sm))
                        .color(theme.text),
                );
            });
        }
        RenderLine::Thinking { content, collapsed } => {
            let icon = if *collapsed {
                crate::theme::ICON_CARET_RIGHT
            } else {
                crate::theme::ICON_CARET_DOWN
            };
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(icon)
                        .font(theme.font_icon(theme.text_sm))
                        .color(theme.text_dim),
                );
                ui.label(
                    egui::RichText::new("Thinking")
                        .size(theme.text_sm)
                        .color(theme.text_dim)
                        .italics(),
                );
            });
            if !collapsed {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(content.as_str())
                            .font(theme.font(theme.text_sm))
                            .color(theme.text_muted),
                    );
                });
            }
        }
        RenderLine::ApprovalPrompt { options } => {
            render_approval_prompt(ui, options, theme);
        }
        RenderLine::StatusLine {
            kind,
            content,
            transient,
        } => {
            render_status_line(ui, *kind, content, *transient, theme);
        }
        RenderLine::ArtifactRef {
            artifact_id,
            summary,
        } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::theme::ICON_SQUARE)
                        .font(theme.font_icon(theme.text_sm))
                        .color(theme.accent),
                );
                ui.label(
                    egui::RichText::new(format!("{} — {}", artifact_id, summary))
                        .size(theme.text_sm)
                        .color(theme.accent)
                        .underline(),
                );
            });
        }
        RenderLine::CrossInstanceRef {
            target_instance,
            target_session,
            message,
        } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("@")
                        .size(theme.text_sm)
                        .color(theme.accent),
                );
                let meta = if let Some(sid) = target_session {
                    format!("{} ({}) — {}", target_instance, sid, message)
                } else {
                    format!("{} — {}", target_instance, message)
                };
                ui.label(
                    egui::RichText::new(meta)
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            });
        }
        RenderLine::SlashCompletion {
            command,
            description,
        } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("/{}  ", command))
                        .size(theme.text_sm)
                        .color(theme.accent)
                        .monospace(),
                );
                ui.label(
                    egui::RichText::new(description.as_str())
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
            });
        }
        RenderLine::StreamingCursor => {
            ui.label(
                egui::RichText::new("▌")
                    .font(theme.font(theme.text_base))
                    .color(theme.accent),
            );
        }
        RenderLine::Divider => {
            ui.add(egui::Separator::default().horizontal());
        }
        RenderLine::Empty => {
            // Preserve vertical spacing.
            ui.add_space(LINE_HEIGHT);
        }
        RenderLine::BlockSlot {
            block_id,
            line_count,
        } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "⤢ Block {} ({} lines) — click to expand",
                        block_id, line_count
                    ))
                    .size(theme.text_sm)
                    .color(theme.text_dim)
                    .italics(),
                );
            });
        }
    }
}

// ============================================================================
// Per-variant renderers
// ============================================================================

fn render_text_line(
    ui: &mut egui::Ui,
    spans: &[Span],
    role: LineRole,
    indent: u8,
    theme: &Theme,
    _is_selected: bool,
) {
    let indent_px = indent as f32 * 16.0;
    if indent_px > 0.0 {
        ui.add_space(0.0); // ensure horizontal layout starts fresh
    }

    let (color, size, strong) = role_style(role, theme);

    ui.horizontal(|ui| {
        if indent_px > 0.0 {
            ui.add_space(indent_px);
        }

        // Role prefix icons / bullets.
        match role {
            LineRole::UnorderedListItem(_) => {
                ui.label(egui::RichText::new("• ").size(size).color(color));
            }
            LineRole::OrderedListItem { num, .. } => {
                ui.label(
                    egui::RichText::new(format!("{}. ", num))
                        .size(size)
                        .color(color),
                );
            }
            LineRole::Quote => {
                ui.label(egui::RichText::new("| ").size(size).color(theme.text_dim));
            }
            _ => {}
        }

        // Flatten spans to a single string for the skeleton.
        // S5+ (renderer polish) will style each span individually.
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        let mut rich = egui::RichText::new(text).size(size).color(color);
        if strong {
            rich = rich.strong();
        }
        if matches!(role, LineRole::Quote) {
            rich = rich.italics();
        }
        ui.label(rich);
    });
}

fn render_code_line(
    ui: &mut egui::Ui,
    _lang: &str,
    content: &str,
    line_no: Option<u32>,
    diff: DiffKind,
    theme: &Theme,
) {
    let bg = match diff {
        DiffKind::Added => egui::Color32::from_rgb(20, 60, 20),
        DiffKind::Removed => egui::Color32::from_rgb(60, 20, 20),
        DiffKind::Context => egui::Color32::from_rgb(40, 40, 40),
        DiffKind::Normal => theme.bg_elevated,
    };

    let _full_width = ui.available_width();
    let line_rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(line_rect, 0, bg);

    ui.horizontal(|ui| {
        if let Some(n) = line_no {
            ui.label(
                egui::RichText::new(format!("{:4} ", n))
                    .font(theme.font_mono(theme.text_sm))
                    .color(theme.text_dim),
            );
        }
        ui.label(
            egui::RichText::new(content)
                .font(theme.font_mono(theme.text_base))
                .color(theme.text),
        );
    });
}

fn render_tool_call_header(
    ui: &mut egui::Ui,
    name: &str,
    status: ToolStatus,
    expanded: bool,
    theme: &Theme,
) {
    let (icon, color) = match status {
        ToolStatus::Running => (crate::theme::ICON_HOURGLASS, theme.status_busy),
        ToolStatus::Success => (crate::theme::ICON_CHECK, theme.status_online),
        ToolStatus::Warning => (crate::theme::ICON_WARNING, theme.warn),
        ToolStatus::Error => (crate::theme::ICON_X, theme.danger),
    };
    let caret = if expanded {
        crate::theme::ICON_CARET_DOWN
    } else {
        crate::theme::ICON_CARET_RIGHT
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(caret)
                .font(theme.font_icon(theme.text_sm))
                .color(theme.text_dim),
        );
        ui.label(
            egui::RichText::new(icon)
                .font(theme.font_icon(theme.text_sm))
                .color(color),
        );
        ui.label(
            egui::RichText::new(name)
                .size(theme.text_sm)
                .color(theme.text_strong),
        );
    });
}

fn render_approval_prompt(ui: &mut egui::Ui, options: &[ApprovalOption], theme: &Theme) {
    ui.horizontal(|ui| {
        for (i, opt) in options.iter().enumerate() {
            let label = match opt {
                ApprovalOption::Yes => format!("[{}] Yes", i + 1),
                ApprovalOption::YesAndRemember => format!("[{}] Yes (remember)", i + 1),
                ApprovalOption::No { .. } => format!("[{}] No", i + 1),
                ApprovalOption::Custom(s) => format!("[{}] {}", i + 1, s),
            };
            if i > 0 {
                ui.add_space(theme.space_12);
            }
            ui.label(
                egui::RichText::new(label)
                    .size(theme.text_sm)
                    .color(theme.accent),
            );
        }
    });
}

fn render_status_line(
    ui: &mut egui::Ui,
    kind: StatusKind,
    content: &str,
    transient: bool,
    theme: &Theme,
) {
    let icon = match kind {
        StatusKind::Spinner => "◐",
        StatusKind::Progress { .. } => "▶",
        StatusKind::Network => "⇅",
        StatusKind::Compaction => "◷",
        StatusKind::ModelSwitch => "◈",
    };
    let alpha = if transient { 0.6 } else { 1.0 };
    let color = theme.text_muted;
    let fg = egui::Color32::from_rgba_premultiplied(
        color.r(),
        color.g(),
        color.b(),
        (color.a() as f32 * alpha) as u8,
    );

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(icon)
                .font(theme.font_icon(theme.text_sm))
                .color(fg),
        );
        ui.label(egui::RichText::new(content).size(theme.text_sm).color(fg));
        if let StatusKind::Progress { current, total } = kind {
            ui.label(
                egui::RichText::new(format!(" {}/{}", current, total))
                    .size(theme.text_sm)
                    .color(fg),
            );
        }
    });
}

// ============================================================================
// Helpers
// ============================================================================

/// Map `LineRole` to (color, font_size, strong) for the skeleton renderer.
fn role_style(role: LineRole, theme: &Theme) -> (egui::Color32, f32, bool) {
    match role {
        LineRole::UserMessage => (theme.chat_text, theme.text_base, false),
        LineRole::AgentMessage => (theme.chat_text, theme.text_base, false),
        LineRole::SystemMessage => (theme.text_muted, theme.text_sm, false),
        LineRole::ErrorMessage => (theme.error_text, theme.text_base, false),
        LineRole::Heading(l) => {
            let size = match l {
                1 => theme.text_2xl,
                2 => theme.text_xl,
                3 => theme.text_lg,
                4 => theme.text_md,
                5 => theme.text_base,
                _ => theme.text_sm,
            };
            (theme.text_strong, size, true)
        }
        LineRole::Quote => (theme.text_dim, theme.text_base, false),
        LineRole::UnorderedListItem(_) | LineRole::OrderedListItem { .. } => {
            (theme.chat_text, theme.text_base, false)
        }
        LineRole::Mention => (theme.accent, theme.text_base, false),
        LineRole::FileRef => (theme.accent_subtle, theme.text_base, false),
        LineRole::Status => (theme.status_online, theme.text_sm, false),
        LineRole::Warning => (theme.warn, theme.text_sm, false),
        LineRole::Note => (theme.text_muted, theme.text_sm, false),
        LineRole::TokenUsage | LineRole::ContextCompaction => {
            (theme.text_dim, theme.text_sm, false)
        }
        LineRole::Sandbox => (theme.text_muted, theme.text_sm, false),
    }
}

/// Compute the half-open visible index range for virtual scrolling.
fn compute_visible_range(
    total_lines: usize,
    scroll_offset: f32,
    viewport_height: f32,
    line_height: f32,
) -> (usize, usize) {
    if total_lines == 0 {
        return (0, 0);
    }
    let start = (scroll_offset / line_height).floor() as usize;
    let visible_count = (viewport_height / line_height).ceil() as usize;
    let end = (start + visible_count + 2).min(total_lines);
    (start.min(total_lines), end)
}
