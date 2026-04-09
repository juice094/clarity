use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Clear,
    Frame,
};

use crate::app::App;
use crate::command_bar;
use crate::popup;
use crate::widgets::{
    chat_pane::ChatPane,
    generating_indicator::GeneratingIndicator,
    status_bar::StatusBar,
};

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

    // 状态栏
    let status_bar = StatusBar::new(&app.model_name, &app.session_id);
    status_bar.render(f, chunks[0]);

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
        GeneratingIndicator::render(f, size);
    }

    // 弹窗（最上层）
    if let Some(ref popup) = app.popup {
        let (w, h) = popup.preferred_size();
        let area = popup::centered_rect(w, h, size);
        f.render_widget(Clear, area);
        popup.draw(f, area);
    }
}
