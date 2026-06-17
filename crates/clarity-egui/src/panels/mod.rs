//! egui UI panels organized by physical screen region.
//!
//! Layout topology:
//!   - `navigation_tree/` — left navigation rail (S6 Pretext)
//!   - `chat/`            — center main view (chat stream + input)
//!   - `work/`            — center main view (OpenClaw project orchestration)
//!   - `bot_bar/`         — bottom context bar
//!   - `right_ide_panel/` — right utility rail (IDE-style panels)
//!   - `settings/`        — center modal (settings tabs)
//!   - `modals/`          — top-layer blocking dialogs
//!   - `system/`          — global overlays (toast, dashboard)
//!   - `legacy/`          — panels awaiting redesign or integration into the new layout
//!
//! Backward-compatibility re-exports are provided at the bottom so existing
//! call sites (e.g. `panels::approval`) keep compiling during migration.

pub mod bot_bar;
pub mod chat;
pub mod legacy;
pub mod modals;
pub mod navigation_tree;
pub mod right_ide_panel;
pub mod settings;
pub mod system;
pub mod work;

// -----------------------------------------------------------------------------
// Backward-compatibility re-exports (remove once all call sites are migrated)
// -----------------------------------------------------------------------------
pub use legacy::gantt;
pub use legacy::mcp;
pub use legacy::skill;
pub use legacy::task_board;
pub use modals::approval;
pub use modals::cron_create;
pub use modals::snapshot;
pub use modals::subagent_view;
pub use modals::task_create;
pub use modals::task_view;
pub use modals::team_create;
pub use system::dashboard;
pub use system::toast;
