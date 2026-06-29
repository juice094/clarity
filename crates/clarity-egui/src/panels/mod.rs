//! egui UI panels organized by physical screen region.
//!
//! Layout topology:
//!   - `navigation_tree/` — left navigation rail (S6 Pretext)
//!   - `chat/`            — center main view (chat stream + input)
//!   - `bot_bar/`         — bottom context bar
//!   - `right_ide_panel/` — right utility rail (IDE-style panels)
//!   - `settings/`        — center modal (settings tabs)
//!   - `modals/`          — top-layer blocking dialogs
//!   - `system/`          — global overlays (toast, dashboard)
//!   - `mcp.rs` / `skill.rs` — floating overlay panels
//!
//! The `legacy/` directory was removed in S6 cleanup. Gantt and TaskBoard
//! panels had no UI navigation path (dead code). MCP and Skill overlays
//! were promoted to top-level `panels/` modules.

pub mod bot_bar;
pub mod chat;
pub mod mcp;
pub mod modals;
pub mod navigation_tree;
pub mod right_ide_panel;
pub mod settings;
pub mod skill;
pub mod system;

// Re-exports for backward compatibility at existing call sites.
pub use modals::approval;
pub use modals::cron_create;
pub use modals::snapshot;
pub use modals::subagent_view;
pub use modals::task_create;
pub use modals::task_view;
pub use modals::team_create;
pub use system::dashboard;
pub use system::toast;
