//! Unified `AgentLoop` trait and shared loop skeleton.

use crate::agent::Agent;
use crate::agent::lifecycle::{RunEvent, RunState};
use crate::agent::tool_prompt_manager::ToolPromptManager;
use crate::agent::yolo_guardrails::{GuardrailOutcome, GuardrailState};
use crate::error::AgentError;
use crate::types::ToolCall;
use clarity_contract::{LlmProvider, Message};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Result of a single iteration of the agent loop.
// Intentionally retained: token fields are populated by loop implementations and
// reserved for future cost/telemetry accounting.
#[allow(dead_code)]
pub(crate) struct IterationResult {
    pub response_content: String,
    pub tool_calls: Vec<ToolCall>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// Final outcome after running all iterations.
pub(crate) struct LoopOutcome {
    pub final_response: String,
    pub completed: bool,
    pub tool_names: Vec<String>,
}

/// Outcome of dispatching tool calls.
pub(crate) enum DispatchOutcome {
    Success(Vec<String>),
    Break {
        final_response: String,
        is_error: bool,
    },
    Fatal(AgentError),
}

/// Trait for pluggable agent loop implementations.
#[async_trait::async_trait]
pub(crate) trait AgentLoop: Send {
    /// Called at the start of each iteration before the LLM call.
    async fn before_iteration(
        &mut self,
        agent: &Agent,
        messages: &mut Vec<Message>,
        llm: Arc<dyn LlmProvider>,
    );

    /// Execute one iteration: LLM inference, cost recording, usage accumulation,
    /// and tool-parser fallback.
    async fn run_iteration(
        &mut self,
        agent: &Agent,
        messages: &mut Vec<Message>,
        tools: &serde_json::Value,
        llm: Arc<dyn LlmProvider>,
    ) -> Result<IterationResult, AgentError>;

    /// Called when the iteration produces no tool calls (final response).
    async fn handle_final_response(&mut self, agent: &Agent, response: &str, iteration: usize);

    /// Called when the iteration produces tool calls.
    async fn dispatch_tool_calls(
        &mut self,
        agent: &Agent,
        tool_calls: &[ToolCall],
        messages: &mut Vec<Message>,
        cancel_token: &CancellationToken,
    ) -> DispatchOutcome;
}

/// Apply a lifecycle event to the running state and record the transition.
fn apply_and_record(
    agent: &Agent,
    state: RunState,
    event: RunEvent,
) -> Result<RunState, AgentError> {
    let new_state = state.apply(event.clone())?;
    agent.record_lifecycle_event(event, new_state.clone());
    Ok(new_state)
}

/// Generic iteration driver.  Delegates per-iteration logic to `loop_impl` and
/// handles cancellation, max-iteration limits, and turn bookkeeping.
pub(crate) async fn run_loop_iterations<L: AgentLoop>(
    agent: &Agent,
    loop_impl: &mut L,
    messages: &mut Vec<Message>,
    tool_prompt_manager: &mut ToolPromptManager,
    llm: Arc<dyn LlmProvider>,
    cancel_token: &CancellationToken,
) -> Result<LoopOutcome, AgentError> {
    let mut tool_names = Vec::new();
    let max_iterations = agent.config.max_iterations;
    let mut tool_failure_counts: HashMap<String, u8> = HashMap::new();
    let mut total_tool_calls: usize = 0;

    // Load the initial turn state; begin_turn() sets it to Planning.
    let mut run_state = agent
        .inner
        .read()
        .turn_context
        .as_ref()
        .map(|ctx| ctx.run_state.clone())
        .unwrap_or(RunState::Planning);

    for iteration in 0..max_iterations {
        // Check global iteration budget before each iteration
        if let Some(ref budget) = agent.config.iteration_budget {
            let remaining = budget.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if remaining == 0 {
                let reason = "Global iteration budget exhausted".to_string();
                run_state = apply_and_record(
                    agent,
                    run_state,
                    RunEvent::Stopped {
                        reason: reason.clone(),
                    },
                )
                .ok()
                .unwrap_or(RunState::Interrupted { reason });
                break;
            }
        }

        tracing::debug!("Iteration {}/{}", iteration + 1, max_iterations);

        if cancel_token.is_cancelled() {
            tracing::warn!("Agent run cancelled");
            let reason = "cancelled by user".to_string();
            run_state = apply_and_record(
                agent,
                run_state,
                RunEvent::Cancelled {
                    reason: reason.clone(),
                },
            )
            .ok()
            .unwrap_or(RunState::Interrupted { reason });
            break;
        }

        loop_impl
            .before_iteration(agent, messages, llm.clone())
            .await;

        let result = loop_impl
            .run_iteration(
                agent,
                messages,
                tool_prompt_manager.tools_value(),
                llm.clone(),
            )
            .await?;

        if result.tool_calls.is_empty() {
            loop_impl
                .handle_final_response(agent, &result.response_content, iteration)
                .await;
            tracing::info!("Agent loop completed after {} iterations", iteration + 1);
            run_state = apply_and_record(
                agent,
                run_state,
                RunEvent::FinalResponse {
                    response: result.response_content,
                },
            )?;
            break;
        }

        run_state = apply_and_record(
            agent,
            run_state,
            RunEvent::ToolCallsRequested {
                count: result.tool_calls.len(),
            },
        )?;

        if !result.response_content.is_empty() {
            agent.send_wire_message(clarity_wire::WireMessage::ContentPart {
                turn_id: String::new(),
                text: result.response_content.clone(),
            });
        }

        messages.push(clarity_contract::Message {
            role: clarity_contract::MessageRole::Assistant,
            content: result.response_content,
            tool_calls: Some(result.tool_calls.clone()),
            tool_call_id: None,
        });

        let outcome = loop_impl
            .dispatch_tool_calls(agent, &result.tool_calls, messages, cancel_token)
            .await;

        match outcome {
            DispatchOutcome::Success(names) => {
                for tc in &result.tool_calls {
                    tool_failure_counts.remove(&tc.function.name);
                }
                tool_names.extend(names.iter().cloned());
                total_tool_calls += names.len();
                run_state = apply_and_record(
                    agent,
                    run_state,
                    RunEvent::ToolsSucceeded { tool_names: names },
                )?;

                // Check auto-execution convergence guardrails.
                if let Some(detector) = agent
                    .inner
                    .read()
                    .turn_context
                    .as_ref()
                    .map(|ctx| &ctx.loop_detector)
                {
                    let state = GuardrailState {
                        total_tool_calls,
                        detector,
                    };
                    match agent.config.yolo_guardrails.check(&state) {
                        GuardrailOutcome::Ok => {}
                        GuardrailOutcome::AskUser { question } => {
                            run_state =
                                apply_and_record(agent, run_state, RunEvent::AskUser { question })?;
                            break;
                        }
                        GuardrailOutcome::Stop { reason } => {
                            run_state =
                                apply_and_record(agent, run_state, RunEvent::Stopped { reason })?;
                            break;
                        }
                    }
                }
            }
            DispatchOutcome::Break {
                is_error: false,
                final_response: fr,
            } => {
                run_state = apply_and_record(agent, run_state, RunEvent::AskUser { question: fr })?;
                break;
            }
            DispatchOutcome::Break { is_error: true, .. } | DispatchOutcome::Fatal(_) => {
                for tc in &result.tool_calls {
                    let count = tool_failure_counts
                        .entry(tc.function.name.clone())
                        .or_insert(0);
                    *count = count.saturating_add(1);
                    if *count >= 2 {
                        tool_prompt_manager.filter_tool(&tc.function.name);
                    }
                }

                if tool_prompt_manager
                    .tools_value()
                    .as_array()
                    .map(|arr| arr.is_empty())
                    .unwrap_or(true)
                {
                    let mut parts = Vec::new();
                    for (name, count) in &tool_failure_counts {
                        parts.push(format!("Tool '{}' failed {} times", name, count));
                    }
                    let reason = format!(
                        "All available tools have been disabled due to repeated failures:\n{}",
                        parts.join("\n")
                    );
                    run_state = apply_and_record(agent, run_state, RunEvent::Stopped { reason })?;
                    break;
                }

                match outcome {
                    DispatchOutcome::Break {
                        final_response: fr, ..
                    } => {
                        run_state =
                            apply_and_record(agent, run_state, RunEvent::Stopped { reason: fr })?;
                        break;
                    }
                    DispatchOutcome::Fatal(e) => {
                        // Fatal errors propagate immediately; do not mask them as
                        // an Interrupted state.
                        return Err(e);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // Persist the final turn state.
    if let Some(ctx) = agent.inner.write().turn_context.as_mut() {
        ctx.run_state = run_state.clone();
    }

    let final_response = run_state.response().unwrap_or("").to_string();
    let completed = matches!(run_state, RunState::Complete { .. });

    Ok(LoopOutcome {
        final_response,
        completed,
        tool_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentConfig};
    use crate::registry::ToolRegistry;
    use crate::types::{FunctionCall, ToolCall};
    use clarity_contract::{LlmProvider, LlmResponse, Message, StreamDelta};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockLlmCircuit;

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmCircuit {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<LlmResponse, AgentError> {
            unreachable!()
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
        {
            unreachable!()
        }

        fn set_prompt_cache_key(&self, _key: &str) {}
    }

    struct MockLoopCircuit {
        dispatch_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl AgentLoop for MockLoopCircuit {
        async fn before_iteration(
            &mut self,
            _agent: &Agent,
            _messages: &mut Vec<Message>,
            _llm: Arc<dyn LlmProvider>,
        ) {
        }

        async fn run_iteration(
            &mut self,
            _agent: &Agent,
            _messages: &mut Vec<Message>,
            _tools: &serde_json::Value,
            _llm: Arc<dyn LlmProvider>,
        ) -> Result<IterationResult, AgentError> {
            Ok(IterationResult {
                response_content: "test".to_string(),
                tool_calls: vec![
                    ToolCall {
                        id: "call_1".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: "bad_tool".to_string(),
                            arguments: "{}".to_string(),
                        },
                    },
                    ToolCall {
                        id: "call_2".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: "bad_tool".to_string(),
                            arguments: "{}".to_string(),
                        },
                    },
                ],
                prompt_tokens: 0,
                completion_tokens: 0,
            })
        }

        async fn handle_final_response(
            &mut self,
            _agent: &Agent,
            _response: &str,
            _iteration: usize,
        ) {
        }

        async fn dispatch_tool_calls(
            &mut self,
            _agent: &Agent,
            _tool_calls: &[ToolCall],
            _messages: &mut Vec<Message>,
            _cancel_token: &CancellationToken,
        ) -> DispatchOutcome {
            let count = self.dispatch_count.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                DispatchOutcome::Break {
                    final_response: "error".to_string(),
                    is_error: true,
                }
            } else {
                DispatchOutcome::Success(vec!["bad_tool".to_string()])
            }
        }
    }

    struct MockLoopBudget {
        iteration_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl AgentLoop for MockLoopBudget {
        async fn before_iteration(
            &mut self,
            _agent: &Agent,
            _messages: &mut Vec<Message>,
            _llm: Arc<dyn LlmProvider>,
        ) {
        }

        async fn run_iteration(
            &mut self,
            _agent: &Agent,
            _messages: &mut Vec<Message>,
            _tools: &serde_json::Value,
            _llm: Arc<dyn LlmProvider>,
        ) -> Result<IterationResult, AgentError> {
            self.iteration_count.fetch_add(1, Ordering::SeqCst);
            Ok(IterationResult {
                response_content: "test".to_string(),
                tool_calls: vec![],
                prompt_tokens: 0,
                completion_tokens: 0,
            })
        }

        async fn handle_final_response(
            &mut self,
            _agent: &Agent,
            _response: &str,
            _iteration: usize,
        ) {
        }

        async fn dispatch_tool_calls(
            &mut self,
            _agent: &Agent,
            _tool_calls: &[ToolCall],
            _messages: &mut Vec<Message>,
            _cancel_token: &CancellationToken,
        ) -> DispatchOutcome {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn agent_tool_failure_circuit_breaker() {
        let registry = ToolRegistry::new();
        let config = AgentConfig::new().with_max_iterations(5);
        let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlmCircuit));

        let tools = serde_json::json!([{
            "type": "function",
            "function": {
                "name": "bad_tool"
            }
        }]);
        let mut tool_prompt_manager =
            crate::agent::tool_prompt_manager::ToolPromptManager::new(&tools);
        let mut messages = vec![Message::system("test")];
        let cancel = tokio_util::sync::CancellationToken::new();

        let mut loop_impl = MockLoopCircuit {
            dispatch_count: AtomicUsize::new(0),
        };
        let outcome = run_loop_iterations(
            &agent,
            &mut loop_impl,
            &mut messages,
            &mut tool_prompt_manager,
            Arc::new(MockLlmCircuit),
            &cancel,
        )
        .await
        .unwrap();

        assert!(
            outcome
                .final_response
                .contains("disabled due to repeated failures"),
            "Expected circuit-breaker summary, got: {}",
            outcome.final_response
        );
        assert!(
            outcome.final_response.contains("bad_tool"),
            "Expected bad_tool mentioned, got: {}",
            outcome.final_response
        );
        assert!(!outcome.completed);
    }

    #[tokio::test]
    async fn agent_iteration_budget_exhausted() {
        let registry = ToolRegistry::new();
        let budget = Arc::new(AtomicUsize::new(0));
        let config = AgentConfig::new()
            .with_max_iterations(5)
            .with_iteration_budget(budget.clone());
        let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlmCircuit));

        let tools = serde_json::json!([]);
        let mut tool_prompt_manager =
            crate::agent::tool_prompt_manager::ToolPromptManager::new(&tools);
        let mut messages = vec![Message::system("test")];
        let cancel = tokio_util::sync::CancellationToken::new();

        let mut loop_impl = MockLoopBudget {
            iteration_count: AtomicUsize::new(0),
        };
        let outcome = run_loop_iterations(
            &agent,
            &mut loop_impl,
            &mut messages,
            &mut tool_prompt_manager,
            Arc::new(MockLlmCircuit),
            &cancel,
        )
        .await
        .unwrap();

        assert_eq!(
            outcome.final_response, "Global iteration budget exhausted",
            "Expected budget-exhausted message"
        );
        assert!(!outcome.completed);
        assert_eq!(
            loop_impl.iteration_count.load(Ordering::SeqCst),
            0,
            "No iterations should run when budget is zero"
        );
    }
}
