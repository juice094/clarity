use std::collections::HashMap;
use std::sync::Arc;

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

pub struct SettingsCommand;
impl CommandHandler for SettingsCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        use clarity_core::view_models::settings::SettingsViewModel;
        let vm = SettingsViewModel::new();
        app.cached_view_commands = vm.commands();
        app.settings_vm = Some(vm);
        app.settings_mode = true;
    }
    fn description(&self) -> &str {
        "打开设置面板"
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

pub struct SkillListCommand;
impl CommandHandler for SkillListCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        match &app.skill_registry {
            Some(reg) if !reg.is_empty() => {
                let mut lines = vec!["可用 Skills:".to_string()];
                for summary in reg.list_summaries() {
                    lines.push(format!("  • {}", summary));
                }
                if let Some(active) = app.active_skill() {
                    lines.push(String::new());
                    lines.push(format!("当前激活: {}", active));
                }
                app.messages
                    .push(Message::new(lines.join("\n"), MessageType::System));
            }
            _ => {
                app.messages
                    .push(Message::new("暂无已加载的 Skills。", MessageType::System));
            }
        }
    }
    fn description(&self) -> &str {
        "列出可用的 Skills"
    }
}

pub struct SkillUseCommand;
impl CommandHandler for SkillUseCommand {
    fn execute(&self, app: &mut App, args: &[&str]) {
        if args.is_empty() {
            match app.active_skill() {
                Some(id) => {
                    app.messages.push(Message::new(
                        format!("当前 Skill: {}", id),
                        MessageType::System,
                    ));
                }
                None => {
                    app.messages.push(Message::new(
                        "未激活任何 Skill。用法: /skill use <id>",
                        MessageType::System,
                    ));
                }
            }
            return;
        }
        let id = args.join(" ");
        if let Some(ref reg) = app.skill_registry {
            if reg.contains(&id) {
                app.set_active_skill(Some(id.clone()));
                app.messages.push(Message::new(
                    format!("已激活 Skill: {}", id),
                    MessageType::System,
                ));
                return;
            }
        }
        app.messages.push(Message::new(
            format!("未找到 Skill: {}", id),
            MessageType::System,
        ));
    }
    fn description(&self) -> &str {
        "激活或查看当前 Skill"
    }
}

pub struct TaskCommand;
impl CommandHandler for TaskCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.messages.push(Message::new(
            "用法: /task list | /task status <id> | /task cancel <id> | /task spawn <name> <prompt>",
            MessageType::System,
        ));
    }
    fn description(&self) -> &str {
        "后台任务管理 (list, status, cancel, spawn)"
    }
}

pub struct PlanCommand;
impl CommandHandler for PlanCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.messages.push(Message::new(
            "用法: /plan <query> — 让 Agent 生成结构化执行计划",
            MessageType::System,
        ));
    }
    fn description(&self) -> &str {
        "生成执行计划 (plan)"
    }
}

pub struct ExecuteCommand;
impl CommandHandler for ExecuteCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.messages.push(Message::new(
            "用法: /execute — 执行最近一次 /plan 生成的计划",
            MessageType::System,
        ));
    }
    fn description(&self) -> &str {
        "执行待处理计划 (execute)"
    }
}

pub struct ParallelCommand;
impl CommandHandler for ParallelCommand {
    fn execute(&self, app: &mut App, _args: &[&str]) {
        app.messages.push(Message::new(
            "用法: /parallel <type>:<prompt> [| <type>:<prompt>...]\n示例: /parallel coder:实现斐波那契函数 | explore:查找所有测试文件",
            MessageType::System,
        ));
    }
    fn description(&self) -> &str {
        "并行执行多个子代理 (parallel)"
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
    registry.register("/settings", Arc::new(SettingsCommand));

    registry.register("/stop", Arc::new(StopCommand));

    registry.register("/skill", Arc::new(SkillUseCommand));
    registry.alias("/skill use", "/skill");
    registry.register("/skills", Arc::new(SkillListCommand));

    registry.register("/task", Arc::new(TaskCommand));
    registry.register("/plan", Arc::new(PlanCommand));
    registry.register("/execute", Arc::new(ExecuteCommand));
    registry.register("/parallel", Arc::new(ParallelCommand));

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
        assert!(names.contains(&"/skill"));
        assert!(names.contains(&"/skills"));

        let handler = registry.get("/help").unwrap();
        assert!(!handler.description().is_empty());
    }
}
