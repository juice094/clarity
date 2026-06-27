use crate::app::{Message, MessageType};
use clarity_tui::render_line::render_line_to_ratatui;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use std::time::Instant;

/// Format an elapsed duration as a short relative time label.
///
/// ponytail: use simple threshold buckets — no chrono, no i18n.
fn relative_label(created: Instant) -> String {
    let secs = created.elapsed().as_secs();
    match secs {
        0..=5 => "just now".into(),
        6..=59 => format!("{}s ago", secs),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

/// 聊天区域组件
pub struct ChatPane<'a> {
    messages: &'a mut [Message],
    scroll_offset: usize,
}

impl<'a> ChatPane<'a> {
    /// Create a new chat pane bound to the given message slice and scroll offset.
    pub fn new(messages: &'a mut [Message], scroll_offset: usize) -> Self {
        Self {
            messages,
            scroll_offset,
        }
    }
}

impl<'a> Widget for ChatPane<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
            .title(" Chat ")
            .title_alignment(ratatui::layout::Alignment::Left);

        let inner_area = block.inner(area);
        block.render(area, buf);

        if self.messages.is_empty() {
            return;
        }

        // ── Viewport culling ──
        // Skip messages entirely below the visible area to avoid building
        // ratatui Lines that will be clipped by the terminal anyway.
        // Messages above are still rendered — ratatui's Paragraph::scroll()
        // handles skipping them correctly.
        let visible_budget = inner_area.height as usize + self.scroll_offset;

        let mut lines: Vec<Line> = Vec::new();
        let mut prev_role: Option<MessageType> = None;
        let mut line_estimate = 0usize;

        for msg in self.messages.iter_mut() {
            // Early exit: past visible area.
            if line_estimate >= visible_budget + 64 {
                break;
            }

            let show_header = prev_role.as_ref() != Some(&msg.msg_type);
            prev_role = Some(msg.msg_type.clone());

            line_estimate += msg.line_count();

            match msg.msg_type {
                MessageType::User => {
                    if show_header {
                        lines.push(Line::from(""));
                        let time = Span::styled(
                            format!(" {} ", relative_label(msg.created_at)),
                            Style::default().fg(Color::Rgb(100, 100, 120)),
                        );
                        lines.push(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                " You ",
                                Style::default()
                                    .fg(Color::Rgb(220, 230, 255))
                                    .bg(Color::Rgb(50, 100, 180))
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(" "),
                            time,
                        ]));
                    } else {
                        // Light separator between consecutive same-role messages.
                        lines.push(Line::from(Span::styled(
                            "  ┈".to_string(),
                            Style::default().fg(Color::Rgb(50, 50, 65)),
                        )));
                    }
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  ▎ ", Style::default().fg(Color::Rgb(80, 140, 220))),
                            Span::styled(line, Style::default().fg(Color::Rgb(200, 210, 240))),
                        ]));
                    }
                }
                MessageType::Assistant => {
                    if show_header {
                        lines.push(Line::from(""));
                        let prefix = if msg.is_streaming {
                            Span::styled(" ● ", Style::default().fg(Color::Rgb(100, 220, 150)))
                        } else {
                            Span::styled(" ● ", Style::default().fg(Color::Rgb(80, 180, 140)))
                        };
                        let name = Span::styled(
                            " Clarity ",
                            Style::default()
                                .fg(Color::Rgb(220, 255, 235))
                                .bg(Color::Rgb(40, 120, 80))
                                .add_modifier(Modifier::BOLD),
                        );
                        let time = Span::styled(
                            format!(" {} ", relative_label(msg.created_at)),
                            Style::default().fg(Color::Rgb(100, 100, 120)),
                        );
                        lines.push(Line::from(vec![
                            Span::raw(" "),
                            prefix,
                            name,
                            Span::raw(" "),
                            time,
                        ]));
                    } else {
                        // Light separator between consecutive same-role messages.
                        lines.push(Line::from(Span::styled(
                            "  ┈".to_string(),
                            Style::default().fg(Color::Rgb(50, 50, 65)),
                        )));
                    }

                    // S7 Phase 3A: cached markdown parse — only re-parses
                    // when content changes (streaming). Avoids repeated parsing
                    // every frame for static messages.
                    let render_lines = msg.render_lines();
                    let agent_base = Style::default().fg(Color::Rgb(210, 230, 220));
                    for rl in render_lines {
                        let rata_line = render_line_to_ratatui(rl, agent_base);
                        let mut spans = vec![Span::styled(
                            "  ▎ ",
                            Style::default().fg(Color::Rgb(80, 180, 140)),
                        )];
                        spans.extend(rata_line.spans);
                        lines.push(Line::from(spans));
                    }

                    if msg.is_streaming {
                        lines.push(Line::from(vec![Span::styled(
                            "  ▌",
                            Style::default().fg(Color::Rgb(100, 220, 150)),
                        )]));
                    }
                }
                MessageType::System => {
                    lines.push(Line::from(""));
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {} ", line),
                            Style::default()
                                .fg(Color::Rgb(140, 140, 160))
                                .add_modifier(Modifier::ITALIC),
                        )]));
                    }
                }
                MessageType::ToolCall => {
                    if show_header {
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(vec![
                        Span::styled("  ▶ ", Style::default().fg(Color::Rgb(220, 180, 80))),
                        Span::styled(
                            &msg.content,
                            Style::default()
                                .fg(Color::Rgb(240, 220, 160))
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }
            }
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));

        paragraph.render(inner_area, buf);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── relative_label ──

    #[test]
    fn relative_label_just_now() {
        let now = Instant::now();
        assert_eq!(relative_label(now), "just now");
    }

    #[test]
    fn relative_label_seconds() {
        let ten_sec_ago = Instant::now().checked_sub(Duration::from_secs(10)).unwrap();
        assert_eq!(relative_label(ten_sec_ago), "10s ago");
    }

    #[test]
    fn relative_label_minutes() {
        let five_min_ago = Instant::now()
            .checked_sub(Duration::from_secs(300))
            .unwrap();
        assert_eq!(relative_label(five_min_ago), "5m ago");
    }

    #[test]
    fn relative_label_hours() {
        let three_hr_ago = Instant::now()
            .checked_sub(Duration::from_secs(10800))
            .unwrap();
        assert_eq!(relative_label(three_hr_ago), "3h ago");
    }

    #[test]
    fn relative_label_days() {
        let two_days_ago = Instant::now()
            .checked_sub(Duration::from_secs(172800))
            .unwrap();
        assert_eq!(relative_label(two_days_ago), "2d ago");
    }

    #[test]
    fn relative_label_boundary_5_to_6() {
        let at_5 = Instant::now().checked_sub(Duration::from_secs(5)).unwrap();
        let at_6 = Instant::now().checked_sub(Duration::from_secs(6)).unwrap();
        assert_eq!(relative_label(at_5), "just now");
        assert_eq!(relative_label(at_6), "6s ago");
    }

    // ── Layout integration tests ──

    /// Extract plain text from rendered ratatui Lines.
    fn collect_text(lines: &[Line]) -> String {
        lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn consecutive_same_role_has_separator() {
        let mut msgs = vec![
            Message::new("first message", MessageType::Assistant),
            Message::new("second message", MessageType::Assistant),
        ];
        let pane = ChatPane::new(&mut msgs, 0);
        let lines = render_to_lines(pane);
        let text = collect_text(&lines);
        // Second message should have separator (no header).
        assert!(
            text.contains("┈"),
            "expected separator between same-role msgs"
        );
        // Only one Clarity header.
        assert_eq!(
            text.matches("Clarity").count(),
            1,
            "only first msg should have header"
        );
    }

    #[test]
    fn role_switch_has_header() {
        let mut msgs = vec![
            Message::new("user says hi", MessageType::User),
            Message::new("assistant replies", MessageType::Assistant),
        ];
        let pane = ChatPane::new(&mut msgs, 0);
        let lines = render_to_lines(pane);
        let text = collect_text(&lines);
        assert!(text.contains("You"), "user header missing");
        assert!(text.contains("Clarity"), "assistant header missing");
    }

    #[test]
    fn streaming_message_shows_cursor() {
        let mut msgs = vec![Message::new("thinking...", MessageType::Assistant).streaming()];
        let pane = ChatPane::new(&mut msgs, 0);
        let lines = render_to_lines(pane);
        let text = collect_text(&lines);
        assert!(text.contains('▌'), "streaming cursor missing");
    }

    #[test]
    fn markdown_paragraph_renders() {
        let mut msgs = vec![Message::new(
            "**bold** and `code` and *italic*",
            MessageType::Assistant,
        )];
        let pane = ChatPane::new(&mut msgs, 0);
        let lines = render_to_lines(pane);
        let text = collect_text(&lines);
        assert!(text.contains("bold"), "bold text present");
        assert!(text.contains("code"), "code text present");
    }

    #[test]
    fn system_message_no_role_header() {
        let mut msgs = vec![Message::new("system info", MessageType::System)];
        let pane = ChatPane::new(&mut msgs, 0);
        let lines = render_to_lines(pane);
        let text = collect_text(&lines);
        assert!(
            !text.contains("Clarity"),
            "system msg should not have Clarity header"
        );
        assert!(
            !text.contains("You"),
            "system msg should not have You header"
        );
        assert!(text.contains("system info"), "content present");
    }
}

/// Helper: render a ChatPane and collect the output Lines.
#[cfg(test)]
fn render_to_lines(pane: ChatPane<'_>) -> Vec<Line<'static>> {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    let area = Rect::new(0, 0, 80, 40);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    // Reconstruct Lines from the buffer's cell content.
    let mut lines: Vec<Line> = Vec::new();
    for y in 0..area.height {
        let row: String = (0..area.width)
            .map(|x| buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
            .collect::<String>()
            .trim_end()
            .to_string();
        if !row.is_empty() {
            lines.push(Line::from(row));
        }
    }
    lines
}
