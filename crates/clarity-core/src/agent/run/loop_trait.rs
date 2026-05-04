//! Unified `AgentLoop` trait and shared loop skeleton.

use crate::agent::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, Message};
use crate::types::ToolCall;
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
    Break { final_response: String },
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
    async fn handle_final_response(
        &mut self,
        agent: &Agent,
        response: &str,
        iteration: usize,
    );

    /// Called when the iteration produces tool calls.
    async fn dispatch_tool_calls(
        &mut self,
        agent: &Agent,
        tool_calls: &[ToolCall],
        messages: &mut Vec<Message>,
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

    for iteration in 0..max_iterations {
        tracing::debug!("Iteration {}/{}", iteration + 1, max_iterations);

        if cancel_token.is_cancelled() {
            tracing::warn!("Agent run cancelled");
            return Err(AgentError::Cancelled);
        }

        loop_impl.before_iteration(agent, messages, llm.clone()).await;

        let result = loop_impl
            .run_iteration(agent, messages, tools, llm.clone())
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

        match loop_impl
            .dispatch_tool_calls(agent, &result.tool_calls, messages)
            .await
        {
            DispatchOutcome::Success(names) => tool_names.extend(names),
            DispatchOutcome::Break { final_response: fr } => {
                final_response = fr;
                break;
            }
            DispatchOutcome::Fatal(e) => return Err(e),
        }
    }

    Ok(LoopOutcome {
        final_response,
        completed,
        tool_names,
    })
}
