use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
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

pub fn get_commands_for_app(_app: &App) -> Vec<CommandInfo> {
    vec![
        CommandInfo {
            name: "send",
            key: "Enter",
            description: "发送消息",
        },
        CommandInfo {
            name: "help",
            key: "?",
            description: "显示帮助",
        },
        CommandInfo {
            name: "quit",
            key: "q",
            description: "退出",
        },
        CommandInfo {
            name: "scroll",
            key: "↑/↓",
            description: "滚动聊天",
        },
        CommandInfo {
            name: "stop",
            key: "Ctrl+C",
            description: "停止生成",
        },
    ]
}

pub fn render_command_bar(f: &mut Frame, area: Rect, commands: &[CommandInfo]) {
    let spans: Vec<Span> = commands
        .iter()
        .enumerate()
        .flat_map(|(i, cmd)| {
            let mut parts = vec![Span::styled(
                format!("{}:{}", cmd.key, cmd.name),
                Style::default().fg(Color::Cyan),
            )];
            if i + 1 < commands.len() {
                parts.push(Span::raw(" | "));
            }
            parts
        })
        .collect();
    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
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
        assert!(commands.iter().any(|c| c.name == "help"));
    }

    #[test]
    fn test_command_info_fields() {
        let cmd = CommandInfo {
            name: "test",
            key: "t",
            description: "test desc",
        };
        assert_eq!(cmd.name, "test");
        assert_eq!(cmd.key, "t");
        assert_eq!(cmd.description, "test desc");
    }
}
