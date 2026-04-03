use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// 生成中指示器组件
pub struct GeneratingIndicator;

impl GeneratingIndicator {
    pub fn render(f: &mut Frame, _area: Rect) {
        let size = f.size();
        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Length(3),
                Constraint::Percentage(45),
            ])
            .split(size)[1];

        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(40),
                Constraint::Percentage(30),
            ])
            .split(popup_area)[1];

        let dots = ["", ".", "..", "..."];
        let dot_idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / 500) as usize
            % 4;

        let text = format!("🤔 思考中{} (Ctrl+C 停止)", dots[dot_idx]);

        let popup = Paragraph::new(text)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Generating ")
                    .title_alignment(Alignment::Center),
            );

        f.render_widget(Clear, popup_area);
        f.render_widget(popup, popup_area);
    }
}
