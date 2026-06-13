//! RenderLine → ratatui mapping.
//!
//! Converts `clarity_core::ui::render_line::RenderLine` into ratatui
//! `Line`/`Span` primitives so that `clarity-tui` shares the same semantic
//! line model as `clarity-egui`.
//!
//! Pipeline:
//!   markdown -> `clarity_core::ui::markdown_to_lines()` -> `Vec<RenderLine>`
//!     -> `render_line_to_ratatui()` -> `ratatui::text::Line`.

use clarity_core::ui::render_line::{DiffKind, LineRole, RenderLine, Span, StatusKind, ToolStatus};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span as RataSpan},
};

/// Convert a single `RenderLine` into a ratatui `Line`.
///
/// `theme_base` provides the default text style; semantic roles override
/// colours and modifiers as needed.
pub fn render_line_to_ratatui(line: &RenderLine, theme_base: Style) -> Line<'static> {
    match line {
        RenderLine::Text {
            spans,
            role,
            indent,
        } => render_text_line(spans, *role, *indent, theme_base),
        RenderLine::CodeLine {
            lang,
            content,
            line_no,
            diff,
        } => render_code_line(lang, content, *line_no, *diff),
        RenderLine::ToolCallHeader {
            name,
            status,
            expanded,
        } => render_tool_header(name, *status, *expanded),
        RenderLine::ToolCallArg { key, value } => render_tool_arg(key, value),
        RenderLine::Thinking { content, collapsed } => render_thinking(content, *collapsed),
        RenderLine::ApprovalPrompt { options } => render_approval(options),
        RenderLine::StatusLine {
            kind,
            content,
            transient,
        } => render_status(*kind, content, *transient),
        RenderLine::ArtifactRef {
            artifact_id,
            summary,
        } => render_artifact(artifact_id, summary),
        RenderLine::CrossInstanceRef {
            target_instance,
            target_session,
            message,
        } => render_cross_instance(target_instance, target_session.as_deref(), message),
        RenderLine::SlashCompletion {
            command,
            description,
        } => render_slash(command, description),
        RenderLine::StreamingCursor => Line::from(RataSpan::styled(
            "\u{258C}".to_string(),
            Style::default().fg(Color::Rgb(150, 200, 255)),
        )),
        RenderLine::Divider => Line::from(RataSpan::styled(
            "\u{2500}".repeat(40),
            Style::default().fg(Color::Rgb(80, 80, 100)),
        )),
        RenderLine::Empty => Line::from(""),
        RenderLine::BlockSlot {
            block_id,
            line_count,
        } => Line::from(RataSpan::styled(
            format!("[Block {} - {} lines]", block_id, line_count),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Rgb(150, 200, 255)),
        )),
    }
}

// ===== Variant-specific renderers =========================================

fn render_text_line(spans: &[Span], role: LineRole, indent: u8, base: Style) -> Line<'static> {
    let mut rata_spans: Vec<RataSpan<'static>> = Vec::with_capacity(spans.len() + 1);
    if indent > 0 {
        rata_spans.push(RataSpan::from(" ".repeat((indent as usize) * 2)));
    }
    for span in spans {
        rata_spans.push(span_to_ratatui(span, role, base));
    }
    Line::from(rata_spans)
}

fn render_code_line(
    lang: &str,
    content: &str,
    line_no: Option<u32>,
    diff: DiffKind,
) -> Line<'static> {
    let mut parts: Vec<RataSpan<'static>> = Vec::with_capacity(3);
    if let Some(n) = line_no {
        parts.push(RataSpan::styled(
            format!("{:>4} ", n),
            Style::default().fg(Color::Rgb(100, 100, 120)),
        ));
    }
    let content_style = match diff {
        DiffKind::Normal => Style::default().fg(Color::Rgb(200, 200, 220)),
        DiffKind::Added => Style::default().fg(Color::Rgb(100, 220, 140)),
        DiffKind::Removed => Style::default().fg(Color::Rgb(220, 100, 100)),
        DiffKind::Context => Style::default().fg(Color::Rgb(180, 180, 200)),
    };
    parts.push(RataSpan::styled(content.to_owned(), content_style));
    if !lang.is_empty() {
        parts.push(RataSpan::styled(
            format!("  [{}]", lang),
            Style::default().fg(Color::Rgb(120, 120, 140)),
        ));
    }
    Line::from(parts)
}

fn render_tool_header(name: &str, status: ToolStatus, expanded: bool) -> Line<'static> {
    let icon = match status {
        ToolStatus::Running => "\u{25B6}",
        ToolStatus::Success => "\u{2713}",
        ToolStatus::Warning => "\u{26A0}",
        ToolStatus::Error => "\u{2717}",
    };
    let expand_icon = if expanded { "\u{25BC}" } else { "\u{25B6}" };
    Line::from(vec![
        RataSpan::from(format!("{} {} ", expand_icon, icon)),
        RataSpan::styled(
            name.to_owned(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Rgb(150, 180, 220)),
        ),
    ])
}

fn render_tool_arg(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        RataSpan::styled(
            format!("  {}: ", key),
            Style::default().fg(Color::Rgb(180, 180, 200)),
        ),
        RataSpan::from(value.to_owned()),
    ])
}

fn render_thinking(content: &str, collapsed: bool) -> Line<'static> {
    let prefix = if collapsed { "\u{25B6} " } else { "\u{25BC} " };
    Line::from(vec![
        RataSpan::styled(
            prefix.to_string(),
            Style::default().fg(Color::Rgb(120, 120, 140)),
        ),
        RataSpan::styled(
            "Thinking... ".to_string(),
            Style::default()
                .add_modifier(Modifier::ITALIC)
                .fg(Color::Rgb(140, 140, 160)),
        ),
        RataSpan::styled(
            content.to_owned(),
            Style::default().fg(Color::Rgb(140, 140, 160)),
        ),
    ])
}

fn render_approval(options: &[clarity_core::ui::render_line::ApprovalOption]) -> Line<'static> {
    use clarity_core::ui::render_line::ApprovalOption;
    let mut parts: Vec<RataSpan<'static>> = vec![RataSpan::styled(
        "Approval required: ".to_string(),
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Rgb(220, 180, 100)),
    )];
    for (i, opt) in options.iter().enumerate() {
        let label = match opt {
            ApprovalOption::Yes => "Yes".to_string(),
            ApprovalOption::YesAndRemember => "Yes & remember".to_string(),
            ApprovalOption::No { reason_required } => {
                if *reason_required {
                    "No (reason)".to_string()
                } else {
                    "No".to_string()
                }
            }
            ApprovalOption::Custom(s) => s.to_string(),
        };
        parts.push(RataSpan::styled(
            format!(" [{}] {}  ", i + 1, label),
            Style::default().fg(Color::Rgb(200, 200, 220)),
        ));
    }
    Line::from(parts)
}

fn render_status(kind: StatusKind, content: &str, transient: bool) -> Line<'static> {
    let prefix = if transient { "\u{27F3} " } else { "\u{2022} " };
    let (color, kind_label) = match kind {
        StatusKind::Spinner => (Color::Rgb(150, 200, 255), String::new()),
        StatusKind::Progress { current, total } => (
            Color::Rgb(100, 220, 140),
            format!("[{}/{}] ", current, total),
        ),
        StatusKind::Network => (Color::Rgb(120, 120, 140), "[net] ".to_string()),
        StatusKind::Compaction => (Color::Rgb(220, 200, 100), "[compact] ".to_string()),
        StatusKind::ModelSwitch => (Color::Rgb(180, 200, 140), "[model] ".to_string()),
    };
    Line::from(vec![
        RataSpan::styled(prefix.to_string(), Style::default().fg(color)),
        RataSpan::styled(kind_label, Style::default().fg(color)),
        RataSpan::styled(content.to_owned(), Style::default().fg(color)),
    ])
}

fn render_artifact(artifact_id: &str, summary: &str) -> Line<'static> {
    Line::from(vec![
        RataSpan::styled(
            "[A] ".to_string(),
            Style::default().fg(Color::Rgb(150, 200, 255)),
        ),
        RataSpan::styled(
            artifact_id.to_owned(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Rgb(150, 200, 255)),
        ),
        RataSpan::styled(
            format!("  {}", summary),
            Style::default().fg(Color::Rgb(180, 180, 200)),
        ),
    ])
}

fn render_cross_instance(
    target_instance: &str,
    target_session: Option<&str>,
    message: &str,
) -> Line<'static> {
    Line::from(vec![
        RataSpan::styled(
            format!("\u{21B3} {} ", target_instance),
            Style::default().fg(Color::Rgb(150, 180, 220)),
        ),
        RataSpan::styled(
            target_session
                .map(|s| format!("({}) ", s))
                .unwrap_or_default(),
            Style::default().fg(Color::Rgb(120, 120, 140)),
        ),
        RataSpan::from(message.to_owned()),
    ])
}

fn render_slash(command: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        RataSpan::styled(
            format!("/{}  ", command),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Rgb(150, 200, 255)),
        ),
        RataSpan::styled(
            description.to_owned(),
            Style::default().fg(Color::Rgb(180, 180, 200)),
        ),
    ])
}

/// Convert a `Span` into a ratatui `Span`, applying role-specific styling.
fn span_to_ratatui(span: &Span, role: LineRole, base: Style) -> RataSpan<'static> {
    let text = span.text.as_str().to_owned();
    match role {
        LineRole::UserMessage => RataSpan::styled(text, base.fg(Color::Rgb(200, 200, 220))),
        LineRole::AgentMessage => RataSpan::styled(text, base.fg(Color::Rgb(220, 220, 240))),
        LineRole::SystemMessage => RataSpan::styled(
            text,
            base.fg(Color::Rgb(180, 180, 200))
                .add_modifier(Modifier::ITALIC),
        ),
        LineRole::ErrorMessage => RataSpan::styled(text, base.fg(Color::Rgb(220, 100, 100))),
        LineRole::Heading(level) => {
            let color = match level {
                1 => Color::Rgb(220, 220, 240),
                2 => Color::Rgb(200, 200, 230),
                3 => Color::Rgb(180, 180, 220),
                _ => Color::Rgb(160, 160, 200),
            };
            RataSpan::styled(text, base.fg(color).add_modifier(Modifier::BOLD))
        }
        LineRole::UnorderedListItem(_) | LineRole::OrderedListItem { .. } => {
            RataSpan::styled(text, base.fg(Color::Rgb(200, 200, 220)))
        }
        LineRole::Quote => RataSpan::styled(
            text,
            base.fg(Color::Rgb(180, 180, 200))
                .add_modifier(Modifier::ITALIC),
        ),
        LineRole::FileRef => RataSpan::styled(text, base.fg(Color::Rgb(150, 200, 255))),
        LineRole::Mention => RataSpan::styled(
            text,
            base.fg(Color::Rgb(150, 200, 255))
                .add_modifier(Modifier::BOLD),
        ),
        LineRole::Status => RataSpan::styled(text, base.fg(Color::Rgb(150, 200, 255))),
        LineRole::Warning => RataSpan::styled(text, base.fg(Color::Rgb(220, 200, 100))),
        LineRole::Note => RataSpan::styled(text, base.fg(Color::Rgb(180, 200, 140))),
        LineRole::TokenUsage => RataSpan::styled(text, base.fg(Color::Rgb(120, 120, 140))),
        LineRole::ContextCompaction => RataSpan::styled(text, base.fg(Color::Rgb(220, 200, 100))),
        LineRole::Sandbox => RataSpan::styled(text, base.fg(Color::Rgb(220, 180, 100))),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_core::ui::markdown_to_lines;

    #[test]
    fn empty_renders_empty_line() {
        let line = render_line_to_ratatui(&RenderLine::Empty, Style::default());
        // ratatui's `Line::from("")` produces a line with zero spans (no content),
        // which is the canonical empty line representation.
        let total_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(total_text.is_empty(), "empty line must have no content");
    }

    #[test]
    fn divider_renders_horizontal_rule() {
        let line = render_line_to_ratatui(&RenderLine::Divider, Style::default());
        assert!(line.spans[0].content.contains('\u{2500}'));
    }

    #[test]
    fn streaming_cursor_renders_block_char() {
        let line = render_line_to_ratatui(&RenderLine::StreamingCursor, Style::default());
        assert!(line.spans[0].content.contains('\u{258C}'));
    }

    #[test]
    fn markdown_pipeline_produces_lines() {
        let md = "# Heading\n\nParagraph.";
        let lines = markdown_to_lines(md);
        let rata: Vec<Line<'static>> = lines
            .iter()
            .map(|l| render_line_to_ratatui(l, Style::default()))
            .collect();
        assert!(rata.len() >= 2);
    }
}
