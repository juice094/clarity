use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

/// 输入框组件
/// 
/// cursor_position 是按字符计数的位置（Unicode scalar value）
/// 所有字符串操作都需要先将字符位置转换为字节索引
pub struct InputPane {
    input: String,
    cursor_position: usize, // Character position, NOT byte index
}

impl InputPane {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
        }
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Convert character position to byte index
    fn char_pos_to_byte_idx(&self, char_pos: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_pos)
            .map(|(idx, _)| idx)
            .unwrap_or(self.input.len())
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_idx = self.char_pos_to_byte_idx(self.cursor_position);
        self.input.insert(byte_idx, c);
        self.cursor_position += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            let byte_idx = self.char_pos_to_byte_idx(self.cursor_position);
            // Get the char at this position to know its byte length
            if let Some(c) = self.input.chars().nth(self.cursor_position) {
                self.input.drain(byte_idx..byte_idx + c.len_utf8());
            }
        }
    }

    pub fn delete_char_forward(&mut self) {
        let byte_idx = self.char_pos_to_byte_idx(self.cursor_position);
        if byte_idx < self.input.len() {
            // Get the char at this position to know its byte length
            if let Some(c) = self.input.chars().nth(self.cursor_position) {
                self.input.drain(byte_idx..byte_idx + c.len_utf8());
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input.chars().count() {
            self.cursor_position += 1;
        }
    }

    pub fn set_cursor_position(&mut self, pos: usize) {
        self.cursor_position = pos.min(self.input.chars().count());
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor_position = 0;
    }
}

impl Widget for &InputPane {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Input ")
            .title_alignment(ratatui::layout::Alignment::Left);

        let inner_area = block.inner(area);
        block.render(area, buf);

        let prompt = "> ";
        // Calculate display width up to cursor position (handle multi-byte UTF-8 chars)
        let text_before_cursor: String = self.input.chars().take(self.cursor_position).collect();
        let cursor_pos_in_line = prompt.width() + text_before_cursor.width();

        let line = Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(&self.input),
        ]);

        let paragraph = Paragraph::new(line);
        paragraph.render(inner_area, buf);

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
