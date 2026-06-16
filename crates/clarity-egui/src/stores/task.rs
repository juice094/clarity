//! Task Store
//!
//! background task list, creation modal

use std::time::Instant;

/// Holds task UI state.
pub struct TaskStore {
    pub tasks: Vec<clarity_core::background::TaskInfo>,
    pub last_task_refresh: Instant,
    pub task_create_name: String,
    pub task_create_desc: String,
    pub task_create_prompt: String,
    pub task_create_priority: u8,
    /// ID of the task whose result is being viewed.
    pub viewing_task_id: Option<String>,
    /// Fetched result for the viewing task.
    pub viewing_task_result: Option<clarity_core::background::TaskResult>,
}
