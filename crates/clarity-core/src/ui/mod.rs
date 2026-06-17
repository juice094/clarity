//! Pretext UI — 跨前端共享的信息架构层
//!
//! 本模块为 clarity-egui (GUI) 和 clarity-tui (TUI) 提供统一的视图状态、
//! 命令体系和面板语义。任何前端特有的渲染细节不应出现在此模块中。

/// Commands module.
pub mod commands;
pub mod render_line;
pub mod shortcut;
pub mod view_state;

pub use commands::{CommandItem, CommandScope, ids};
pub use render_line::{
    ApprovalOption, ArtifactId, BlockId, DiffKind, InstanceId, LineRole, RenderLine, SessionId,
    Span, SpanStyle, StatusKind, ToolStatus, markdown_to_lines, render_line_plain_text,
};
pub use shortcut::{KeyEvent, ShortcutBinding, ShortcutRegistry};
pub use view_state::{
    AppView, FocusScope, LeftRailSection, ModalType, PanelExpansion, PanelKind, RightRailCard,
    RightRailContext, RightRailPanel, RightRailSection, SidePanel, TurnState, ViewState,
};
