use crate::app::{App, AppMode};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// A single entry shown in the bottom command bar.
pub struct CommandInfo {
    /// Display label for the action.
    pub name: &'static str,
    /// Key shortcut that triggers the action.
    pub key: &'static str,
}

/// Build the list of command-bar hints for the current application state.
pub fn get_commands_for_app(app: &App) -> Vec<CommandInfo> {
    if app.is_generating() {
        vec![CommandInfo {
            name: "停止生成",
            key: "Ctrl+C",
        }]
    } else if app.mode == AppMode::Normal {
        vec![
            CommandInfo {
                name: "输入",
                key: "i",
            },
            CommandInfo {
                name: "帮助",
                key: "?",
            },
            CommandInfo {
                name: "滚动",
                key: "↑/↓",
            },
            CommandInfo {
                name: "退出",
                key: "q",
            },
        ]
    } else {
        vec![
            CommandInfo {
                name: "发送",
                key: "Enter",
            },
            CommandInfo {
                name: "返回",
                key: "Esc",
            },
            CommandInfo {
                name: "停止",
                key: "Ctrl+C",
            },
        ]
    }
}

/// Render the command bar centered at the bottom of the given area.
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
