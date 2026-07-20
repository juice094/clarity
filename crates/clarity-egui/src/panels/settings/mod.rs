//! Settings tab content implementations (compatibility re-export).
//!
//! The canonical implementations moved to `clarity_apps::settings_panels`
//! during P1c. This stub preserves the legacy `crate::panels::settings::*`
//! import path during the migration window.

#![allow(missing_docs, unused_imports)]
pub use clarity_apps::settings_panels::*;
