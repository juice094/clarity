use std::time::Instant;

use crate::stores::TaskStore;

/// Handles the task list event.
pub fn on_task_list(task_store: &mut TaskStore, tasks: Vec<clarity_core::background::TaskInfo>) {
    task_store.tasks = tasks;
    task_store.last_task_refresh = Instant::now();
}

/// 处理 Gateway 推送的后台任务状态变更。
///
/// ponytail: 目前直接触发一次任务列表刷新；高频任务场景下可改为差量更新。
pub fn on_background_task_update(
    app: &crate::App,
    task_id: String,
    _task_name: String,
    status: String,
) {
    tracing::debug!("Background task {} status changed to {}", task_id, status);
    app.refresh_tasks();
}
