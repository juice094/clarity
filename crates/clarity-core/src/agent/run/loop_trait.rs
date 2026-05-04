//! Unified `AgentLoop` trait and shared loop skeleton.

use crate::agent::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, Message};
use crate::types::ToolCall;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Result of a single iteration of the agent loop.
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

/// Generic iteration driver.  Delegates per-iteration logic to `loop_impl` and
/// handles cancellation, max-iteration limits, and turn bookkeeping.
pub(crate) async fn run_loop_iterations<L: AgentLoop>(
    agent: &Agent,
    loop_impl: &mut L,
    messages: &mut Vec<Message>,
    tools: &serde_json::Value,
    llm: Arc<dyn LlmProvider>,
    cancel_token: &CancellationToken,
) -> Result<LoopOutcome, AgentError> {
    let mut final_response = String::new();
    let mut completed = false;
    let mut tool_names = Vec::new();
    let max_iterations = agent.config.max_iterations;
    let mut tool_failure_counts: HashMap<String, u8> = HashMap::new();
    let mut working_tools = tools.clone();

    for iteration in 0..max_iterations {
        // Check global iteration budget before each iteration
        if let Some(ref budget) = agent.config.iteration_budget {
            let remaining = budget.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if remaining == 0 {
                final_response = "Global iteration budget exhausted".to_string();
                break;
            }
        }

        tracing::debug!("Iteration {}/{}", iteration + 1, max_iterations);

        if cancel_token.is_cancelled() {
            tracing::warn!("Agent run cancelled");
            return Err(AgentError::Cancelled);
        }

        loop_impl
            .before_iteration(agent, messages, llm.clone())
            .await;

        let result = loop_impl
            .run_iteration(agent, messages, &working_tools, llm.clone())
            .await?;

        final_response = result.response_content.clone();

        if result.tool_calls.is_empty() {
            loop_impl
                .handle_final_response(agent, &result.response_content, iteration)
                .await;
            tracing::info!("Agent loop completed after {} iterations", iteration + 1);
            completed = true;
            break;
        }

        if !result.response_content.is_empty() {
            agent.send_wire_message(clarity_wire::WireMessage::ContentPart {
                text: result.response_content.clone(),
            });
        }

        messages.push(crate::llm::api::Message {
            role: crate::llm::api::MessageRole::Assistant,
            content: result.response_content,
            tool_calls: Some(result.tool_calls.clone()),
            tool_call_id: None,
        });

        let outcome = loop_impl
            .dispatch_tool_calls(agent, &result.tool_calls, messages, cancel_token)
            .await;

        match &outcome {
            DispatchOutcome::Success(names) => {
                for tc in &result.tool_calls {
                    tool_failure_counts.remove(&tc.function.name);
                }
                tool_names.extend(names.iter().cloned());
            }
            DispatchOutcome::Break {
                is_error: false,
                final_response: fr,
            } => {
                final_response = fr.clone();
                break;
            }
            DispatchOutcome::Break { is_error: true, .. } | DispatchOutcome::Fatal(_) => {
                for tc in &result.tool_calls {
                    let count = tool_failure_counts
                        .entry(tc.function.name.clone())
                        .or_insert(0);
                    *count = count.saturating_add(1);
                    if *count >= 2 {
                        filter_tool_from_schema(&mut working_tools, &tc.function.name);
                    }
                }

                if all_tools_filtered(&working_tools) {
                    let mut parts = Vec::new();
                    for (name, count) in &tool_failure_counts {
                        parts.push(format!("Tool '{}' failed {} times", name, count));
                    }
                    final_response = format!(
                        "All available tools have been disabled due to repeated failures:\n{}",
                        parts.join("\n")
                    );
                    break;
                }

                match outcome {
                    DispatchOutcome::Break {
                        final_response: fr, ..
                    } => {
                        final_response = fr;
                        break;
                    }
                    DispatchOutcome::Fatal(e) => return Err(e),
                    _ => unreachable!(),
                }
            }
        }
    }

    Ok(LoopOutcome {
        final_response,
        completed,
        tool_names,
    })
}

fn filter_tool_from_schema(tools: &mut serde_json::Value, tool_name: &str) {
    if let Some(arr) = tools.as_array_mut() {
        arr.retain(|v| {
            v.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(|name| name != tool_name)
                .unwrap_or(true)
        });
    }
}

fn all_tools_filtered(tools: &serde_json::Value) -> bool {
    tools.as_array().map(|arr| arr.is_empty()).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentConfig};
    use crate::llm::api::{LlmProvider, LlmResponse, Message, StreamDelta};
    use crate::registry::ToolRegistry;
    use crate::types::{FunctionCall, ToolCall};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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

        fn set_prompt_cache_key(&mut self, _key: &str) {}
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
        let mut messages = vec![Message::system("test")];
        let cancel = tokio_util::sync::CancellationToken::new();

        let mut loop_impl = MockLoopCircuit {
            dispatch_count: AtomicUsize::new(0),
        };
        let outcome = run_loop_iterations(
            &agent,
            &mut loop_impl,
            &mut messages,
            &tools,
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
        let mut messages = vec![Message::system("test")];
        let cancel = tokio_util::sync::CancellationToken::new();

        let mut loop_impl = MockLoopBudget {
            iteration_count: AtomicUsize::new(0),
        };
        let outcome = run_loop_iterations(
            &agent,
            &mut loop_impl,
            &mut messages,
            &tools,
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
