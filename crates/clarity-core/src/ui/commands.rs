use crate::ui::view_state::AppView;

/// 全局可执行操作的原子定义 — 两端各自渲染，共享同一语义。
///
/// GUI 将其渲染为浮动 CommandPalette；
/// TUI 将其渲染为底部固定的 CommandBar。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandItem {
    /// 全局唯一标识（kebab-case，如 "new-session"）
    pub id: String,
    /// 显示名称（人类可读）
    pub name: String,
    /// 可见范围
    pub scope: CommandScope,
    /// 快捷键（GUI 显示，TUI 可忽略）
    pub shortcut: Option<String>,
}

impl CommandItem {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        scope: CommandScope,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            scope,
            shortcut: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// 命令可见性范围 — 决定命令在哪些上下文中出现。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandScope {
    /// 任何视图、任何状态下可见
    Global,
    /// 仅在指定主视图下可见
    View(AppView),
    /// 仅在特定上下文（如选中了某行、焦点在某面板）下可见
    Context(&'static str),
}

/// 命令过滤前缀协议 — GUI/TUI 统一。
pub mod prefix {
    /// 执行命令（默认前缀）
    pub const EXECUTE: &str = ">";
    /// 引用资源
    pub const RESOURCE: &str = "@";
    /// 切换视图
    pub const VIEW: &str = "#";
    /// 搜索帮助
    pub const HELP: &str = "?";
}

/// 预置的常用命令常量 — 避免两端硬编码重复。
pub mod built_in {
    use super::*;

    pub fn new_session() -> CommandItem {
        CommandItem::new("new-session", "New Session", CommandScope::Global)
            .with_shortcut("Ctrl+N")
    }

    pub fn stop_generation() -> CommandItem {
        CommandItem::new("stop-generation", "Stop Generation", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+S")
    }

    pub fn toggle_sidebar() -> CommandItem {
        CommandItem::new("toggle-sidebar", "Toggle Sidebar", CommandScope::Global)
            .with_shortcut("Ctrl+B")
    }

    pub fn focus_input() -> CommandItem {
        CommandItem::new("focus-input", "Focus Input", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+F")
    }

    pub fn open_settings() -> CommandItem {
        CommandItem::new("open-settings", "Settings", CommandScope::Global)
            .with_shortcut("Esc")
    }

    pub fn toggle_skill_panel() -> CommandItem {
        CommandItem::new("toggle-skill-panel", "Toggle Skill Panel", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+L")
    }

    pub fn toggle_team_panel() -> CommandItem {
        CommandItem::new("toggle-team-panel", "Toggle Team Panel", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+T")
    }

    pub fn toggle_dashboard() -> CommandItem {
        CommandItem::new("toggle-dashboard", "Toggle Dashboard", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+D")
    }

    /// 返回所有内置命令列表。
    pub fn all() -> Vec<CommandItem> {
        vec![
            new_session(),
            stop_generation(),
            toggle_sidebar(),
            focus_input(),
            open_settings(),
            toggle_skill_panel(),
            toggle_team_panel(),
            toggle_dashboard(),
        ]
    }
}
