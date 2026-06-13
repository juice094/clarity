pub mod agent_turn;
pub mod chat;
pub mod file_preview_overlay;
pub mod thinking_log;
pub mod tools_section;
pub mod web_tabs;

// Backward-compatibility re-exports (moved to panels/ during module reorganization)
pub use crate::panels::settings;
