//! Plan generation —— ask the LLM to produce a structured execution plan.
//!
//! This is the cognitive prerequisite for Plan Mode approval:
//! before any tool is executed, the Agent enumerates the steps it intends
//! to take and presents them to the user for confirmation.

use super::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmResponse, Message};
use serde::{Deserialize, Serialize};

/// A single step inside an execution plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanStep {
    /// Human-readable identifier (e.g. "1", "2a")
    pub id: String,
    /// What this step does in plain language.
    pub description: String,
    /// The tool to invoke (must exist in the registry).
    pub tool_name: String,
    /// JSON payload for the tool call.
    #[serde(default)]
    pub tool_params: serde_json::Value,
}

/// A structured execution plan produced by `Agent::plan()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plan {
    /// Short title summarising the plan.
    pub title: String,
    /// Ordered steps to execute.
    pub steps: Vec<PlanStep>,
}

/// Result of executing a single plan step.
#[derive(Debug, Clone)]
pub struct PlanResult {
    /// The step that was executed.
    pub step_id: String,
    /// Whether the tool call succeeded.
    pub success: bool,
    /// Stringified tool output (or error message).
    pub output: String,
}

impl Plan {
    /// Render the plan as human-readable Markdown.
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![format!("## {}\n", self.title)];
        for step in &self.steps {
            lines.push(format!(
                "**{}.** {}  
`{}` {}",
                step.id,
                step.description,
                step.tool_name,
                serde_json::to_string(&step.tool_params).unwrap_or_default()
            ));
        }
        if self.steps.is_empty() {
            lines.push("_No steps required._".to_string());
        }
        lines.join("\n")
    }

    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Agent {
    /// Ask the LLM to generate a structured execution plan for `query`.
    ///
    /// The returned `Plan` is *advisory* — the caller (usually the TUI)
    /// decides whether to execute it, edit it, or discard it.
    ///
    /// # Errors
    ///
    /// - `AgentError::Unconfigured` if no LLM is set.
    /// - `AgentError::Llm` if the LLM call fails or returns unparseable JSON.
    pub async fn plan(&self, query: impl AsRef<str>) -> Result<Plan, AgentError> {
        let query = query.as_ref();
        let _cancel_token = self.begin_turn()?;

        let llm = self.llm().ok_or(AgentError::Unconfigured)?;

        // Build a compact list of available tools for the planning prompt.
        let tool_names = self.registry.list_tools().unwrap_or_default().join(", ");

        let system_prompt = format!(
            r#"You are a planning assistant.
Given a user request, produce a concise step-by-step execution plan using the available tools.

Available tools: {tool_names}

Respond **only** with a JSON object in this exact shape (no markdown fences, no commentary):
{{
  "title": "Brief plan title",
  "steps": [
    {{
      "id": "1",
      "description": "What this step does",
      "tool_name": "tool_name",
      "tool_params": {{}}
    }}
  ]
}}

Rules:
- Each step must use exactly one tool.
- `tool_name` must be one of the available tools.
- `tool_params` must be a valid JSON object matching that tool's schema.
- If the request needs no tools, return an empty `steps` array."#
        );

        let messages = vec![Message::system(system_prompt), Message::user(query)];

        // We pass an empty tools object because the planning prompt itself
        // enumerates the tools; we do not want the LLM to emit function_call
        // metadata here — we want raw JSON in the content field.
        let empty_tools = serde_json::json!({ "functions": [] });
        let LlmResponse {
            content,
            tool_calls,
            is_complete: _,
        } = llm.complete(&messages, &empty_tools).await?;

        // Defensive: if the LLM decided to call a function instead of
        // returning JSON content, treat that as an error.
        if !tool_calls.is_empty() {
            return Err(AgentError::Llm(
                "LLM returned tool calls instead of a plan JSON".to_string(),
            ));
        }

        // Strip optional markdown fences that some models wrap JSON in.
        let raw = content.trim();
        let json_str = if raw.starts_with("```json") {
            raw.trim_start_matches("```json")
                .trim_end_matches("```")
                .trim()
        } else if raw.starts_with("```") {
            raw.trim_start_matches("```").trim_end_matches("```").trim()
        } else {
            raw
        };

        let plan: Plan = serde_json::from_str(json_str).map_err(|e| {
            AgentError::Llm(format!(
                "Failed to parse plan JSON: {}. Raw response:\n{}",
                e, content
            ))
        })?;

        self.finish_turn();
        Ok(plan)
    }

    /// Execute a previously-generated plan step-by-step.
    ///
    /// Each step is run through `ToolRegistry::execute` with the same
    /// `ToolContext` that the Agent uses during a normal turn.
    /// Errors on individual steps are captured in `PlanResult` rather than
    /// aborting the whole plan, so the caller can see partial progress.
    pub async fn execute_plan(&self, plan: &Plan) -> Result<Vec<PlanResult>, AgentError> {
        let _cancel_token = self.begin_turn()?;

        let mode = self.inner.read().unwrap().approval_mode;
        let ctx = crate::tools::ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs)
            .with_approval_mode(mode);

        let mut results = Vec::with_capacity(plan.steps.len());
        for step in &plan.steps {
            let output = match self
                .registry
                .execute(&step.tool_name, step.tool_params.clone(), ctx.clone())
                .await
            {
                Ok(val) => val.to_string(),
                Err(e) => format!("Error: {}", e),
            };
            results.push(PlanResult {
                step_id: step.id.clone(),
                success: !output.starts_with("Error:"),
                output,
            });
        }

        self.finish_turn();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentConfig;
    use crate::llm::api::LlmProvider;
    use crate::registry::ToolRegistry;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_plan_parses_valid_json() {
        // A mock LLM that returns valid plan JSON.
        struct PlanMock;
        #[async_trait::async_trait]
        impl LlmProvider for PlanMock {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &serde_json::Value,
            ) -> Result<LlmResponse, AgentError> {
                Ok(LlmResponse {
                    content: r#"{"title":"Test Plan","steps":[{"id":"1","description":"Do thing","tool_name":"bash","tool_params":{"command":"echo hi"}}]}"#.to_string(),
                    tool_calls: vec![],
                    is_complete: true,
                })
            }
            fn stream(
                &self,
                _messages: &[Message],
                _tools: &serde_json::Value,
            ) -> Result<
                tokio::sync::mpsc::Receiver<Result<crate::llm::api::StreamDelta, AgentError>>,
                AgentError,
            > {
                let (_, rx) = tokio::sync::mpsc::channel(1);
                Ok(rx)
            }
            fn set_prompt_cache_key(&mut self, _key: &str) {}
        }

        let registry = ToolRegistry::with_builtin_tools();
        let agent = Agent::with_config(registry, AgentConfig::new()).with_llm(Arc::new(PlanMock));

        let plan = agent.plan("do something").await.unwrap();
        assert_eq!(plan.title, "Test Plan");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].tool_name, "bash");
    }

    #[tokio::test]
    async fn test_plan_rejects_tool_calls() {
        struct BadMock;
        #[async_trait::async_trait]
        impl LlmProvider for BadMock {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &serde_json::Value,
            ) -> Result<LlmResponse, AgentError> {
                Ok(LlmResponse {
                    content: "ignored".to_string(),
                    tool_calls: vec![crate::types::ToolCall {
                        id: "call_1".to_string(),
                        call_type: "function".to_string(),
                        function: crate::types::FunctionCall {
                            name: "bash".to_string(),
                            arguments: "{}".to_string(),
                        },
                    }],
                    is_complete: true,
                })
            }
            fn stream(
                &self,
                _messages: &[Message],
                _tools: &serde_json::Value,
            ) -> Result<
                tokio::sync::mpsc::Receiver<Result<crate::llm::api::StreamDelta, AgentError>>,
                AgentError,
            > {
                let (_, rx) = tokio::sync::mpsc::channel(1);
                Ok(rx)
            }
            fn set_prompt_cache_key(&mut self, _key: &str) {}
        }

        let registry = ToolRegistry::with_builtin_tools();
        let agent = Agent::with_config(registry, AgentConfig::new()).with_llm(Arc::new(BadMock));

        let err = agent.plan("test").await.unwrap_err();
        assert!(matches!(err, AgentError::Llm(_)));
    }

    #[test]
    fn test_plan_markdown_rendering() {
        let plan = Plan {
            title: "Demo".to_string(),
            steps: vec![PlanStep {
                id: "1".to_string(),
                description: "Say hello".to_string(),
                tool_name: "bash".to_string(),
                tool_params: serde_json::json!({"command": "echo hello"}),
            }],
        };
        let md = plan.to_markdown();
        assert!(md.contains("Demo"));
        assert!(md.contains("bash"));
        assert!(md.contains("echo hello"));
    }
}
