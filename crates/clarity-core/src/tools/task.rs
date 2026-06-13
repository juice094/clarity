//! Task management tools: TaskList, TaskOutput, TaskStop

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use crate::background::store::{TaskSpec, TaskStatus, TaskStore};
use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

fn task_store_path() -> ToolResult<std::path::PathBuf> {
    super::clarity_data_dir().map(|p| p.join("tasks"))
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
            _ => store.list_all().await,
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

        let result_opt = store.get_result_opt(task_id).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to read task result: {}", e))
        })?;

        match result_opt {
            Some(result) => {
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
            None => Ok(json!({
                "exists": false,
                "task_id": task_id,
                "message": format!("Task '{}' has no result yet or does not exist", task_id)
            })),
        }
    }
}

/// Tool for creating a new background task
///
/// Use this when the user wants to defer work to run asynchronously,
/// schedule a recurring analysis, or spawn a long-running sub-agent.
pub struct TaskCreateTool;

impl TaskCreateTool {
    /// Create a new TaskCreateTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn description(&self) -> &str {
        "Create a new background task that will be executed asynchronously. \
         Returns the task ID and initial status. Use this for long-running \
         operations, scheduled work, or delegating to a sub-agent."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Short name for the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt or instruction for the agent to execute"
                },
                "description": {
                    "type": "string",
                    "description": "Optional longer description of what the task does"
                },
                "agent_type": {
                    "type": "string",
                    "description": "Agent type to use: explore, coder, plan, or default (default: default)"
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum number of agent iterations (default: 10)"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 300)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["background", "low", "normal", "high", "critical"],
                    "description": "Task priority (default: normal)"
                },
                "model_alias": {
                    "type": "string",
                    "description": "Optional model alias override"
                }
            },
            "required": ["name", "prompt"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let name = helpers::required_str(&args, "name")?;
        let prompt = helpers::required_str(&args, "prompt")?;

        let mut spec = TaskSpec::new(name, prompt);

        if let Some(desc) = helpers::optional_str(&args, "description") {
            spec = spec.with_description(desc);
        }
        if let Some(agent_type) = helpers::optional_str(&args, "agent_type") {
            spec = spec.with_agent_type(agent_type);
        }
        if let Some(max) = args.get("max_iterations").and_then(|v| v.as_u64()) {
            spec = spec.with_max_iterations(max as usize);
        }
        if let Some(timeout) = args.get("timeout_seconds").and_then(|v| v.as_u64()) {
            spec = spec.with_timeout_seconds(timeout);
        }
        if let Some(alias) = helpers::optional_str(&args, "model_alias") {
            spec = spec.with_model_alias(alias);
        }

        let priority = helpers::optional_str(&args, "priority").unwrap_or("normal");
        let priority_enum = match priority {
            "background" => crate::background::store::TaskPriority::Background,
            "low" => crate::background::store::TaskPriority::Low,
            "normal" => crate::background::store::TaskPriority::Normal,
            "high" => crate::background::store::TaskPriority::High,
            "critical" => crate::background::store::TaskPriority::Critical,
            _ => crate::background::store::TaskPriority::Normal,
        };
        spec = spec.with_priority(priority_enum);

        let task_id = uuid::Uuid::new_v4().to_string();
        let store = TaskStore::new(task_store_path()?);

        store
            .create(&task_id, spec)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to create task: {}", e)))?;

        Ok(json!({
            "success": true,
            "task_id": task_id,
            "name": name,
            "status": "pending",
            "message": format!("Task '{}' created with ID {}", name, task_id)
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
