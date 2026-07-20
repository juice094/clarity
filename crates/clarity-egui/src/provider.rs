//! Re-export of the provider schema now hosted in `clarity-apps`.
//!
//! The canonical implementation moved during P1c so `clarity-apps` can own the
//! Settings surfaces without depending on the egui host. This stub preserves
//! the legacy `crate::provider::*` import path during the migration window.

#![allow(missing_docs, unused_imports)]
pub use clarity_apps::provider::*;
