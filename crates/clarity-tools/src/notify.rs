//! Notify tool: sends a notification that can be displayed by the UI
//!
//! Notifications are persisted to ~/.clarity/notifications/ so that
//! system tray apps (claw) or other UIs can pick them up and display
//! them via the OS notification center.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;

use crate::helpers;
use crate::{Tool, ToolContext, ToolResult};
use clarity_contract::ToolError;

fn notifications_dir() -> ToolResult<PathBuf> {
    super::clarity_data_dir().map(|p| p.join("notifications"))
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

/// Tool for pushing notifications to external channels (webhook, file, or desktop)
///
/// Extends `NotifyTool` with webhook support. If no `webhook_url` is provided,
/// behaves identically to `NotifyTool` (writes to the filesystem notification dir).
pub struct PushNotificationTool;

impl PushNotificationTool {
    /// Create a new PushNotificationTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for PushNotificationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PushNotificationTool {
    fn name(&self) -> &str {
        "push_notify"
    }

    fn description(&self) -> &str {
        "Send a push notification to one or more channels. \
         Supports file-based persistence (default) and webhook delivery. \
         Use this for alerting external systems or when a notification \
         needs to reach multiple destinations."
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
                },
                "channels": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["file", "webhook"] },
                    "description": "Delivery channels (default: [\"file\"])"
                },
                "webhook_url": {
                    "type": "string",
                    "description": "Webhook URL for HTTP POST delivery (required if 'webhook' is in channels)"
                }
            },
            "required": ["title", "body"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let title = helpers::required_str(&args, "title")?;
        let body = helpers::required_str(&args, "body")?;
        let urgency = helpers::optional_str(&args, "urgency").unwrap_or("normal");

        let channels = args
            .get("channels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["file".to_string()]);

        let mut results = Vec::new();
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        for channel in &channels {
            match channel.as_str() {
                "file" => {
                    let dir = notifications_dir()?;
                    tokio::fs::create_dir_all(&dir).await.map_err(|e| {
                        ToolError::execution_failed(format!(
                            "Failed to create notifications dir: {}",
                            e
                        ))
                    })?;

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
                            ToolError::execution_failed(format!(
                                "Failed to write notification: {}",
                                e
                            ))
                        })?;

                    results.push(json!({
                        "channel": "file",
                        "sent": true,
                        "id": id
                    }));
                }
                "webhook" => {
                    let webhook_url =
                        helpers::optional_str(&args, "webhook_url").ok_or_else(|| {
                            ToolError::invalid_params(
                                "webhook_url is required when 'webhook' is in channels",
                            )
                        })?;

                    let payload = json!({
                        "id": id,
                        "title": title,
                        "body": body,
                        "urgency": urgency,
                        "timestamp": timestamp,
                    });

                    let client = reqwest::Client::new();
                    let resp = client.post(webhook_url).json(&payload).send().await;

                    match resp {
                        Ok(r) => {
                            let status = r.status();
                            let ok = status.is_success();
                            results.push(json!({
                                "channel": "webhook",
                                "sent": ok,
                                "status": status.as_u16(),
                                "url": webhook_url
                            }));
                        }
                        Err(e) => {
                            results.push(json!({
                                "channel": "webhook",
                                "sent": false,
                                "error": e.to_string(),
                                "url": webhook_url
                            }));
                        }
                    }
                }
                _ => {
                    results.push(json!({
                        "channel": channel,
                        "sent": false,
                        "error": "Unknown channel"
                    }));
                }
            }
        }

        let all_sent = results.iter().all(|r| r["sent"].as_bool().unwrap_or(false));

        Ok(json!({
            "sent": all_sent,
            "id": id,
            "title": title,
            "urgency": urgency,
            "timestamp": timestamp,
            "channels": results,
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

    #[tokio::test]
    async fn test_push_notify_file_channel() {
        let tool = PushNotificationTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "title": "Build Complete",
            "body": "All tests passed.",
            "channels": ["file"]
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["sent"].as_bool().unwrap());
        assert_eq!(result["title"].as_str().unwrap(), "Build Complete");

        let channels = result["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["channel"].as_str().unwrap(), "file");
        assert!(channels[0]["sent"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_push_notify_webhook_missing_url() {
        let tool = PushNotificationTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "title": "Alert",
            "body": "Something happened",
            "channels": ["webhook"]
        });

        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
    }
}
