use crate::app::{App, AppMode};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub struct CommandInfo {
    pub name: &'static str,
    pub key: &'static str,
    #[allow(dead_code)]
    pub description: &'static str,
}

pub fn get_commands_for_app(app: &App) -> Vec<CommandInfo> {
    if app.is_generating() {
        vec![CommandInfo {
            name: "停止生成",
            key: "Ctrl+C",
            description: "停止生成",
        }]
    } else if app.mode == AppMode::Normal {
        vec![
            CommandInfo {
                name: "输入",
                key: "i",
                description: "进入输入模式",
            },
            CommandInfo {
                name: "帮助",
                key: "?",
                description: "显示帮助",
            },
            CommandInfo {
                name: "滚动",
                key: "↑/↓",
                description: "滚动聊天",
            },
            CommandInfo {
                name: "退出",
                key: "q",
                description: "退出程序",
            },
        ]
    } else {
        vec![
            CommandInfo {
                name: "发送",
                key: "Enter",
                description: "发送消息",
            },
            CommandInfo {
                name: "返回",
                key: "Esc",
                description: "返回正常模式",
            },
            CommandInfo {
                name: "停止",
                key: "Ctrl+C",
                description: "停止生成",
            },
        ]
    }
}

pub fn render_command_bar(f: &mut Frame, area: Rect, commands: &[CommandInfo]) {
    let spans: Vec<Span> = commands
        .iter()
        .enumerate()
        .flat_map(|(i, cmd)| {
            let mut parts = vec![
                Span::styled(
                    cmd.key,
                    Style::default()
                        .fg(Color::Rgb(255, 200, 100))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", cmd.name),
                    Style::default().fg(Color::Rgb(180, 180, 200)),
                ),
            ];
            if i + 1 < commands.len() {
                parts.push(Span::styled(
                    "  •  ",
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                ));
            }
            parts
        })
        .collect();
    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    #[test]
    fn test_get_commands_not_empty() {
        let app = App::default();
        let commands = get_commands_for_app(&app);
        assert!(!commands.is_empty());
    }
}
