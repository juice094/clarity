//! Settings Store (compatibility re-export).
//!
//! The canonical definitions now live in `clarity_apps` as part of P1c. This
//! module remains as a temporary shim so existing imports keep compiling during
//! the migration window.

pub use clarity_apps::{KimiCodeLoginState, SettingsStore};
