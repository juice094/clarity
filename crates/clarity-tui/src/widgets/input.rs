use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

/// 输入框组件
pub struct InputWidget<'a> {
    input: &'a str,
    cursor_position: usize,
}

impl<'a> InputWidget<'a> {
    pub fn new(input: &'a str, cursor_position: usize) -> Self {
        Self {
            input,
            cursor_position,
        }
    }
}

impl<'a> Widget for InputWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 创建带边框的块
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Input ")
            .title_alignment(ratatui::layout::Alignment::Left);

        let inner_area = block.inner(area);
        block.render(area, buf);

        // 渲染输入提示符和内容
        let prompt = "> ";
        let content = format!("{}{}", prompt, self.input);

        // 计算光标位置
        let cursor_pos_in_line = prompt.width() + self.input[..self.cursor_position.min(self.input.len())].width();

        // 渲染内容
        let line = Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(self.input),
        ]);

        let paragraph = Paragraph::new(line);
        paragraph.render(inner_area, buf);

        // 设置光标位置
        if inner_area.width > 0 && inner_area.height > 0 {
            let cursor_x = inner_area.left() + (cursor_pos_in_line as u16).min(inner_area.width - 1);
            let cursor_y = inner_area.top();
            buf.set_style(
                Rect::new(cursor_x, cursor_y, 1, 1),
                Style::default().bg(Color::Cyan).fg(Color::Black),
            );
        }
    }
}
