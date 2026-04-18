use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::command_bar;
use crate::popup;
use crate::widgets::{chat_pane::ChatPane, generating_indicator::GeneratingIndicator};

/// 渲染主界面
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                // 状态栏
            Constraint::Min(5),                   // 聊天区域
            Constraint::Length(app.input_height), // 输入框
            Constraint::Length(1),                // 命令栏
        ])
        .split(size);

    // 状态栏 - 深色背景 + 三段式布局
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(chunks[0]);

    let status_dot = if app.is_generating { "◐" } else { "●" };
    let dot_color = if app.is_generating {
        Color::Rgb(255, 200, 80)
    } else {
        Color::Rgb(100, 220, 120)
    };

    let left = Paragraph::new(Line::from(vec![
        Span::styled(
            status_dot,
            Style::default().fg(dot_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " Clarity",
            Style::default()
                .fg(Color::Rgb(200, 200, 220))
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .alignment(Alignment::Left);

    let center = Paragraph::new(Line::from(vec![
        Span::styled("Model: ", Style::default().fg(Color::Rgb(140, 140, 160))),
        Span::styled(
            &app.model_name,
            Style::default()
                .fg(Color::Rgb(220, 220, 240))
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .alignment(Alignment::Center);

    let session_short = &app.session_id[..8.min(app.session_id.len())];
    let right_text = if let Some((prompt, completion, total)) = app.session_usage {
        format!(
            "{} │ {}↑ {}↓ {}∑",
            session_short, prompt, completion, total
        )
    } else {
        session_short.to_string()
    };
    let right = Paragraph::new(Line::from(vec![
        Span::styled("Session: ", Style::default().fg(Color::Rgb(140, 140, 160))),
        Span::styled(
            right_text,
            Style::default().fg(Color::Rgb(180, 180, 200)),
        ),
    ]))
    .alignment(Alignment::Right);

    f.render_widget(left, header_chunks[0]);
    f.render_widget(center, header_chunks[1]);
    f.render_widget(right, header_chunks[2]);

    // 聊天区域
    let chat_pane = ChatPane::new(&app.messages, app.scroll_offset);
    f.render_widget(chat_pane, chunks[1]);

    // 输入框
    f.render_widget(&app.input_pane, chunks[2]);

    // 命令栏
    let commands = command_bar::get_commands_for_app(app);
    command_bar::render_command_bar(f, chunks[3], &commands);

    // 生成中指示器
    if app.is_generating {
        GeneratingIndicator::render(f, size, app.generation_metrics.as_ref());
    }

    // 弹窗（最上层）
    if let Some(ref popup) = app.popup {
        let (w, h) = popup.preferred_size();
        let area = popup::centered_rect(w, h, size);
        f.render_widget(Clear, area);
        popup.draw(f, area);
    }
}
