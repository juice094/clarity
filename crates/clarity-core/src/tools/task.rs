//! Task management tools: TaskList, TaskOutput, TaskStop

use async_trait::async_trait;
use serde_json::json;
use serde_json::Value;

use crate::background::store::{TaskStatus, TaskStore};
use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

fn task_store_path() -> ToolResult<std::path::PathBuf> {
    dirs::home_dir()
        .map(|p| p.join(".clarity").join("tasks"))
        .ok_or_else(|| ToolError::execution_failed("Could not determine home directory"))
}

/// Tool for listing background tasks
pub struct TaskListTool;

impl TaskListTool {
    /// Create a new TaskListTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn description(&self) -> &str {
        "List all background tasks. Supports filtering by status. \
         Returns a JSON array of tasks with id, name, description, status, and created_at."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status_filter": {
                    "type": "string",
                    "description": "Filter tasks by status: pending, running, completed, failed, or all (default: all)",
                    "enum": ["pending", "running", "completed", "failed", "all"]
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let store = TaskStore::new(task_store_path()?);

        let filter = helpers::optional_str(&args, "status_filter").unwrap_or("all");

        let tasks = match filter {
            "pending" => store.list_by_status(TaskStatus::Pending).await,
            "running" => store.list_by_status(TaskStatus::Running).await,
            "completed" => store.list_by_status(TaskStatus::Completed).await,
            "failed" => store.list_by_status(TaskStatus::Failed).await,
            "all" | _ => store.list_all().await,
        }
        .map_err(|e| ToolError::execution_failed(format!("Failed to list tasks: {}", e)))?;

        let tasks_json: Vec<Value> = tasks
            .into_iter()
            .map(|t| {
                json!({
                    "id": t.id,
                    "name": t.spec.name,
                    "description": t.spec.description,
                    "status": t.status.as_str(),
                    "created_at": t.created_at,
                })
            })
            .collect();

        Ok(Value::Array(tasks_json))
    }
}

/// Tool for getting task output/result
pub struct TaskOutputTool;

impl TaskOutputTool {
    /// Create a new TaskOutputTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskOutputTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "task_output"
    }

    fn description(&self) -> &str {
        "Get the result of a background task by its ID. \
         Returns status, output (truncated to 5000 chars if too long), elapsed_ms, and steps."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the task to get output for"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let task_id = helpers::required_str(&args, "task_id")?;
        let store = TaskStore::new(task_store_path()?);

        let result = store
            .get_result(task_id)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Task not found: {}", e)))?;

        let output = if result.output.len() > 5000 {
            format!("{}...(truncated)", &result.output[..5000])
        } else {
            result.output
        };

        Ok(json!({
            "status": result.status.as_str(),
            "output": output,
            "elapsed_ms": result.elapsed_ms,
            "steps": result.steps,
        }))
    }
}

/// Tool for stopping/cancelling a background task
pub struct TaskStopTool;

impl TaskStopTool {
    /// Create a new TaskStopTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskStopTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "task_stop"
    }

    fn description(&self) -> &str {
        "Stop a background task by updating its status to Cancelled. \
         Returns a success message or error if the task is not found."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the task to stop"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let task_id = helpers::required_str(&args, "task_id")?;
        let store = TaskStore::new(task_store_path()?);

        store
            .update_status(task_id, TaskStatus::Cancelled)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to stop task: {}", e)))?;

        Ok(json!({
            "success": true,
            "message": format!("Task '{}' has been cancelled", task_id)
        }))
    }
}
