//! Plan tool: create and manage execution plans
//!
//! Plans are persisted to ~/.clarity/plans/<plan_id>.json.
//! A plan consists of ordered steps that the Agent can execute sequentially.
//! This helps break complex tasks into manageable chunks and track progress.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

fn default_plans_dir() -> ToolResult<PathBuf> {
    dirs::home_dir()
        .map(|p| p.join(".clarity").join("plans"))
        .ok_or_else(|| {
            ToolError::execution_failed("Could not determine home directory".to_string())
        })
}

fn plan_path(dir: &std::path::Path, plan_id: &str) -> PathBuf {
    dir.join(format!("{}.json", plan_id))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlanStep {
    id: String,
    description: String,
    #[serde(default)]
    status: String, // pending, in_progress, completed, skipped, failed
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    steps: Vec<PlanStep>,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

async fn load_plan(dir: &std::path::Path, plan_id: &str) -> ToolResult<Plan> {
    let path = plan_path(dir, plan_id);
    let contents = tokio::fs::read_to_string(&path).await.map_err(|e| {
        ToolError::execution_failed(format!("Failed to read plan '{}': {}", plan_id, e))
    })?;
    serde_json::from_str(&contents).map_err(|e| {
        ToolError::execution_failed(format!("Failed to parse plan '{}': {}", plan_id, e))
    })
}

async fn save_plan(dir: &PathBuf, plan: &Plan) -> ToolResult<()> {
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ToolError::execution_failed(format!("Failed to create plans dir: {}", e)))?;
    let path = plan_path(dir, &plan.id);
    let json = serde_json::to_string_pretty(plan)
        .map_err(|e| ToolError::execution_failed(format!("Failed to serialize plan: {}", e)))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| ToolError::execution_failed(format!("Failed to write plan: {}", e)))
}

/// Tool for managing execution plans
pub struct PlanTool {
    custom_dir: Option<PathBuf>,
}

impl PlanTool {
    /// Create a new PlanTool instance
    pub fn new() -> Self {
        Self { custom_dir: None }
    }

    /// Create with a custom directory (useful for testing)
    pub fn with_dir(dir: PathBuf) -> Self {
        Self {
            custom_dir: Some(dir),
        }
    }

    fn dir(&self) -> ToolResult<PathBuf> {
        self.custom_dir
            .clone()
            .map(Ok)
            .unwrap_or_else(default_plans_dir)
    }
}

impl Default for PlanTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PlanTool {
    fn name(&self) -> &str {
        "plan"
    }

    fn description(&self) -> &str {
        "Create and manage execution plans. A plan is an ordered list of steps \
         that breaks a complex task into manageable chunks. Use this before starting \
         multi-step work to organize your approach and track progress."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "get", "update_step", "list", "delete"],
                    "description": "Action to perform"
                },
                "plan_id": {
                    "type": "string",
                    "description": "Plan ID (required for get, update_step, delete)"
                },
                "title": {
                    "type": "string",
                    "description": "Plan title (required for create)"
                },
                "description": {
                    "type": "string",
                    "description": "Optional plan description"
                },
                "steps": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of step descriptions (required for create)"
                },
                "step_id": {
                    "type": "string",
                    "description": "Step ID to update (required for update_step). Examples: 'step_1', 'step_2'. Numeric strings like '1' are also accepted and mapped to 'step_1'."
                },
                "step_status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "skipped", "failed"],
                    "description": "New status for the step (required for update_step)"
                },
                "step_result": {
                    "type": "string",
                    "description": "Optional result/note for the step update"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let action = helpers::required_str(&args, "action")?;
        let dir = self.dir()?;

        match action {
            "create" => {
                let title = helpers::required_str(&args, "title")?;
                let description = helpers::optional_str(&args, "description").unwrap_or("");
                let step_descs = helpers::required_string_array(&args, "steps")?;

                let plan_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
                let steps: Vec<PlanStep> = step_descs
                    .into_iter()
                    .enumerate()
                    .map(|(i, desc)| PlanStep {
                        id: format!("step_{}", i + 1),
                        description: desc,
                        status: "pending".to_string(),
                        result: None,
                    })
                    .collect();

                let plan = Plan {
                    id: plan_id.clone(),
                    title: title.to_string(),
                    description: description.to_string(),
                    steps,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: None,
                };
                save_plan(&dir, &plan).await?;

                info!("Plan created: {} ({})", title, plan_id);
                Ok(json!({
                    "created": true,
                    "plan_id": plan_id,
                    "title": title,
                    "step_count": plan.steps.len(),
                }))
            }
            "get" => {
                let plan_id = helpers::required_str(&args, "plan_id")?;
                let plan = load_plan(&dir, plan_id).await?;

                let steps_json: Vec<Value> = plan
                    .steps
                    .iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "description": s.description,
                            "status": s.status,
                            "result": s.result,
                        })
                    })
                    .collect();

                let completed = plan
                    .steps
                    .iter()
                    .filter(|s| s.status == "completed")
                    .count();
                let pending = plan.steps.iter().filter(|s| s.status == "pending").count();

                Ok(json!({
                    "plan_id": plan.id,
                    "title": plan.title,
                    "description": plan.description,
                    "steps": steps_json,
                    "progress": format!("{}/{}", completed, plan.steps.len()),
                    "completed": completed,
                    "pending": pending,
                    "created_at": plan.created_at,
                }))
            }
            "update_step" => {
                let plan_id = helpers::required_str(&args, "plan_id")?;
                let step_id = helpers::required_str(&args, "step_id")?;
                let status = helpers::required_str(&args, "step_status")?;
                let result = helpers::optional_str(&args, "step_result");

                let mut plan = load_plan(&dir, plan_id).await?;
                // Flexible step-id matching: accept both "1" and "step_1" forms.
                let normalized = if step_id.parse::<usize>().is_ok() {
                    format!("step_{}", step_id)
                } else {
                    step_id.to_string()
                };
                if let Some(step) = plan.steps.iter_mut().find(|s| s.id == step_id || s.id == normalized) {
                    step.status = status.to_string();
                    if let Some(r) = result {
                        step.result = Some(r.to_string());
                    }
                    plan.updated_at = Some(chrono::Utc::now().to_rfc3339());
                    save_plan(&dir, &plan).await?;

                    info!("Plan step updated: {} -> {}", step_id, status);
                    Ok(json!({
                        "updated": true,
                        "plan_id": plan_id,
                        "step_id": step_id,
                        "status": status,
                    }))
                } else {
                    Err(ToolError::execution_failed(format!(
                        "Step '{}' not found in plan '{}'",
                        step_id, plan_id
                    )))
                }
            }
            "list" => {
                let mut plans = Vec::new();

                if dir.exists() {
                    let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| {
                        ToolError::execution_failed(format!("Failed to read plans dir: {}", e))
                    })?;
                    while let Some(entry) = entries.next_entry().await.map_err(|e| {
                        ToolError::execution_failed(format!("Failed to read entry: {}", e))
                    })? {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("json") {
                            if let Ok(contents) = tokio::fs::read_to_string(&path).await {
                                if let Ok(plan) = serde_json::from_str::<Plan>(&contents) {
                                    let completed = plan
                                        .steps
                                        .iter()
                                        .filter(|s| s.status == "completed")
                                        .count();
                                    plans.push(json!({
                                        "plan_id": plan.id,
                                        "title": plan.title,
                                        "step_count": plan.steps.len(),
                                        "completed": completed,
                                        "created_at": plan.created_at,
                                    }));
                                }
                            }
                        }
                    }
                }

                Ok(json!({
                    "plans": plans,
                    "total": plans.len(),
                }))
            }
            "delete" => {
                let plan_id = helpers::required_str(&args, "plan_id")?;
                let path = plan_path(&dir, plan_id);
                tokio::fs::remove_file(&path).await.map_err(|e| {
                    ToolError::execution_failed(format!(
                        "Failed to delete plan '{}': {}",
                        plan_id, e
                    ))
                })?;

                info!("Plan deleted: {}", plan_id);
                Ok(json!({
                    "deleted": true,
                    "plan_id": plan_id,
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
    async fn test_plan_create_and_get() {
        let dir = std::env::temp_dir().join(format!("clarity_plans_{}", uuid::Uuid::new_v4()));
        let tool = PlanTool::with_dir(dir.clone());
        let ctx = ToolContext::new();

        let args = json!({
            "action": "create",
            "title": "Refactor auth",
            "description": "Improve authentication flow",
            "steps": ["Analyze current code", "Extract auth module", "Add tests"]
        });
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert!(result["created"].as_bool().unwrap());
        assert_eq!(result["step_count"].as_u64().unwrap(), 3);
        let plan_id = result["plan_id"].as_str().unwrap().to_string();

        let args = json!({
            "action": "get",
            "plan_id": plan_id
        });
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["title"].as_str().unwrap(), "Refactor auth");
        assert_eq!(result["progress"].as_str().unwrap(), "0/3");
    }

    #[tokio::test]
    async fn test_plan_update_step() {
        let dir = std::env::temp_dir().join(format!("clarity_plans_{}", uuid::Uuid::new_v4()));
        let tool = PlanTool::with_dir(dir.clone());
        let ctx = ToolContext::new();

        let args = json!({
            "action": "create",
            "title": "Test plan",
            "steps": ["Step 1", "Step 2"]
        });
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        let plan_id = result["plan_id"].as_str().unwrap().to_string();

        let args = json!({
            "action": "update_step",
            "plan_id": plan_id,
            "step_id": "step_1",
            "step_status": "completed",
            "step_result": "Done successfully"
        });
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert!(result["updated"].as_bool().unwrap());

        let args = json!({"action": "get", "plan_id": plan_id});
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["progress"].as_str().unwrap(), "1/2");
    }

    #[tokio::test]
    async fn test_plan_list_and_delete() {
        let dir = std::env::temp_dir().join(format!("clarity_plans_{}", uuid::Uuid::new_v4()));
        let tool = PlanTool::with_dir(dir.clone());
        let ctx = ToolContext::new();

        let args = json!({
            "action": "create",
            "title": "Plan A",
            "steps": ["Do A"]
        });
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        let plan_id = result["plan_id"].as_str().unwrap().to_string();

        let args = json!({"action": "list"});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 1);

        let args = json!({"action": "delete", "plan_id": plan_id});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert!(result["deleted"].as_bool().unwrap());

        let args = json!({"action": "list"});
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }
}
