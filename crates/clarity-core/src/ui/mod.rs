//! Pretext UI — 跨前端共享的信息架构层
//!
//! 本模块为 clarity-egui (GUI) 和 clarity-tui (TUI) 提供统一的视图状态、
//! 命令体系和面板语义。任何前端特有的渲染细节不应出现在此模块中。

pub mod commands;
pub mod render_line;
pub mod shortcut;
pub mod view_state;

pub use commands::{ids, CommandItem, CommandScope};
pub use render_line::{
    markdown_to_lines, render_line_plain_text, ApprovalOption, ArtifactId, BlockId, DiffKind,
    InstanceId, LineRole, RenderLine, SessionId, Span, SpanStyle, StatusKind, ToolStatus,
};
pub use shortcut::{KeyEvent, ShortcutBinding, ShortcutRegistry};
pub use view_state::{
    AppView, FocusScope, ModalType, PanelExpansion, PanelKind, SidePanel, TurnState, ViewState,
};
