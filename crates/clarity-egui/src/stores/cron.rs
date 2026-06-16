//! Cron Store
//!
//! cron-scheduled task list, creation modal

use std::time::Instant;

/// Holds cron UI state.
pub struct CronStore {
    pub tasks: Vec<clarity_core::background::cron::CronTask>,
    #[allow(dead_code)]
    pub last_refresh: Instant,
    pub create_name: String,
    pub create_desc: String,
    pub create_prompt: String,
    pub create_expr: String,
    pub create_priority: u8,
}
