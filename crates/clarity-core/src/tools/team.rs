//! Team management tools: TeamCreate, TeamDelete, TeamList
//!
//! These tools allow an agent to dynamically create, delete, and list
//! collaborative agent team configurations that are persisted to disk.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

fn teams_dir() -> ToolResult<PathBuf> {
    super::clarity_data_dir().map(|p| p.join("teams"))
}

/// Serializable team member configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMemberConfig {
    pub name: String,
    pub description: String,
    pub agent_type: String,
}

/// Serializable team configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    pub name: String,
    pub goal: String,
    pub members: Vec<TeamMemberConfig>,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_max_concurrency() -> usize {
    4
}

fn default_timeout_secs() -> u64 {
    300
}

impl TeamConfig {
    /// Load a team config from disk
    pub async fn load(name: &str, root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = root.as_ref().join(format!("{}.json", name));
        let content = tokio::fs::read_to_string(&path).await?;
        let config: TeamConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save a team config to disk
    pub async fn save(&self, root: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = root.as_ref().join(format!("{}.json", self.name));
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }
}

/// Tool for creating a new agent team configuration
pub struct TeamCreateTool;

impl TeamCreateTool {
    /// Create a new TeamCreateTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TeamCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        "team_create"
    }

    fn description(&self) -> &str {
        "Create a new collaborative agent team configuration. \
         Members will work toward a shared goal using a shared mailbox. \
         Returns the team name and member count."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Unique name for the team (used as identifier)"
                },
                "goal": {
                    "type": "string",
                    "description": "High-level objective the team works toward"
                },
                "members": {
                    "type": "array",
                    "description": "List of team members",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Member name / role" },
                            "description": { "type": "string", "description": "What this member should do" },
                            "agent_type": { "type": "string", "description": "Agent type: explore, coder, plan, or default (default: default)" }
                        },
                        "required": ["name", "description"]
                    }
                },
                "max_concurrency": {
                    "type": "integer",
                    "description": "Max parallel members (default: 4)"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout per member in seconds (default: 300)"
                }
            },
            "required": ["team_name", "goal", "members"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let team_name = helpers::required_str(&args, "team_name")?;
        let goal = helpers::required_str(&args, "goal")?;

        let members_raw = args
            .get("members")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::invalid_params("missing required parameter: members"))?;

        let mut members = Vec::with_capacity(members_raw.len());
        for m in members_raw {
            let name = m
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::invalid_params("each member must have a name"))?
                .to_string();
            let description = m
                .get("description")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::invalid_params("each member must have a description"))?
                .to_string();
            let agent_type = m
                .get("agent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            members.push(TeamMemberConfig {
                name,
                description,
                agent_type,
            });
        }

        let max_concurrency = args
            .get("max_concurrency")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as usize;
        let timeout_secs = args
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let config = TeamConfig {
            name: team_name.to_string(),
            goal: goal.to_string(),
            members,
            max_concurrency,
            timeout_secs,
        };

        let dir = teams_dir()?;
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to create teams directory: {}", e))
        })?;

        config.save(&dir).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to save team config: {}", e))
        })?;

        info!(
            "Created team '{}' with {} members",
            team_name,
            config.members.len()
        );

        Ok(json!({
            "success": true,
            "team_name": team_name,
            "goal": goal,
            "member_count": config.members.len(),
            "max_concurrency": max_concurrency,
            "timeout_secs": timeout_secs,
            "message": format!("Team '{}' created with {} members", team_name, config.members.len())
        }))
    }
}

/// Tool for deleting a team configuration
pub struct TeamDeleteTool;

impl TeamDeleteTool {
    /// Create a new TeamDeleteTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TeamDeleteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TeamDeleteTool {
    fn name(&self) -> &str {
        "team_delete"
    }

    fn description(&self) -> &str {
        "Delete a team configuration by name. Returns success or error if not found."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to delete"
                }
            },
            "required": ["team_name"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let team_name = helpers::required_str(&args, "team_name")?;
        let dir = teams_dir()?;
        let path = dir.join(format!("{}.json", team_name));

        if !path.exists() {
            return Err(ToolError::execution_failed(format!(
                "Team '{}' not found",
                team_name
            )));
        }

        tokio::fs::remove_file(&path).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to delete team '{}': {}", team_name, e))
        })?;

        info!("Deleted team '{}'", team_name);

        Ok(json!({
            "success": true,
            "team_name": team_name,
            "message": format!("Team '{}' deleted", team_name)
        }))
    }
}

/// Tool for listing all team configurations
pub struct TeamListTool;

impl TeamListTool {
    /// Create a new TeamListTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TeamListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TeamListTool {
    fn name(&self) -> &str {
        "team_list"
    }

    fn description(&self) -> &str {
        "List all saved agent team configurations. \
         Returns name, goal, member count, and timeout for each team."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let dir = teams_dir()?;

        if !dir.exists() {
            return Ok(json!({ "teams": [] }));
        }

        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to read teams directory: {}", e))
        })?;

        let mut teams = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            let config: TeamConfig = match serde_json::from_str(&content) {
                Ok(c) => c,
                Err(_) => continue,
            };

            teams.push(json!({
                "name": config.name,
                "goal": config.goal,
                "member_count": config.members.len(),
                "max_concurrency": config.max_concurrency,
                "timeout_secs": config.timeout_secs,
            }));
        }

        Ok(json!({ "teams": teams }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_team_create_and_list() {
        let tool = TeamCreateTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "team_name": "test-refactor-squad",
            "goal": "Refactor the auth module",
            "members": [
                {"name": "Extract traits", "description": "Identify shared traits", "agent_type": "explore"},
                {"name": "Migrate code", "description": "Move impl blocks", "agent_type": "coder"}
            ],
            "max_concurrency": 2,
            "timeout_secs": 120
        });

        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["member_count"].as_u64().unwrap(), 2);
        assert_eq!(result["max_concurrency"].as_u64().unwrap(), 2);

        // List
        let list_tool = TeamListTool::new();
        let list_result = list_tool.execute(json!({}), ctx.clone()).await.unwrap();
        let teams = list_result["teams"].as_array().unwrap();
        assert!(!teams.is_empty());

        // Delete
        let del_tool = TeamDeleteTool::new();
        let del_result = del_tool
            .execute(json!({"team_name": "test-refactor-squad"}), ctx)
            .await
            .unwrap();
        assert!(del_result["success"].as_bool().unwrap());
    }
}
