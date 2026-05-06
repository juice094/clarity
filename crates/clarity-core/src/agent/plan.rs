//! Plan generation —— ask the LLM to produce a structured execution plan.
//!
//! This is the cognitive prerequisite for Plan Mode approval:
//! before any tool is executed, the Agent enumerates the steps it intends
//! to take and presents them to the user for confirmation.

use super::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmResponse, Message};
use crate::types::{Plan, PlanResult};

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
        self.maybe_snapshot_pre_turn().await;

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

        // Estimate and accumulate token usage for plan generation.
        let prompt_tokens = messages
            .iter()
            .map(|m| m.content.len())
            .sum::<usize>()
            .div_ceil(4) as u32;
        let completion_tokens = content.len().div_ceil(4) as u32;
        self.accumulate_usage(prompt_tokens, completion_tokens);

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

        self.maybe_snapshot_post_turn().await;
        self.finish_turn();
        Ok(plan)
    }

    /// Execute a previously-generated plan step-by-step.
    ///
    /// Each step is run through `Agent::execute_tool_call` so that the full
    /// safety pipeline (sensitive-file detection, risk evaluation, approval)
    /// is applied.  Step lifecycle events are emitted over the wire.
    /// Errors on individual steps are captured in `PlanResult` rather than
    /// aborting the whole plan, so the caller can see partial progress.
    pub async fn execute_plan(&self, plan: &Plan) -> Result<Vec<PlanResult>, AgentError> {
        let cancel_token = self.begin_turn()?;
        self.maybe_snapshot_pre_turn().await;

        self.send_wire_message(clarity_wire::WireMessage::TurnBegin {
            user_input: format!("Execute plan: {}", plan.title),
        });

        let mut results = Vec::with_capacity(plan.steps.len());
        for step in &plan.steps {
            if cancel_token.is_cancelled() {
                tracing::info!("Plan execution cancelled at step {}", step.id);
                break;
            }

            self.send_wire_message(clarity_wire::WireMessage::PlanStepBegin {
                step_id: step.id.clone(),
                tool_name: step.tool_name.clone(),
            });

            let tool_call = crate::types::ToolCall {
                id: format!("plan-step-{}", step.id),
                call_type: "function".to_string(),
                function: crate::types::FunctionCall {
                    name: step.tool_name.clone(),
                    arguments: serde_json::to_string(&step.tool_params).unwrap_or_default(),
                },
            };

            let (success, output) = match self.execute_tool_call(&tool_call).await {
                Ok(val) => (true, val.to_string()),
                Err(e) => (false, format!("Error: {}", e)),
            };

            self.send_wire_message(clarity_wire::WireMessage::PlanStepEnd {
                step_id: step.id.clone(),
                success,
            });

            results.push(PlanResult {
                step_id: step.id.clone(),
                success,
                output,
            });
        }

        self.send_wire_message(clarity_wire::WireMessage::TurnEnd);
        self.maybe_snapshot_post_turn().await;
        self.finish_turn();
        Ok(results)
    }

    /// Run the agent in Plan approval mode.
    pub async fn execute_plan_mode(&self, query: &str) -> Result<String, AgentError> {
        let plan = self.plan(query).await?;
        self.send_wire_message(clarity_wire::WireMessage::TurnBegin {
            user_input: query.to_string(),
        });
        if !plan.is_empty() {
            self.send_wire_message(clarity_wire::WireMessage::ContentPart {
                text: format!("📋 Executing plan: {}\n{}", plan.title, plan.to_markdown()),
            });
        }
        let results = self.execute_plan(&plan).await?;
        let final_response = crate::agent::run::format_plan_results(&results);
        self.send_wire_message(clarity_wire::WireMessage::TurnEnd);
        Ok(final_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentConfig;
    use crate::llm::api::LlmProvider;
    use crate::types::PlanStep;
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
            fn set_prompt_cache_key(&self, _key: &str) {}
        }

        let registry = crate::registry::mock_registry_with_tools(vec![]);
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
            fn set_prompt_cache_key(&self, _key: &str) {}
        }

        let registry = crate::registry::mock_registry_with_tools(vec![]);
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
