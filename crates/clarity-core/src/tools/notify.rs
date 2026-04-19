//! Notify tool: sends a notification that can be displayed by the UI
//!
//! Notifications are persisted to ~/.clarity/notifications/ so that
//! system tray apps (claw) or other UIs can pick them up and display
//! them via the OS notification center.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

fn notifications_dir() -> ToolResult<PathBuf> {
    dirs::home_dir()
        .map(|p| p.join(".clarity").join("notifications"))
        .ok_or_else(|| ToolError::execution_failed("Could not determine home directory".to_string()))
}

/// Tool for sending notifications
///
/// Use this to alert the user about important events, completions,
/// or when human attention is required.
pub struct NotifyTool;

impl NotifyTool {
    /// Create a new NotifyTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for NotifyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for NotifyTool {
    fn name(&self) -> &str {
        "notify"
    }

    fn description(&self) -> &str {
        "Send a notification to the user. Use this for important events, \
         task completions, errors that need attention, or when a long-running \
         operation finishes. The notification will be displayed by the system tray \
         or other UI components."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short title of the notification"
                },
                "body": {
                    "type": "string",
                    "description": "Detailed message body"
                },
                "urgency": {
                    "type": "string",
                    "enum": ["low", "normal", "high"],
                    "description": "Urgency level (default: normal)"
                }
            },
            "required": ["title", "body"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let title = helpers::required_str(&args, "title")?;
        let body = helpers::required_str(&args, "body")?;
        let urgency = helpers::optional_str(&args, "urgency").unwrap_or("normal");

        info!("Notify tool invoked: {} [{}]", title, urgency);

        let dir = notifications_dir()?;
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to create notifications dir: {}", e))
        })?;

        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let file_path = dir.join(format!("{}.json", id));

        let notification = json!({
            "id": id,
            "title": title,
            "body": body,
            "urgency": urgency,
            "timestamp": timestamp,
            "read": false,
        });

        tokio::fs::write(&file_path, notification.to_string())
            .await
            .map_err(|e| {
                ToolError::execution_failed(format!("Failed to write notification: {}", e))
            })?;

        Ok(json!({
            "sent": true,
            "id": id,
            "title": title,
            "urgency": urgency,
            "timestamp": timestamp,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notify_basic() {
        let tool = NotifyTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "title": "Task Complete",
            "body": "The build finished successfully."
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["sent"].as_bool().unwrap());
        assert_eq!(result["title"].as_str().unwrap(), "Task Complete");
        assert_eq!(result["urgency"].as_str().unwrap(), "normal");
    }

    #[tokio::test]
    async fn test_notify_with_urgency() {
        let tool = NotifyTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "title": "Error",
            "body": "Build failed!",
            "urgency": "high"
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["urgency"].as_str().unwrap(), "high");
    }

    #[tokio::test]
    async fn test_notify_missing_title() {
        let tool = NotifyTool::new();
        let ctx = ToolContext::new();

        let args = json!({"body": "missing title"});
        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
    }
}
