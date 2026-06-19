//! Shared pipeline steps for agent execution loops.

use crate::agent::Agent;
use crate::error::AgentError;
use crate::types::ToolCall;
use clarity_contract::{LlmProvider, Message};

/// Run lifecycle hooks before LLM input.
pub(crate) async fn run_hooks(agent: &Agent, messages: &mut Vec<Message>) {
    let hooks_opt = {
        let inner = agent.inner.read();
        inner.hook_registry.clone()
    };
    if let Some(hooks) = hooks_opt {
        let registry = hooks.read().await;
        registry.on_llm_input(messages).await;
    }
}

/// Estimate prompt tokens for a message list.
pub(crate) fn estimate_prompt_tokens(messages: &[Message]) -> u32 {
    crate::agent::compaction_service::CompactionService::estimate_tokens(messages) as u32
}

/// Check if the estimated cost exceeds budget limits.
pub(crate) fn check_budget(
    agent: &Agent,
    llm: &dyn LlmProvider,
    prompt_tokens: u32,
) -> Result<(), AgentError> {
    if let Some(pricing) = llm.capabilities().pricing {
        let estimated_completion = prompt_tokens / 2;
        let estimated_cost = pricing.estimate_cost(prompt_tokens, estimated_completion);
        agent.check_budget(estimated_cost)?;
    }
    Ok(())
}

/// Record actual cost after an LLM call.
pub(crate) fn record_cost(
    agent: &Agent,
    llm: &dyn LlmProvider,
    prompt_tokens: u32,
    completion_tokens: u32,
) {
    if let Some(pricing) = llm.capabilities().pricing {
        let actual_cost = pricing.estimate_cost(prompt_tokens, completion_tokens);
        agent.record_cost(actual_cost);
    }
}

/// Fallback: parse tool calls from content if native tool_calls are empty.
pub(crate) fn parse_tool_calls(response: &clarity_contract::LlmResponse) -> Vec<ToolCall> {
    if response.tool_calls.is_empty() && !response.content.is_empty() {
        if let Some(format) = crate::agent::tool_parser::detect_tool_format(&response.content) {
            tracing::info!(
                "Detected tool format {:?}, parsing tool calls from content",
                format
            );
            crate::agent::tool_parser::parse_tool_calls(&response.content, format)
        } else {
            Vec::new()
        }
    } else {
        response.tool_calls.clone()
    }
}
