use std::time::Instant;

use crate::stores::TaskStore;

pub fn on_task_list(task_store: &mut TaskStore, tasks: Vec<clarity_core::background::TaskInfo>) {
    task_store.tasks = tasks;
    task_store.last_task_refresh = Instant::now();
}
