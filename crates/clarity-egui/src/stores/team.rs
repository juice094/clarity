//! Team Store (compatibility re-export).
//!
//! The canonical definitions now live in `clarity_apps::dashboard` as part of
//! the P1c migration. This module remains as a temporary shim so existing
//! imports keep compiling.

pub use clarity_apps::dashboard::{Team, TeamMember, TeamStore};
