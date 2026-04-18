//! Think tool: allows the Agent to pause and organize its thoughts
//!
//! This is a no-op tool that gives the Agent a structured way to think out loud
//! before taking action. It captures the Agent's reasoning process without
//! modifying any state, accessing files, or executing commands.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

/// Tool for structured thinking
///
/// When the LLM sees "think" in its available tools, it naturally uses it
/// to organize complex reasoning, reducing impulsive mistakes.
pub struct ThinkTool;

impl ThinkTool {
    /// Create a new ThinkTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThinkTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ThinkTool {
    fn name(&self) -> &str {
        "think"
    }

    fn description(&self) -> &str {
        "Pause to organize your thoughts before taking action. \
         Use this tool to reflect on the current state, plan your approach, \
         or reason through a complex problem. This tool does not modify any state."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "thought": {
                    "type": "string",
                    "description": "Your current reasoning, reflection, or plan"
                },
                "next_step": {
                    "type": "string",
                    "description": "What you plan to do next (optional)"
                }
            },
            "required": ["thought"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let thought = helpers::required_str(&args, "thought")?;
        let next_step = helpers::optional_str(&args, "next_step");

        info!("Think tool invoked");
        info!("Thought: {}", thought);
        if let Some(step) = next_step {
            info!("Next step: {}", step);
        }

        let summary: String = thought.chars().take(200).collect();

        Ok(json!({
            "acknowledged": true,
            "thought_length": thought.len(),
            "summary": summary
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_think_tool_basic() {
        let tool = ThinkTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "thought": "I need to read the file first, then analyze its contents."
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["acknowledged"].as_bool().unwrap());
        assert_eq!(result["thought_length"].as_u64().unwrap(), 57);
        assert_eq!(
            result["summary"].as_str().unwrap(),
            "I need to read the file first, then analyze its contents."
        );
    }

    #[tokio::test]
    async fn test_think_tool_with_next_step() {
        let tool = ThinkTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "thought": "This is a complex problem that requires careful analysis.",
            "next_step": "Break the problem into smaller sub-problems."
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["acknowledged"].as_bool().unwrap());
        assert_eq!(result["thought_length"].as_u64().unwrap(), 57);
    }

    #[tokio::test]
    async fn test_think_tool_long_thought_summary_truncated() {
        let tool = ThinkTool::new();
        let ctx = ToolContext::new();

        let long_thought = "a".repeat(300);
        let args = json!({"thought": long_thought});

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["acknowledged"].as_bool().unwrap());
        assert_eq!(result["thought_length"].as_u64().unwrap(), 300);
        assert_eq!(result["summary"].as_str().unwrap().len(), 200);
    }

    #[tokio::test]
    async fn test_think_tool_missing_required_param() {
        let tool = ThinkTool::new();
        let ctx = ToolContext::new();

        let args = json!({"next_step": "do something"});

        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing required parameter: thought"));
    }
}
