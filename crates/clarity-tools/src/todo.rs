//! Todo tool: manage a todo list for the Agent
//!
//! Todos are persisted to ~/.clarity/todos.json and survive across sessions.
//! The Agent can create, list, mark complete, and delete todo items.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;

use crate::helpers;
use crate::{Tool, ToolContext, ToolResult};
use clarity_contract::ToolError;

fn default_todos_path() -> ToolResult<PathBuf> {
    super::clarity_data_dir().map(|p| p.join("todos.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoItem {
    id: String,
    content: String,
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    priority: String, // low, normal, high
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<String>,
}

async fn load_todos(path: &PathBuf) -> ToolResult<Vec<TodoItem>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| ToolError::execution_failed(format!("Failed to read todos: {}", e)))?;
    serde_json::from_str(&contents)
        .map_err(|e| ToolError::execution_failed(format!("Failed to parse todos: {}", e)))
}

async fn save_todos(path: &PathBuf, todos: &[TodoItem]) -> ToolResult<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to create todos dir: {}", e))
        })?;
    }
    let json = serde_json::to_string_pretty(todos)
        .map_err(|e| ToolError::execution_failed(format!("Failed to serialize todos: {}", e)))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| ToolError::execution_failed(format!("Failed to write todos: {}", e)))
}

/// Tool for managing todo items
pub struct TodoTool {
    custom_path: Option<PathBuf>,
}

impl TodoTool {
    /// Create a new TodoTool instance
    pub fn new() -> Self {
        Self { custom_path: None }
    }

    /// Create with a custom path (useful for testing)
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            custom_path: Some(path),
        }
    }

    fn path(&self) -> ToolResult<PathBuf> {
        self.custom_path
            .clone()
            .map(Ok)
            .unwrap_or_else(default_todos_path)
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage a todo list. Supports adding, listing, completing, and deleting items. \
         Use this to track tasks, reminders, and follow-ups across conversation turns."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "complete", "delete", "clear_completed"],
                    "description": "Action to perform"
                },
                "content": {
                    "type": "string",
                    "description": "Content for the todo item (required for add)"
                },
                "id": {
                    "type": "string",
                    "description": "Todo item ID (required for complete/delete)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["low", "normal", "high"],
                    "description": "Priority for new items (default: normal)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let action = helpers::required_str(&args, "action")?;
        let path = self.path()?;

        match action {
            "add" => {
                let content = helpers::required_str(&args, "content")?;
                let priority = helpers::optional_str(&args, "priority").unwrap_or("normal");

                let mut todos = load_todos(&path).await?;
                let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
                let item = TodoItem {
                    id: id.clone(),
                    content: content.to_string(),
                    completed: false,
                    priority: priority.to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    completed_at: None,
                };
                todos.push(item);
                save_todos(&path, &todos).await?;

                info!("Todo added: {} ({})", content, id);
                Ok(json!({
                    "added": true,
                    "id": id,
                    "content": content,
                    "priority": priority,
                    "total": todos.len(),
                }))
            }
            "list" => {
                let todos = load_todos(&path).await?;
                let pending: Vec<&TodoItem> = todos.iter().filter(|t| !t.completed).collect();
                let completed: Vec<&TodoItem> = todos.iter().filter(|t| t.completed).collect();

                let items: Vec<Value> = todos
                    .iter()
                    .map(|t| {
                        json!({
                            "id": t.id,
                            "content": t.content,
                            "completed": t.completed,
                            "priority": t.priority,
                            "created_at": t.created_at,
                        })
                    })
                    .collect();

                info!(
                    "Todo list: {} pending, {} completed",
                    pending.len(),
                    completed.len()
                );
                Ok(json!({
                    "items": items,
                    "pending_count": pending.len(),
                    "completed_count": completed.len(),
                    "total_count": todos.len(),
                }))
            }
            "complete" => {
                let id = helpers::required_str(&args, "id")?;
                let mut todos = load_todos(&path).await?;

                let content = if let Some(item) = todos.iter_mut().find(|t| t.id == id) {
                    item.completed = true;
                    item.completed_at = Some(chrono::Utc::now().to_rfc3339());
                    item.content.clone()
                } else {
                    return Err(ToolError::execution_failed(format!(
                        "Todo item '{}' not found",
                        id
                    )));
                };

                save_todos(&path, &todos).await?;
                let pending_count = todos.iter().filter(|t| !t.completed).count();

                info!("Todo completed: {}", id);
                Ok(json!({
                    "completed": true,
                    "id": id,
                    "content": content,
                    "pending_count": pending_count,
                }))
            }
            "delete" => {
                let id = helpers::required_str(&args, "id")?;
                let mut todos = load_todos(&path).await?;
                let original_len = todos.len();
                todos.retain(|t| t.id != id);

                if todos.len() == original_len {
                    return Err(ToolError::execution_failed(format!(
                        "Todo item '{}' not found",
                        id
                    )));
                }

                save_todos(&path, &todos).await?;
                info!("Todo deleted: {}", id);
                Ok(json!({
                    "deleted": true,
                    "id": id,
                    "remaining": todos.len(),
                }))
            }
            "clear_completed" => {
                let mut todos = load_todos(&path).await?;
                let completed_count = todos.iter().filter(|t| t.completed).count();
                todos.retain(|t| !t.completed);
                save_todos(&path, &todos).await?;

                info!("Cleared {} completed todos", completed_count);
                Ok(json!({
                    "cleared": true,
                    "removed_count": completed_count,
                    "remaining": todos.len(),
                }))
            }
            _ => Err(ToolError::invalid_params(format!(
                "Unknown action: {}",
                action
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_todo_add_and_list() {
        let path =
            std::env::temp_dir().join(format!("clarity_todos_{}.json", uuid::Uuid::new_v4()));
        let tool = TodoTool::with_path(path.clone());
        let ctx = ToolContext::new();

        let args = json!({
            "action": "add",
            "content": "Test todo item",
            "priority": "high"
        });
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert!(result["added"].as_bool().unwrap());
        assert_eq!(result["priority"].as_str().unwrap(), "high");

        let args = json!({"action": "list"});
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["total_count"].as_u64().unwrap(), 1);
        assert_eq!(result["pending_count"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_todo_complete() {
        let path =
            std::env::temp_dir().join(format!("clarity_todos_{}.json", uuid::Uuid::new_v4()));
        let tool = TodoTool::with_path(path.clone());
        let ctx = ToolContext::new();

        let args = json!({"action": "add", "content": "Complete me"});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        let id = result["id"].as_str().unwrap().to_string();

        let args = json!({"action": "complete", "id": id});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert!(result["completed"].as_bool().unwrap());

        let args = json!({"action": "list"});
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["completed_count"].as_u64().unwrap(), 1);
        assert_eq!(result["pending_count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_todo_delete() {
        let path =
            std::env::temp_dir().join(format!("clarity_todos_{}.json", uuid::Uuid::new_v4()));
        let tool = TodoTool::with_path(path.clone());
        let ctx = ToolContext::new();

        let args = json!({"action": "add", "content": "Delete me"});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        let id = result["id"].as_str().unwrap().to_string();

        let args = json!({"action": "delete", "id": id});
        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["deleted"].as_bool().unwrap());
    }
}
