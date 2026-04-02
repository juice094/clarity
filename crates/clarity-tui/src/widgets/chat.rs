use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{Message, MessageType};

/// 聊天区域组件
pub struct ChatWidget<'a> {
    messages: &'a [Message],
    scroll_offset: usize,
}

impl<'a> ChatWidget<'a> {
    pub fn new(messages: &'a [Message], scroll_offset: usize) -> Self {
        Self {
            messages,
            scroll_offset,
        }
    }
}

impl<'a> Widget for ChatWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 创建带边框的块
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

        // 渲染消息
        let mut lines: Vec<Line> = vec![];

        for msg in self.messages.iter().skip(self.scroll_offset) {
            // 空行
            lines.push(Line::from(""));

            // 根据消息类型渲染
            match msg.msg_type {
                MessageType::User => {
                    // 用户消息
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

                    // 内容
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::raw("  "), Span::raw(line)]));
                    }
                }
                MessageType::Assistant => {
                    // 助手消息
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

                    // 内容
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::raw("  "), Span::raw(line)]));
                    }

                    // 流式指示器
                    if msg.is_streaming {
                        lines.push(Line::from(vec![Span::styled(
                            "  ▌",
                            Style::default().fg(Color::Green),
                        )]));
                    }
                }
                MessageType::System => {
                    // 系统消息
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {} ", line),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
                MessageType::ToolCall => {
                    // 工具调用消息
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

        // 创建段落并渲染
        let paragraph = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .scroll((0, 0));

        paragraph.render(inner_area, buf);
    }
}
