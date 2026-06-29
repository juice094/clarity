pub mod agent_turn;
pub mod chat;
pub mod file_preview_overlay;

// NOTE: `panels::settings` was previously re-exported here as a backward-
// compatibility shim after a module reorganization. Callers should import
// `crate::panels::settings` directly. The re-export has been removed to
// eliminate the layering violation where the low-level `components` module
// depended on the higher-level `panels` layer.
