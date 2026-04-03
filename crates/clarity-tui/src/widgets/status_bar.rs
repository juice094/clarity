use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};

/// 状态栏组件
pub struct StatusBar {
    model_name: String,
    session_id: String,
}

impl StatusBar {
    pub fn new(model_name: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            session_id: session_id.into(),
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let status_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        let status_text = format!(
            " Clarity v0.1.0 │ Model: {} │ Session: {} ",
            self.model_name,
            &self.session_id[..16.min(self.session_id.len())]
        );

        let status_bar = Paragraph::new(status_text)
            .style(status_style)
            .alignment(Alignment::Left);

        f.render_widget(status_bar, area);
    }
}
