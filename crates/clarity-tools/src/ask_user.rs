//! AskUser tool: allows the Agent to ask the user a question and wait for a response
//!
//! This tool sends a question to the UI via the Wire channel. The UI displays it
//! and the user replies through the normal input channel, which feeds back into
//! the conversation as a new user turn.

use async_trait::async_trait;
use serde_json::{Value, json};
use tracing::info;

use crate::helpers;
use crate::{Tool, ToolContext, ToolResult};

/// Tool for asking the user a question
///
/// When the LLM needs clarification, confirmation, or additional information,
/// it uses this tool to pause and request input from the user.
pub struct AskUserTool;

impl AskUserTool {
    /// Create a new AskUserTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for AskUserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "Ask the user a question when you need clarification, confirmation, \
         or additional information to proceed. The conversation will pause \
         until the user responds. Use this tool instead of guessing or making \
         assumptions about ambiguous requirements."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user. Be specific and concise."
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional predefined options for the user to choose from"
                },
                "context": {
                    "type": "string",
                    "description": "Optional context explaining why you're asking this question"
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let question = helpers::required_str(&args, "question")?;
        let options: Option<Vec<String>> =
            args.get("options").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });
        let context = helpers::optional_str(&args, "context");

        info!("AskUser tool invoked: {}", question);

        let options_msg = options
            .as_ref()
            .map(|opts| format!("\nOptions: {}", opts.join(", ")));

        let full_question = if let Some(ctx) = context {
            format!("{}\n{}", ctx, question)
        } else {
            question.to_string()
        };

        let display = if let Some(ref opts) = options_msg {
            format!("{}{}", full_question, opts)
        } else {
            full_question.clone()
        };

        // Return a result that signals the agent is waiting for user input.
        // The UI layer should display this prominently.
        Ok(json!({
            "asked": true,
            "question": full_question,
            "display": display,
            "options": options,
            "status": "waiting_for_user",
            "hint": "The agent is waiting for your response. Please reply with your answer."
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ask_user_basic() {
        let tool = AskUserTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "question": "Which file should I modify?"
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["asked"].as_bool().unwrap());
        assert_eq!(
            result["question"].as_str().unwrap(),
            "Which file should I modify?"
        );
        assert_eq!(result["status"].as_str().unwrap(), "waiting_for_user");
    }

    #[tokio::test]
    async fn test_ask_user_with_options() {
        let tool = AskUserTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "question": "Choose a color:",
            "options": ["red", "green", "blue"]
        });

        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["asked"].as_bool().unwrap());
        let options = result["options"].as_array().unwrap();
        assert_eq!(options.len(), 3);
    }

    #[tokio::test]
    async fn test_ask_user_with_context() {
        let tool = AskUserTool::new();
        let ctx = ToolContext::new();

        let args = json!({
            "question": "Should I proceed?",
            "context": "I'm about to delete the old backup files."
        });

        let result = tool.execute(args, ctx).await.unwrap();
        let question = result["question"].as_str().unwrap();
        assert!(question.contains("delete the old backup files"));
        assert!(question.contains("Should I proceed?"));
    }

    #[tokio::test]
    async fn test_ask_user_missing_question() {
        let tool = AskUserTool::new();
        let ctx = ToolContext::new();

        let args = json!({"context": "missing question"});
        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
    }
}
