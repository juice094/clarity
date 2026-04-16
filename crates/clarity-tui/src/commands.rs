use std::collections::HashMap;
use std::sync::Arc;

use clarity_core::agent::Op;
use clarity_core::personality::{PersonalityConfig, YuanType};

use crate::app::{App, Message, MessageType};

pub trait CommandHandler: Send + Sync {
    fn execute(&self, app: &mut App, args: &[&str]);
    fn description(&self) -> &str;
}

pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn CommandHandler>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn CommandHandler>) {
        self.commands.insert(name.into(), handler);
    }

    pub fn alias(&mut self, alias: impl Into<String>, target: impl Into<String>) {
        self.aliases.insert(alias.into(), target.into());
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn CommandHandler>> {
        self.commands.get(name).cloned().or_else(|| {
            self.aliases
                .get(name)
                .and_then(|t| self.commands.get(t).cloned())
        })
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.commands.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }
}

pub struct ExitCommand;
impl CommandHandler for ExitCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.running = false;
    }
    fn description(&self) -> &str {
        "退出程序"
    }
}

pub struct ClearCommand;
impl CommandHandler for ClearCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.messages.clear();
        app.messages.push(Message::new(
            "屏幕已清空。输入 /help 查看可用命令。",
            MessageType::System,
        ));
        app.scroll_offset = 0;
    }
    fn description(&self) -> &str {
        "清空屏幕"
    }
}

pub struct HelpCommand;
impl CommandHandler for HelpCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        let mut lines = vec!["可用命令:".to_string()];
        for name in app.registry.names() {
            if let Some(handler) = app.registry.get(name) {
                lines.push(format!("  {} - {}", name, handler.description()));
            }
        }
        lines.push(String::new());
        lines.push("快捷键:".to_string());
        lines.push("  Ctrl+C              - 停止生成 / 退出".to_string());
        lines.push("  Ctrl+D              - 退出".to_string());
        lines.push("  ↑ / ↓               - 滚动聊天记录".to_string());
        lines.push("  Home / End          - 移动光标到行首/行尾".to_string());
        app.messages
            .push(Message::new(lines.join("\n"), MessageType::System));
    }
    fn description(&self) -> &str {
        "显示帮助"
    }
}

pub struct ModelCommand;
impl CommandHandler for ModelCommand {
    fn execute(&self, app: &mut App, args: &[&str]) {
        if args.is_empty() {
            app.messages.push(Message::new(
                format!("当前模型: {}", app.model_name),
                MessageType::System,
            ));
        } else {
            app.model_name = args.join(" ");
        }
    }
    fn description(&self) -> &str {
        "显示或设置当前模型"
    }
}

pub struct StopCommand;
impl CommandHandler for StopCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.stop_generation();
    }
    fn description(&self) -> &str {
        "停止生成"
    }
}

pub struct PersonalityCommand;
impl CommandHandler for PersonalityCommand {
    fn execute(&self, app: &mut App, args: &[&str]) {
        if args.is_empty() {
            app.messages.push(Message::new(
                format!(
                    "当前人格: {} (可用: direct, hanako, butter, ming)",
                    app.current_yuan_type
                ),
                MessageType::System,
            ));
            return;
        }

        let name = args[0].to_lowercase();
        match name.parse::<YuanType>() {
            Ok(yuan_type) => {
                app.current_yuan_type = yuan_type.to_string();
                let config = PersonalityConfig::new()
                    .with_agent_name("Clarity")
                    .with_user_name("User")
                    .with_yuan_type(yuan_type)
                    .with_locale("zh-CN");
                if let Some(ref tx) = app.controller_tx {
                    let _ = tx.send(Op::UpdatePersonality(config));
                }
                app.messages.push(Message::new(
                    format!("已切换人格至: {}", yuan_type),
                    MessageType::System,
                ));
            }
            Err(e) => {
                app.messages.push(Message::new(
                    format!("无效人格类型: {}。可用: direct, hanako, butter, ming", e),
                    MessageType::System,
                ));
            }
        }
    }
    fn description(&self) -> &str {
        "显示或切换人格 (direct/hanako/butter/ming)"
    }
}

pub fn build_default_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    registry.register("/exit", Arc::new(ExitCommand));
    registry.alias("/quit", "/exit");
    registry.alias("/q", "/exit");

    registry.register("/clear", Arc::new(ClearCommand));
    registry.alias("/cls", "/clear");

    registry.register("/help", Arc::new(HelpCommand));
    registry.alias("/h", "/help");

    registry.register("/model", Arc::new(ModelCommand));

    registry.register("/personality", Arc::new(PersonalityCommand));
    registry.alias("/p", "/personality");

    registry.register("/stop", Arc::new(StopCommand));

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_names_and_descriptions() {
        let registry = build_default_registry();
        let names = registry.names();
        assert!(!names.is_empty());
        assert!(names.contains(&"/help"));

        let handler = registry.get("/help").unwrap();
        assert!(!handler.description().is_empty());
    }
}
