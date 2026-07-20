//! Chat Store (compatibility re-export).
//!
//! The canonical definitions now live in `clarity_apps::chat` as part of the
//! P1c migration. This module remains as a temporary shim so existing imports
//! keep compiling during the gradual migration.
//!
//! ponytail: remove this shim once all call sites import from `clarity_apps::chat`.

pub use clarity_apps::chat::{ChatStore, TokenUsage};
