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

/// 命令 ID 常量 — kebab-case 字符串字面量，shortcut 与 palette 共用此清单。
///
/// 引入时机：P0.5.C.1 — 统一 ShortcutAction 与 CommandItem 的标识符来源。
/// Phase 0.5 之前，shortcut enum 与 palette built_in 各自维护字符串字面量，
/// 出现 6 处不匹配。此清单消除分歧。
pub mod ids {
    // ── Session / Turn ──
    pub const NEW_SESSION: &str = "new-session";
    pub const STOP_GENERATION: &str = "stop-generation";
    pub const SEND_MESSAGE: &str = "send-message";

    // ── Modal / View ──
    pub const CLOSE_MODAL: &str = "close-modal";
    pub const OPEN_SETTINGS: &str = "open-settings";

    // ── Panels ──
    pub const TOGGLE_SIDEBAR: &str = "toggle-sidebar";
    pub const TOGGLE_SKILL_PANEL: &str = "toggle-skill-panel";
    pub const TOGGLE_TEAM_PANEL: &str = "toggle-team-panel";
    pub const TOGGLE_DASHBOARD: &str = "toggle-dashboard";

    // ── Input / Palette ──
    pub const FOCUS_INPUT: &str = "focus-input";
    pub const TOGGLE_COMMAND_PALETTE: &str = "toggle-command-palette";

    // ── Line-mode navigation (S7 Phase 2D) ──
    pub const NAVIGATE_DOWN: &str = "navigate-down";
    pub const NAVIGATE_UP: &str = "navigate-up";
    pub const NAVIGATE_TOP: &str = "navigate-top";
    pub const NAVIGATE_BOTTOM: &str = "navigate-bottom";
    pub const COPY_LINE: &str = "copy-line";
}

/// 预置的常用命令常量 — 避免两端硬编码重复。
pub mod built_in {
    use super::*;

    pub fn new_session() -> CommandItem {
        CommandItem::new(ids::NEW_SESSION, "New Session", CommandScope::Global)
            .with_shortcut("Ctrl+N")
    }

    pub fn stop_generation() -> CommandItem {
        CommandItem::new(ids::STOP_GENERATION, "Stop Generation", CommandScope::Global)
            .with_shortcut("Ctrl+C")
    }

    pub fn send_message() -> CommandItem {
        CommandItem::new(ids::SEND_MESSAGE, "Send Message", CommandScope::Global)
            .with_shortcut("Ctrl+Enter")
    }

    pub fn toggle_sidebar() -> CommandItem {
        CommandItem::new(ids::TOGGLE_SIDEBAR, "Toggle Sidebar", CommandScope::Global)
            .with_shortcut("Ctrl+B")
    }

    pub fn focus_input() -> CommandItem {
        CommandItem::new(ids::FOCUS_INPUT, "Focus Input", CommandScope::Global)
            .with_shortcut("Ctrl+K")
    }

    pub fn open_settings() -> CommandItem {
        CommandItem::new(ids::OPEN_SETTINGS, "Settings", CommandScope::Global)
            .with_shortcut("Esc")
    }

    pub fn close_modal() -> CommandItem {
        CommandItem::new(ids::CLOSE_MODAL, "Close Modal / Return to Chat", CommandScope::Global)
            .with_shortcut("Esc")
    }

    pub fn toggle_skill_panel() -> CommandItem {
        CommandItem::new(ids::TOGGLE_SKILL_PANEL, "Toggle Skill Panel", CommandScope::Global)
            .with_shortcut("Ctrl+.")
    }

    pub fn toggle_team_panel() -> CommandItem {
        CommandItem::new(ids::TOGGLE_TEAM_PANEL, "Toggle Team Panel", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+T")
    }

    pub fn toggle_dashboard() -> CommandItem {
        CommandItem::new(ids::TOGGLE_DASHBOARD, "Toggle Dashboard", CommandScope::Global)
            .with_shortcut("Ctrl+Shift+D")
    }

    pub fn toggle_command_palette() -> CommandItem {
        CommandItem::new(
            ids::TOGGLE_COMMAND_PALETTE,
            "Toggle Command Palette",
            CommandScope::Global,
        )
        .with_shortcut("Ctrl+Shift+P")
    }

    /// 返回所有内置命令列表。
    pub fn all() -> Vec<CommandItem> {
        vec![
            new_session(),
            stop_generation(),
            send_message(),
            close_modal(),
            open_settings(),
            toggle_sidebar(),
            focus_input(),
            toggle_skill_panel(),
            toggle_team_panel(),
            toggle_dashboard(),
            toggle_command_palette(),
        ]
    }
}
