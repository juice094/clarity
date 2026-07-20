//! Re-export of GUI settings persistence now hosted in `clarity-apps`.
//!
//! The canonical implementation moved during P1c so `clarity-apps` can own the
//! Settings surfaces without depending on the egui host. This stub preserves
//! the legacy `crate::settings::*` import path during the migration window.

#![allow(missing_docs, unused_imports)]
pub use clarity_apps::settings_data::*;
pub use clarity_contract::settings::{
    AgentProfile, OpenClawAuthMode, OpenClawSendMethod, ProfilesFile, WebLink, WorkTemplate,
};
