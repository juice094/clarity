//! Cron scheduling tools for the Agent
//!
//! Allows the Agent to register, list, and cancel recurring tasks using cron expressions.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::background::BackgroundTaskManager;
use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

/// Shared manager reference used by all cron tools.
fn require_manager(
    manager: &Option<Arc<BackgroundTaskManager>>,
) -> ToolResult<Arc<BackgroundTaskManager>> {
    manager.clone().ok_or_else(|| {
        ToolError::execution_failed(
            "BackgroundTaskManager not configured for cron tool. \
             Please ensure the application initialized a BackgroundTaskManager \
             and called agent.with_cron_manager() before invoking cron tools."
                .to_string(),
        )
    })
}

/// Tool for scheduling recurring agent tasks via cron expressions.
pub struct ScheduleCronTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl ScheduleCronTool {
    /// Create a new tool instance without a manager reference.
    ///
    /// The tool will return an error at execution time unless a manager
    /// is provided via [`Self::with_manager`].
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`].
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
    }
}

impl Default for ScheduleCronTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ScheduleCronTool {
    fn check_readiness(&self) -> Option<String> {
        if self.manager.is_none() {
            Some("BackgroundTaskManager not configured. Call agent.with_cron_manager() first.".to_string())
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "schedule_cron"
    }

    fn description(&self) -> &str {
        "Schedule a recurring agent task using a cron expression. \
         The task will be automatically spawned when the cron expression triggers."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_name": {
                    "type": "string",
                    "description": "Name of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt/instruction for the agent task"
                },
                "cron_expr": {
                    "type": "string",
                    "description": "Cron expression in standard format, e.g. '0 0 2 * * *' for daily at 2am UTC"
                }
            },
            "required": ["task_name", "prompt", "cron_expr"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let task_name = helpers::required_str(&args, "task_name")?;
        let prompt = helpers::required_str(&args, "prompt")?;
        let cron_expr = helpers::required_str(&args, "cron_expr")?;

        let manager = require_manager(&self.manager)?;

        let spec =
            crate::background::store::TaskSpec::new(task_name, prompt).with_agent_type("default");

        let task_id = manager.schedule_cron(spec, cron_expr).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to schedule cron task: {}", e))
        })?;

        Ok(json!({
            "task_id": task_id,
            "cron_expr": cron_expr,
            "task_name": task_name,
        }))
    }
}

/// Tool for listing all scheduled cron tasks.
pub struct ListCronTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl ListCronTool {
    /// Create a new tool instance without a manager reference.
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`].
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
    }
}

impl Default for ListCronTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListCronTool {
    fn check_readiness(&self) -> Option<String> {
        if self.manager.is_none() {
            Some("BackgroundTaskManager not configured. Call agent.with_cron_manager() first.".to_string())
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "list_cron"
    }

    fn description(&self) -> &str {
        "List all scheduled cron tasks with their IDs, expressions, and next run times."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let manager = require_manager(&self.manager)?;

        let tasks = manager.list_cron_tasks().await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to list cron tasks: {}", e))
        })?;

        let items: Vec<Value> = tasks
            .into_iter()
            .map(|t| {
                json!({
                    "task_id": t.task_id,
                    "task_name": t.task_spec.name,
                    "cron_expr": t.schedule.expr,
                    "next_run": t.schedule.next_run.to_rfc3339(),
                    "enabled": t.enabled,
                })
            })
            .collect();

        Ok(json!({ "tasks": items, "count": items.len() }))
    }
}

/// Tool for cancelling a scheduled cron task.
pub struct CancelCronTool {
    manager: Option<Arc<BackgroundTaskManager>>,
}

impl CancelCronTool {
    /// Create a new tool instance without a manager reference.
    pub fn new() -> Self {
        Self { manager: None }
    }

    /// Create a tool bound to a specific [`BackgroundTaskManager`].
    pub fn with_manager(manager: Arc<BackgroundTaskManager>) -> Self {
        Self {
            manager: Some(manager),
        }
    }
}

impl Default for CancelCronTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CancelCronTool {
    fn check_readiness(&self) -> Option<String> {
        if self.manager.is_none() {
            Some("BackgroundTaskManager not configured. Call agent.with_cron_manager() first.".to_string())
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "cancel_cron"
    }

    fn description(&self) -> &str {
        "Cancel (remove) a scheduled cron task by its task ID."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the cron task to cancel"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let task_id = helpers::required_str(&args, "task_id")?;

        let manager = require_manager(&self.manager)?;

        manager.cancel_cron(task_id).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to cancel cron task: {}", e))
        })?;

        Ok(json!({
            "task_id": task_id,
            "status": "cancelled",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{BackgroundTaskManager, CronScheduler};
    use crate::tools::ToolContext;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_schedule_cron_tool_with_manager() {
        let temp_dir = TempDir::new().unwrap();
        let cron_scheduler = Arc::new(Mutex::new(CronScheduler::new()));
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_cron_scheduler(cron_scheduler);

        let tool = ScheduleCronTool::with_manager(Arc::new(manager));
        let ctx = ToolContext::new();

        let args = json!({
            "task_name": "daily_cleanup",
            "prompt": "Clean up old temp files",
            "cron_expr": "0 0 2 * * *"
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["task_id"].as_str().unwrap().starts_with("cron_"));
        assert_eq!(result["task_name"], "daily_cleanup");
        assert_eq!(result["cron_expr"], "0 0 2 * * *");
    }

    #[tokio::test]
    async fn test_schedule_cron_tool_without_manager() {
        let tool = ScheduleCronTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "task_name": "test",
            "prompt": "test prompt",
            "cron_expr": "0 0 * * * *"
        });

        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_cron_tool() {
        let temp_dir = TempDir::new().unwrap();
        let cron_scheduler = Arc::new(Mutex::new(CronScheduler::new()));
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_cron_scheduler(cron_scheduler);

        // Schedule a task first
        let spec = crate::background::store::TaskSpec::new("list_test", "test prompt");
        let _ = manager.schedule_cron(spec, "0 0 * * * *").await.unwrap();

        let tool = ListCronTool::with_manager(Arc::new(manager));
        let ctx = ToolContext::new();

        let result = tool.execute(json!({}), ctx).await.unwrap();
        assert_eq!(result["count"], 1);
        assert!(result["tasks"].as_array().unwrap()[0]["task_id"]
            .as_str()
            .unwrap()
            .starts_with("cron_"));
    }

    #[tokio::test]
    async fn test_cancel_cron_tool() {
        let temp_dir = TempDir::new().unwrap();
        let cron_scheduler = Arc::new(Mutex::new(CronScheduler::new()));
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_cron_scheduler(cron_scheduler);

        let spec = crate::background::store::TaskSpec::new("cancel_test", "test prompt");
        let task_id = manager.schedule_cron(spec, "0 0 * * * *").await.unwrap();

        let tool = CancelCronTool::with_manager(Arc::new(manager));
        let ctx = ToolContext::new();

        let result = tool
            .execute(json!({"task_id": task_id}), ctx)
            .await
            .unwrap();
        assert_eq!(result["status"], "cancelled");
    }
}
