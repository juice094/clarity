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

        let mut lines: Vec<Line> = vec![];

        for msg in self.messages.iter_mut() {
            match msg.msg_type {
                MessageType::User => {
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
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  ▎ ", Style::default().fg(Color::Rgb(80, 140, 220))),
                            Span::styled(line, Style::default().fg(Color::Rgb(200, 210, 240))),
                        ]));
                    }
                }
                MessageType::Assistant => {
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
                    lines.push(Line::from(""));
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
