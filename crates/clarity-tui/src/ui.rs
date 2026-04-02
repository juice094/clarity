use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::widgets::chat::ChatWidget;
use crate::widgets::input::InputWidget;

/// 渲染主界面
pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.size();

    // 主布局: 顶部状态栏 + 聊天区域 + 输入框
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // 状态栏
            Constraint::Min(5),         // 聊天区域
            Constraint::Length(app.input_height), // 输入框
        ])
        .split(size);

    // 渲染状态栏
    draw_status_bar(f, app, chunks[0]);

    // 渲染聊天区域
    draw_chat_area(f, app, chunks[1]);

    // 渲染输入框
    draw_input_area(f, app, chunks[2]);

    // 如果有生成中的提示，显示
    if app.is_generating {
        draw_generating_indicator(f, app);
    }
}

/// 渲染状态栏
fn draw_status_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let status_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let status_text = format!(
        " Clarity v0.1.0 │ Model: {} │ Session: {} ",
        app.model_name,
        &app.session_id[..16.min(app.session_id.len())]
    );

    let status_bar = Paragraph::new(status_text)
        .style(status_style)
        .alignment(Alignment::Left);

    f.render_widget(status_bar, area);
}

/// 渲染聊天区域
fn draw_chat_area(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let chat_widget = ChatWidget::new(&app.messages, app.scroll_offset);
    f.render_widget(chat_widget, area);
}

/// 渲染输入区域
fn draw_input_area(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let input_widget = InputWidget::new(&app.input, app.cursor_position);
    f.render_widget(input_widget, area);
}

/// 渲染生成中指示器
fn draw_generating_indicator(f: &mut Frame, _app: &App) {
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
