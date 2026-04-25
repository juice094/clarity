use tracing::info;

/// Task view model exposed to the frontend.
#[derive(serde::Serialize)]
pub struct TaskView {
    pub id: String,
    pub name: String,
    pub status: String,
    pub priority: String,
    pub created_at: String,
}

/// List all active and recent tasks (mock data).
#[tauri::command]
pub fn list_tasks() -> Vec<TaskView> {
    vec![
        TaskView {
            id: "task-001".to_string(),
            name: "Code review: agent.rs".to_string(),
            status: "running".to_string(),
            priority: "high".to_string(),
            created_at: "2026-04-25T08:00:00Z".to_string(),
        },
        TaskView {
            id: "task-002".to_string(),
            name: "Sync repository metadata".to_string(),
            status: "pending".to_string(),
            priority: "medium".to_string(),
            created_at: "2026-04-25T08:05:00Z".to_string(),
        },
        TaskView {
            id: "task-003".to_string(),
            name: "Index memory chunks".to_string(),
            status: "completed".to_string(),
            priority: "low".to_string(),
            created_at: "2026-04-25T07:30:00Z".to_string(),
        },
        TaskView {
            id: "task-004".to_string(),
            name: "Background garbage collection".to_string(),
            status: "failed".to_string(),
            priority: "medium".to_string(),
            created_at: "2026-04-25T07:45:00Z".to_string(),
        },
        TaskView {
            id: "task-005".to_string(),
            name: "Export conversation log".to_string(),
            status: "running".to_string(),
            priority: "high".to_string(),
            created_at: "2026-04-25T08:10:00Z".to_string(),
        },
    ]
}

/// Cancel a task by its ID (mock implementation).
#[tauri::command]
pub fn cancel_task(task_id: String) -> Result<(), String> {
    info!("cancel_task requested for {}", task_id);
    Ok(())
}
