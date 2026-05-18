//! Plan generation —— ask the LLM to produce a structured execution plan.
//!
//! This is the cognitive prerequisite for Plan Mode approval:
//! before any tool is executed, the Agent enumerates the steps it intends
//! to take and presents them to the user for confirmation.

use super::Agent;
use crate::error::AgentError;
use crate::types::{Plan, PlanExecutionState, PlanResult, PlanStepExecutionStatus};
use clarity_llm::api::{LlmResponse, Message};

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
            turn_id: String::new(),
            user_input: format!("Execute plan: {}", plan.title),
        });

        let mut controller = PlanExecutionController::new(plan.clone());
        {
            let mut guard = self.plan_controller.lock().await;
            *guard = Some(controller.clone());
        }

        while controller.has_next() {
            if cancel_token.is_cancelled() {
                tracing::info!("Plan execution cancelled");
                break;
            }
            controller.execute_next(self).await?;
        }

        {
            let mut guard = self.plan_controller.lock().await;
            *guard = None;
        }

        self.send_wire_message(clarity_wire::WireMessage::TurnEnd {
            turn_id: String::new(),
        });
        self.maybe_snapshot_post_turn().await;
        self.finish_turn();
        Ok(controller.results())
    }

    /// Create an execution controller for incremental plan execution.
    pub fn plan_controller(&self, plan: Plan) -> PlanExecutionController {
        PlanExecutionController::new(plan)
    }

    /// Skip a pending plan step by id.
    /// Returns an error if no plan is currently executing.
    pub async fn skip_plan_step(&self, step_id: &str) -> Result<(), AgentError> {
        let mut guard = self.plan_controller.lock().await;
        if let Some(ref mut controller) = *guard {
            controller.skip_step(step_id)?;
            self.send_wire_message(clarity_wire::WireMessage::PlanStepSkipped {
                turn_id: String::new(),
                step_id: step_id.to_string(),
            });
            Ok(())
        } else {
            Err(AgentError::Tool(crate::error::ToolError::invalid_params(
                "No plan is currently executing".to_string(),
            )))
        }
    }

    /// Retry a failed plan step by id.
    /// Returns an error if no plan is currently executing.
    pub async fn retry_plan_step(&self, step_id: &str) -> Result<(), AgentError> {
        let mut guard = self.plan_controller.lock().await;
        if let Some(ref mut controller) = *guard {
            controller.retry_step(step_id)
        } else {
            Err(AgentError::Tool(crate::error::ToolError::invalid_params(
                "No plan is currently executing".to_string(),
            )))
        }
    }

    /// Run the agent in Plan approval mode.
    pub async fn execute_plan_mode(&self, query: &str) -> Result<String, AgentError> {
        let plan = self.plan(query).await?;
        self.send_wire_message(clarity_wire::WireMessage::TurnBegin {
            turn_id: String::new(),
            user_input: query.to_string(),
        });
        if !plan.is_empty() {
            self.send_wire_message(clarity_wire::WireMessage::ContentPart {
                turn_id: String::new(),
                text: format!("📋 Executing plan: {}\n{}", plan.title, plan.to_markdown()),
            });
        }
        let results = self.execute_plan(&plan).await?;
        let final_response = crate::agent::run::format_plan_results(&results);
        self.send_wire_message(clarity_wire::WireMessage::TurnEnd {
            turn_id: String::new(),
        });
        Ok(final_response)
    }

    /// Execute an agent team asynchronously.
    pub async fn run_team(
        &self,
        team: clarity_contract::subagent::AgentTeam,
    ) -> Result<clarity_contract::subagent::TeamResult, AgentError> {
        let orchestrator = self.orchestrator.as_ref().ok_or_else(|| {
            AgentError::Tool(crate::error::ToolError::execution_failed(
                "No subagent orchestrator configured".to_string(),
            ))
        })?;

        orchestrator.run_team(team).await.map_err(|e| {
            AgentError::Tool(crate::error::ToolError::execution_failed(format!(
                "Team execution failed: {}",
                e
            )))
        })
    }
}

#[cfg(test)]
impl Agent {
    /// Test helper: install a plan controller directly without running execute_plan.
    pub async fn install_plan_controller_for_test(&self, plan: Plan) {
        let mut guard = self.plan_controller.lock().await;
        *guard = Some(PlanExecutionController::new(plan));
    }

    /// Test helper: read plan controller states for verification.
    pub async fn plan_controller_states_for_test(&self) -> Option<Vec<PlanExecutionState>> {
        let guard = self.plan_controller.lock().await;
        guard.as_ref().map(|c| c.states().to_vec())
    }
}

// ============================================================================
// PlanExecutionController — incremental step-level control
// ============================================================================

/// Incremental plan execution controller.
/// Supports step-level Skip and Retry without mutating the original Plan.
#[derive(Debug, Clone)]
pub struct PlanExecutionController {
    plan: Plan,
    states: Vec<PlanExecutionState>,
    current_idx: usize,
}

impl PlanExecutionController {
    /// Create a new controller with all steps in Pending state.
    pub fn new(plan: Plan) -> Self {
        let states = plan
            .steps
            .iter()
            .map(|s| PlanExecutionState {
                step_id: s.id.clone(),
                status: PlanStepExecutionStatus::Pending,
                result: None,
            })
            .collect();
        Self {
            plan,
            states,
            current_idx: 0,
        }
    }

    /// Snapshot of all step states.
    pub fn states(&self) -> &[PlanExecutionState] {
        &self.states
    }

    /// All completed results so far (includes Failed and Skipped steps).
    pub fn results(&self) -> Vec<PlanResult> {
        self.states
            .iter()
            .filter_map(|s| s.result.clone())
            .collect()
    }

    /// Whether any Pending steps remain.
    pub fn has_next(&self) -> bool {
        self.states
            .iter()
            .skip(self.current_idx)
            .any(|s| s.status == PlanStepExecutionStatus::Pending)
    }

    /// Mark a Pending step as Skipped.
    pub fn skip_step(&mut self, step_id: &str) -> Result<(), AgentError> {
        let idx = self
            .states
            .iter()
            .position(|s| s.step_id == step_id)
            .ok_or_else(|| {
                AgentError::Tool(crate::error::ToolError::invalid_params(format!(
                    "Step '{}' not found",
                    step_id
                )))
            })?;
        let state = &mut self.states[idx];
        if state.status != PlanStepExecutionStatus::Pending {
            return Err(AgentError::Tool(crate::error::ToolError::invalid_params(
                format!(
                    "Cannot skip step '{}' with status {:?}",
                    step_id, state.status
                ),
            )));
        }
        state.status = PlanStepExecutionStatus::Skipped;
        // If we skipped the step at current_idx, advance past it so
        // execute_next/has_next don't stall on a Skipped step.
        if self.current_idx == idx {
            self.current_idx += 1;
        }
        Ok(())
    }

    /// Mark a Failed step as Pending so it can be retried.
    pub fn retry_step(&mut self, step_id: &str) -> Result<(), AgentError> {
        let state = self
            .states
            .iter_mut()
            .find(|s| s.step_id == step_id)
            .ok_or_else(|| {
                AgentError::Tool(crate::error::ToolError::invalid_params(format!(
                    "Step '{}' not found",
                    step_id
                )))
            })?;
        if state.status != PlanStepExecutionStatus::Failed {
            return Err(AgentError::Tool(crate::error::ToolError::invalid_params(
                format!(
                    "Cannot retry step '{}' with status {:?}",
                    step_id, state.status
                ),
            )));
        }
        state.status = PlanStepExecutionStatus::Pending;
        state.result = None;
        // If the retried step is before current_idx, rewind to it.
        if let Some(idx) = self.states.iter().position(|s| s.step_id == step_id) {
            if idx < self.current_idx {
                self.current_idx = idx;
            }
        }
        Ok(())
    }

    /// Execute the next non-skipped Pending step.
    /// Returns `Ok(Some(result))` on completion, `Ok(None)` if all steps done.
    pub async fn execute_next(&mut self, agent: &Agent) -> Result<Option<PlanResult>, AgentError> {
        let idx = self
            .states
            .iter()
            .skip(self.current_idx)
            .position(|s| s.status == PlanStepExecutionStatus::Pending)
            .map(|pos| self.current_idx + pos);

        let idx = match idx {
            Some(i) => i,
            None => return Ok(None),
        };

        let step = &self.plan.steps[idx];
        self.states[idx].status = PlanStepExecutionStatus::Running;
        self.current_idx = idx;

        agent.send_wire_message(clarity_wire::WireMessage::PlanStepBegin {
            turn_id: String::new(),
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

        let (success, output) = match agent.execute_tool_call(&tool_call).await {
            Ok(val) => (true, val.to_string()),
            Err(e) => (false, format!("Error: {}", e)),
        };

        agent.send_wire_message(clarity_wire::WireMessage::PlanStepEnd {
            turn_id: String::new(),
            step_id: step.id.clone(),
            success,
        });

        let result = PlanResult {
            step_id: step.id.clone(),
            success,
            output,
        };

        self.states[idx].status = if success {
            PlanStepExecutionStatus::Success
        } else {
            PlanStepExecutionStatus::Failed
        };
        self.states[idx].result = Some(result.clone());
        self.current_idx = idx + 1;

        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentConfig;
    use crate::types::PlanStep;
    use clarity_llm::api::LlmProvider;
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
                tokio::sync::mpsc::Receiver<Result<clarity_llm::api::StreamDelta, AgentError>>,
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
                tokio::sync::mpsc::Receiver<Result<clarity_llm::api::StreamDelta, AgentError>>,
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

    // ============================================================================
    // PlanExecutionController tests
    // ============================================================================

    fn sample_plan() -> Plan {
        Plan {
            title: "Test Plan".to_string(),
            steps: vec![
                PlanStep {
                    id: "1".to_string(),
                    description: "Step one".to_string(),
                    tool_name: "think".to_string(),
                    tool_params: serde_json::json!({}),
                },
                PlanStep {
                    id: "2".to_string(),
                    description: "Step two".to_string(),
                    tool_name: "think".to_string(),
                    tool_params: serde_json::json!({}),
                },
                PlanStep {
                    id: "3".to_string(),
                    description: "Step three".to_string(),
                    tool_name: "think".to_string(),
                    tool_params: serde_json::json!({}),
                },
            ],
        }
    }

    #[test]
    fn test_controller_init() {
        let controller = PlanExecutionController::new(sample_plan());
        assert_eq!(controller.states().len(), 3);
        assert!(controller
            .states()
            .iter()
            .all(|s| s.status == PlanStepExecutionStatus::Pending));
        assert!(controller.has_next());
    }

    #[test]
    fn test_skip_pending_step() {
        let mut controller = PlanExecutionController::new(sample_plan());
        controller.skip_step("2").unwrap();
        assert_eq!(
            controller.states()[1].status,
            PlanStepExecutionStatus::Skipped
        );
        assert!(controller.has_next());
    }

    #[test]
    fn test_skip_non_pending_fails() {
        let mut controller = PlanExecutionController::new(sample_plan());
        controller.states[0].status = PlanStepExecutionStatus::Running;
        assert!(controller.skip_step("1").is_err());

        controller.states[0].status = PlanStepExecutionStatus::Success;
        assert!(controller.skip_step("1").is_err());

        controller.states[0].status = PlanStepExecutionStatus::Failed;
        assert!(controller.skip_step("1").is_err());
    }

    #[test]
    fn test_retry_failed_step() {
        let mut controller = PlanExecutionController::new(sample_plan());
        controller.states[0].status = PlanStepExecutionStatus::Failed;
        controller.states[0].result = Some(PlanResult {
            step_id: "1".to_string(),
            success: false,
            output: "error".to_string(),
        });

        controller.retry_step("1").unwrap();
        assert_eq!(
            controller.states()[0].status,
            PlanStepExecutionStatus::Pending
        );
        assert!(controller.states()[0].result.is_none());
    }

    #[test]
    fn test_retry_non_failed_fails() {
        let mut controller = PlanExecutionController::new(sample_plan());
        assert!(controller.retry_step("1").is_err()); // Pending

        controller.states[0].status = PlanStepExecutionStatus::Success;
        assert!(controller.retry_step("1").is_err());

        controller.states[0].status = PlanStepExecutionStatus::Skipped;
        assert!(controller.retry_step("1").is_err());
    }

    #[test]
    fn test_has_next_with_skipped_steps() {
        let mut controller = PlanExecutionController::new(sample_plan());
        controller.skip_step("2").unwrap();
        assert!(controller.has_next()); // 1 and 3 still pending

        controller.skip_step("1").unwrap();
        controller.skip_step("3").unwrap();
        assert!(!controller.has_next());
    }

    #[test]
    fn test_results_collects_all() {
        let mut controller = PlanExecutionController::new(sample_plan());
        controller.states[0].result = Some(PlanResult {
            step_id: "1".to_string(),
            success: true,
            output: "ok".to_string(),
        });
        controller.states[1].status = PlanStepExecutionStatus::Skipped;
        controller.states[2].result = Some(PlanResult {
            step_id: "3".to_string(),
            success: false,
            output: "err".to_string(),
        });

        let results = controller.results();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].step_id, "1");
        assert_eq!(results[1].step_id, "3");
    }

    #[test]
    fn test_skip_step_advances_current_idx() {
        let mut controller = PlanExecutionController::new(sample_plan());
        assert_eq!(controller.current_idx, 0);
        controller.skip_step("1").unwrap();
        // Skipping the step at current_idx should advance it
        assert_eq!(controller.current_idx, 1);
        assert_eq!(
            controller.states()[0].status,
            PlanStepExecutionStatus::Skipped
        );
    }

    #[test]
    fn test_skip_non_current_step_preserves_idx() {
        let mut controller = PlanExecutionController::new(sample_plan());
        controller.current_idx = 1;
        controller.skip_step("1").unwrap(); // skip step 0, not current
        assert_eq!(controller.current_idx, 1); // unchanged
    }

    #[test]
    fn test_retry_step_resets_current_idx() {
        let mut controller = PlanExecutionController::new(sample_plan());
        // Simulate: step 0 failed, current_idx advanced to 1
        controller.states[0].status = PlanStepExecutionStatus::Failed;
        controller.current_idx = 1;

        controller.retry_step("1").unwrap();
        // current_idx should rewind so the retried step is picked up
        assert_eq!(controller.current_idx, 0);
        assert_eq!(
            controller.states()[0].status,
            PlanStepExecutionStatus::Pending
        );
        assert!(controller.states()[0].result.is_none());
    }

    // ============================================================================
    // Agent-level plan control integration tests
    // ============================================================================

    #[tokio::test]
    async fn test_agent_skip_plan_step() {
        let registry = crate::registry::mock_registry_with_tools(vec![]);
        let agent = Agent::with_config(registry, AgentConfig::new());
        agent.install_plan_controller_for_test(sample_plan()).await;

        agent.skip_plan_step("2").await.unwrap();

        let states = agent.plan_controller_states_for_test().await.unwrap();
        assert_eq!(states[1].status, PlanStepExecutionStatus::Skipped);
    }

    #[tokio::test]
    async fn test_agent_retry_plan_step() {
        let registry = crate::registry::mock_registry_with_tools(vec![]);
        let agent = Agent::with_config(registry, AgentConfig::new());
        agent.install_plan_controller_for_test(sample_plan()).await;

        // Manually fail step 1
        {
            let mut guard = agent.plan_controller.lock().await;
            let controller = guard.as_mut().unwrap();
            controller.states[0].status = PlanStepExecutionStatus::Failed;
        }

        agent.retry_plan_step("1").await.unwrap();

        let states = agent.plan_controller_states_for_test().await.unwrap();
        assert_eq!(states[0].status, PlanStepExecutionStatus::Pending);
        assert!(states[0].result.is_none());
    }

    #[tokio::test]
    async fn test_agent_skip_plan_step_no_controller() {
        let registry = crate::registry::mock_registry_with_tools(vec![]);
        let agent = Agent::with_config(registry, AgentConfig::new());

        let err = agent.skip_plan_step("1").await.unwrap_err();
        assert!(err.to_string().contains("No plan is currently executing"));
    }

    #[tokio::test]
    async fn test_agent_retry_plan_step_no_controller() {
        let registry = crate::registry::mock_registry_with_tools(vec![]);
        let agent = Agent::with_config(registry, AgentConfig::new());

        let err = agent.retry_plan_step("1").await.unwrap_err();
        assert!(err.to_string().contains("No plan is currently executing"));
    }
}
