//! egui UI panels organized by physical screen region.
//!
//! Layout topology:
//!   - `chat/`      — center main view (chat stream + input)
//!   - `work/`      — center main view (OpenClaw project orchestration)
//!   - `settings/`  — center modal (settings tabs)
//!   - `sidebar/`   — left navigation rail
//!   - `workspace/` — right side panel
//!   - `modals/`    — top-layer blocking dialogs
//!   - `system/`    — global overlays (toast, dashboard)
//!   - `legacy/`    — panels awaiting redesign or integration into the new layout
//!
//! Backward-compatibility re-exports are provided at the bottom so existing
//! call sites (e.g. `panels::approval`) keep compiling during migration.

pub mod chat;
pub mod legacy;
pub mod modals;
pub mod settings;
pub mod sidebar;
pub mod system;
pub mod work;
pub mod workspace;

// -----------------------------------------------------------------------------
// Backward-compatibility re-exports (remove once all call sites are migrated)
// -----------------------------------------------------------------------------
pub use legacy::gantt;
pub use legacy::mcp;
pub use legacy::skill;
pub use legacy::task_board;
pub use legacy::team;
pub use modals::approval;
pub use modals::cron_create;
pub use modals::snapshot;
pub use modals::subagent_view;
pub use modals::task_create;
pub use modals::task_view;
pub use modals::team_create;
pub use system::dashboard;
pub use system::toast;
