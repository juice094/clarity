use crate::app::Message;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// 聊天区域组件
pub struct ChatPane<'a> {
    messages: &'a [Message],
    scroll_offset: usize,
}

impl<'a> ChatPane<'a> {
    pub fn new(messages: &'a [Message], scroll_offset: usize) -> Self {
        Self {
            messages,
            scroll_offset,
        }
    }

    #[allow(dead_code)]
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    #[allow(dead_code)]
    pub fn scroll_down(&mut self) {
        let max_scroll = self.messages.len().saturating_sub(1);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    #[allow(dead_code)]
    pub fn messages(&self) -> &[Message] {
        self.messages
    }

    #[allow(dead_code)]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
}

impl<'a> Widget for ChatPane<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Chat ")
            .title_alignment(ratatui::layout::Alignment::Left);

        let inner_area = block.inner(area);
        block.render(area, buf);

        if self.messages.is_empty() {
            return;
        }

        let mut lines: Vec<Line> = vec![];

        for msg in self.messages.iter().skip(self.scroll_offset) {
            lines.push(Line::from(""));

            match msg.msg_type {
                crate::app::MessageType::User => {
                    let prefix = Span::styled(
                        "  You ",
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    );
                    let time = Span::styled(
                        format!("({}) ", msg.timestamp),
                        Style::default().fg(Color::DarkGray),
                    );
                    lines.push(Line::from(vec![prefix, time]));

                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::raw("  "), Span::raw(line)]));
                    }
                }
                crate::app::MessageType::Assistant => {
                    let prefix = if msg.is_streaming {
                        Span::styled(
                            "  🤖 ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(
                            "  Clarity ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    };
                    let time = Span::styled(
                        format!("({}) ", msg.timestamp),
                        Style::default().fg(Color::DarkGray),
                    );
                    lines.push(Line::from(vec![prefix, time]));

                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::raw("  "), Span::raw(line)]));
                    }

                    if msg.is_streaming {
                        lines.push(Line::from(vec![Span::styled(
                            "  ▌",
                            Style::default().fg(Color::Green),
                        )]));
                    }
                }
                crate::app::MessageType::System => {
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {} ", line),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
                crate::app::MessageType::ToolCall => {
                    lines.push(Line::from(vec![
                        Span::styled("  🔧 ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            &msg.content,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }
            }
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .scroll((0, 0));

        paragraph.render(inner_area, buf);
    }
}
