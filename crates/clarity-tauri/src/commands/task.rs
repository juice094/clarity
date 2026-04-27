use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Internal task record stored in JSON file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskRecord {
    pub id: String,
    pub name: String,
    pub status: String,
    pub priority: String,
    pub created_at: String,
    pub updated_at: String,
}

static TASK_COUNTER: AtomicU64 = AtomicU64::new(0);

impl TaskRecord {
    fn tasks_file() -> PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("clarity");
        path.push("tasks.json");
        path
    }

    pub fn load_all() -> Vec<TaskRecord> {
        let path = Self::tasks_file();
        if !path.exists() {
            return Vec::new();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_all(tasks: &[TaskRecord]) -> Result<(), String> {
        let path = Self::tasks_file();
        std::fs::create_dir_all(path.parent().ok_or("invalid task directory")?).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(tasks).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}

fn generate_task_id() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        // SAFE: system time is always after UNIX_EPOCH.
        .unwrap()
        .as_millis();
    let counter = TASK_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("task-{}-{}", timestamp, counter)
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// List all active and recent tasks from persistent storage.
#[tauri::command]
pub fn list_tasks() -> Vec<TaskView> {
    TaskRecord::load_all()
        .into_iter()
        .map(|t| TaskView {
            id: t.id,
            name: t.name,
            status: t.status,
            priority: t.priority,
            created_at: t.created_at,
        })
        .collect()
}

/// Cancel a task by its ID.
#[tauri::command]
pub fn cancel_task(task_id: String) -> Result<(), String> {
    info!("cancel_task requested for {}", task_id);
    let mut tasks = TaskRecord::load_all();
    if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = "failed".to_string();
        task.updated_at = now_iso();
        TaskRecord::save_all(&tasks)?;
    }
    Ok(())
}

/// Create a new running task.
#[tauri::command]
pub fn create_task(name: String) -> Result<String, String> {
    let mut tasks = TaskRecord::load_all();
    let id = generate_task_id();
    let now = now_iso();
    let task = TaskRecord {
        id: id.clone(),
        name,
        status: "running".to_string(),
        priority: "normal".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    tasks.push(task);
    TaskRecord::save_all(&tasks)?;
    Ok(id)
}

/// Complete a task by updating its status.
#[tauri::command]
pub fn complete_task(id: String, status: String) -> Result<(), String> {
    let mut tasks = TaskRecord::load_all();
    if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
        task.status = status;
        task.updated_at = now_iso();
        TaskRecord::save_all(&tasks)?;
    }
    Ok(())
}
