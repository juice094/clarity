//! Synchronous (non-streaming) agent execution loop.

use crate::agent::Agent;
use crate::agent::run::loop_helpers::{
    fast_trim_tool_results, is_context_overflow_error, messages_contain_vision,
};
use crate::agent::run::loop_steps::{
    check_budget, estimate_prompt_tokens, parse_tool_calls, record_cost, run_hooks,
};
use crate::agent::run::loop_trait::{
    AgentLoop, DispatchOutcome, IterationResult, LoopOutcome, run_loop_iterations,
};
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, Message};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

struct SyncLoop;

#[async_trait::async_trait]
impl AgentLoop for SyncLoop {
    async fn before_iteration(
        &mut self,
        agent: &Agent,
        messages: &mut Vec<Message>,
        llm: Arc<dyn LlmProvider>,
    ) {
        agent.maybe_compact_turn(messages, llm.as_ref()).await;
    }

    async fn run_iteration(
        &mut self,
        agent: &Agent,
        messages: &mut Vec<Message>,
        tools: &serde_json::Value,
        llm: Arc<dyn LlmProvider>,
    ) -> Result<IterationResult, AgentError> {
        run_hooks(agent, messages).await;

        let prompt_tokens = estimate_prompt_tokens(messages);
        check_budget(agent, llm.as_ref(), prompt_tokens)?;

        let (response, actual_prompt_tokens, actual_completion_tokens) =
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(45),
                llm.complete(messages, tools),
            )
            .await
            {
                Ok(Ok(r)) => {
                    let completion_tokens = r.content.len().div_ceil(4) as u32;
                    (r, prompt_tokens, completion_tokens)
                }
                Ok(Err(e)) if is_context_overflow_error(&e) => {
                    warn!(
                        "Context overflow detected in sync loop, trimming tool results and retrying"
                    );
                    fast_trim_tool_results(messages);
                    let retry_prompt_tokens = estimate_prompt_tokens(messages);
                    check_budget(agent, llm.as_ref(), retry_prompt_tokens)?;
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(45),
                        llm.complete(messages, tools),
                    )
                    .await
                    {
                        Ok(Ok(r)) => {
                            let completion_tokens = r.content.len().div_ceil(4) as u32;
                            (r, retry_prompt_tokens, completion_tokens)
                        }
                        Ok(Err(e2)) => return Err(e2),
                        Err(_) => {
                            return Err(AgentError::Llm(
                                "LLM request timed out after 45s".into(),
                            ))
                        }
                    }
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    return Err(AgentError::Llm(
                        "LLM request timed out after 45s".into(),
                    ))
                }
            };

        record_cost(agent, llm.as_ref(), actual_prompt_tokens, actual_completion_tokens);
        agent.accumulate_usage(actual_prompt_tokens, actual_completion_tokens);

        let tool_calls = parse_tool_calls(&response);

        Ok(IterationResult {
            response_content: response.content,
            tool_calls,
            prompt_tokens: actual_prompt_tokens,
            completion_tokens: actual_completion_tokens,
        })
    }

    async fn handle_final_response(
        &mut self,
        agent: &Agent,
        response: &str,
        _iteration: usize,
    ) {
        agent.send_wire_message(clarity_wire::WireMessage::ContentPart {
            text: response.to_string(),
        });
    }

    async fn dispatch_tool_calls(
        &mut self,
        agent: &Agent,
        tool_calls: &[crate::types::ToolCall],
        messages: &mut Vec<Message>,
    ) -> DispatchOutcome {
        match agent.dispatch_tool_calls(tool_calls, messages).await {
            Ok(output) => {
                if let Some(question) = output.ask_user_question {
                    DispatchOutcome::Break { final_response: question }
                } else {
                    DispatchOutcome::Success(output.tool_names)
                }
            }
            Err(e) => DispatchOutcome::Fatal(e),
        }
    }
}

impl Agent {
    /// Shared core of the non-streaming agent loop.
    ///
    /// Iterates up to `max_iterations`, calling the LLM and executing any tool
    /// calls. Returns `(final_response, completed)`.
    pub(crate) async fn run_sync_loop(
        &self,
        messages: &mut Vec<Message>,
        tools: &serde_json::Value,
        llm: Arc<dyn LlmProvider>,
        cancel_token: &CancellationToken,
    ) -> Result<(String, bool, Vec<String>), AgentError> {
        let llm = if messages_contain_vision(messages) {
            if !llm.capabilities().vision {
                if let Some(ref vision_llm) = self.vision_llm() {
                    if !std::sync::Arc::ptr_eq(&llm, vision_llm) {
                        info!("Switching to vision provider for this turn");
                    }
                    vision_llm.clone()
                } else {
                    warn!("Vision content detected but no vision provider configured");
                    llm
                }
            } else {
                llm
            }
        } else {
            llm
        };

        let LoopOutcome {
            final_response,
            completed,
            tool_names,
        } = run_loop_iterations(self, &mut SyncLoop, messages, tools, llm, cancel_token).await?;
        Ok((final_response, completed, tool_names))
    }
}
