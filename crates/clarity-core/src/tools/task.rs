//! Task management tools: TaskCreate, TaskList, TaskOutput, TaskStop

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

use crate::background::BackgroundTaskManager;
use crate::background::store::{TaskSpec, TaskStatus, TaskStore};
use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

fn task_store_path() -> ToolResult<std::path::PathBuf> {
    super::clarity_data_dir().map(|p| p.join("tasks"))
}

/// Resolve the task store: the shared [`BackgroundTaskManager`] store when
/// bound (keeps tools, REST API, and the worker pool on one directory),
/// otherwise the legacy `~/.clarity/tasks` directory.
fn resolve_store(manager: &Option<Arc<BackgroundTaskManager>>) -> ToolResult<TaskStore> {
    match manager {
        Some(m) => Ok(m.store().clone()),
        None => Ok(TaskStore::new(task_store_path()?)),
    }
}

/// Tool for listing background tasks
pub struct TaskListTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl TaskListTool {
    /// Create a new TaskListTool instance using the legacy store directory.
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`] store.
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
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
        let store = resolve_store(&self.manager)?;

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
pub struct TaskOutputTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl TaskOutputTool {
    /// Create a new TaskOutputTool instance using the legacy store directory.
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`] store.
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
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
        let store = resolve_store(&self.manager)?;

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
pub struct TaskCreateTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl TaskCreateTool {
    /// Create a new TaskCreateTool instance using the legacy store directory.
    ///
    /// Tasks created this way are persisted as `pending` but have no consumer;
    /// hosts should prefer [`Self::with_manager`] so created tasks are spawned.
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`].
    ///
    /// When the manager has an [`AgentTaskExecutor`](crate::background::AgentTaskExecutor)
    /// configured, newly created tasks are spawned immediately instead of
    /// dead-lettering as `pending`.
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
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

        // Bound to a BackgroundTaskManager with an executor: spawn immediately
        // (persists + runs under the manager's concurrency limit).
        if let Some(manager) = &self.manager {
            if manager.agent_executor().is_some() {
                manager
                    .spawn_agent_with_id(task_id.clone(), spec)
                    .await
                    .map_err(|e| {
                        ToolError::execution_failed(format!("Failed to spawn task: {}", e))
                    })?;

                return Ok(json!({
                    "success": true,
                    "task_id": task_id,
                    "name": name,
                    "status": "running",
                    "message": format!("Task '{}' created and spawned with ID {}", name, task_id)
                }));
            }
        }

        // No manager (legacy path) or no executor configured: persist as
        // pending in the resolved store.
        // ponytail: without an executor there is no pending→spawn consumer;
        // upgrade path is a store-scanning scheduler loop in BackgroundTaskManager.
        let store = resolve_store(&self.manager)?;

        store
            .create(&task_id, spec)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to create task: {}", e)))?;

        let status_note = if self.manager.is_some() {
            " (no agent executor configured; task will remain pending)"
        } else {
            ""
        };

        Ok(json!({
            "success": true,
            "task_id": task_id,
            "name": name,
            "status": "pending",
            "message": format!("Task '{}' created with ID {}{}", name, task_id, status_note)
        }))
    }
}

/// Tool for stopping/cancelling a background task
pub struct TaskStopTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl TaskStopTool {
    /// Create a new TaskStopTool instance using the legacy store directory.
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`] store.
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
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
        let store = resolve_store(&self.manager)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::new()
    }

    #[test]
    fn task_list_tool_metadata() {
        let tool = TaskListTool::new();
        assert_eq!(tool.name(), "task_list");
        assert!(tool.description().contains("List all background tasks"));
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(params["properties"].get("status_filter").is_some());
        assert_eq!(params["required"], json!([]));
    }

    #[test]
    fn task_output_tool_metadata() {
        let tool = TaskOutputTool::new();
        assert_eq!(tool.name(), "task_output");
        assert!(tool.description().contains("result of a background task"));
        let params = tool.parameters();
        assert!(params["properties"].get("task_id").is_some());
        assert_eq!(params["required"], json!(["task_id"]));
    }

    #[test]
    fn task_create_tool_metadata() {
        let tool = TaskCreateTool::new();
        assert_eq!(tool.name(), "task_create");
        assert!(tool.description().contains("background task"));
        let params = tool.parameters();
        assert_eq!(params["required"], json!(["name", "prompt"]));
        assert!(params["properties"].get("priority").is_some());
    }

    #[test]
    fn task_stop_tool_metadata() {
        let tool = TaskStopTool::new();
        assert_eq!(tool.name(), "task_stop");
        assert!(tool.description().contains("Cancelled"));
        let params = tool.parameters();
        assert_eq!(params["required"], json!(["task_id"]));
    }

    #[tokio::test]
    async fn task_list_tool_returns_array_for_all_status_filters() {
        let tool = TaskListTool::new();
        for filter in [
            "pending",
            "running",
            "completed",
            "failed",
            "all",
            "invalid",
        ] {
            let result = tool
                .execute(json!({"status_filter": filter}), ctx())
                .await
                .unwrap();
            assert!(
                result.is_array(),
                "filter '{}' should return an array",
                filter
            );
        }

        // No filter defaults to the all branch.
        let result = tool.execute(json!({}), ctx()).await.unwrap();
        assert!(result.is_array());
    }

    #[tokio::test]
    async fn task_output_tool_reports_missing_result() {
        let tool = TaskOutputTool::new();
        let result = tool
            .execute(json!({"task_id": "does-not-exist"}), ctx())
            .await
            .unwrap();
        assert_eq!(result["exists"], false);
        assert_eq!(result["task_id"], "does-not-exist");
    }

    #[tokio::test]
    async fn task_stop_tool_errors_when_task_missing() {
        let tool = TaskStopTool::new();
        let result = tool
            .execute(json!({"task_id": "does-not-exist"}), ctx())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn task_create_tool_requires_name_and_prompt() {
        let tool = TaskCreateTool::new();
        assert!(tool.execute(json!({"name": "x"}), ctx()).await.is_err());
        assert!(tool.execute(json!({"prompt": "y"}), ctx()).await.is_err());
        assert!(tool.execute(json!({}), ctx()).await.is_err());
    }

    // NOTE: TaskCreateTool::execute without a manager writes to the hard-coded
    // Clarity data directory (via super::clarity_data_dir()). On Windows this
    // resolves to the user's profile via a Windows API call and cannot be
    // redirected with an environment variable in a unit test, so the legacy
    // successful-creation path is not exercised here. The manager-bound paths
    // below use tempdirs and cover create→spawn end to end.

    use crate::background::AgentTaskExecutor;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct MockExecutor;

    #[async_trait]
    impl AgentTaskExecutor for MockExecutor {
        async fn execute(
            &self,
            spec: &crate::background::TaskSpec,
        ) -> anyhow::Result<(String, usize)> {
            Ok((format!("done: {}", spec.name), 1))
        }
    }

    fn manager_with_executor(temp: &TempDir) -> BackgroundTaskManager {
        BackgroundTaskManager::new(
            temp.path().join("store"),
            temp.path().join("work"),
            temp.path().join("context"),
        )
        .with_agent_executor(Arc::new(MockExecutor))
    }

    #[tokio::test]
    async fn task_create_with_manager_spawns_and_executes() {
        let temp = TempDir::new().unwrap();
        let manager = Arc::new(manager_with_executor(&temp));
        let tool = TaskCreateTool::with_manager(manager.clone());

        let result = tool
            .execute(json!({"name": "demo", "prompt": "do the thing"}), ctx())
            .await
            .unwrap();
        assert_eq!(result["status"], "running");
        let task_id = result["task_id"].as_str().unwrap().to_string();

        // The created task must actually run to completion (no dead-letter).
        let task_result = manager.wait(&task_id).await.unwrap();
        assert_eq!(task_result.status, TaskStatus::Completed);
        assert_eq!(task_result.output, "done: demo");
    }

    #[tokio::test]
    async fn task_create_with_manager_without_executor_stays_pending() {
        let temp = TempDir::new().unwrap();
        let manager = Arc::new(BackgroundTaskManager::new(
            temp.path().join("store"),
            temp.path().join("work"),
            temp.path().join("context"),
        ));
        let tool = TaskCreateTool::with_manager(manager.clone());

        let result = tool
            .execute(json!({"name": "demo", "prompt": "do the thing"}), ctx())
            .await
            .unwrap();
        assert_eq!(result["status"], "pending");
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("no agent executor")
        );

        // Pending task is visible through the same store the manager uses.
        let listed = TaskListTool::with_manager(manager.clone())
            .execute(json!({"status_filter": "pending"}), ctx())
            .await
            .unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn task_tools_bound_to_manager_share_store() {
        let temp = TempDir::new().unwrap();
        let manager = Arc::new(manager_with_executor(&temp));

        let created = TaskCreateTool::with_manager(manager.clone())
            .execute(json!({"name": "shared", "prompt": "p"}), ctx())
            .await
            .unwrap();
        let task_id = created["task_id"].as_str().unwrap().to_string();
        manager.wait(&task_id).await.unwrap();

        // task_output reads the result saved by the manager's worker.
        let output = TaskOutputTool::with_manager(manager.clone())
            .execute(json!({"task_id": task_id}), ctx())
            .await
            .unwrap();
        assert_eq!(output["status"], "completed");
        assert_eq!(output["output"], "done: shared");
    }
}
