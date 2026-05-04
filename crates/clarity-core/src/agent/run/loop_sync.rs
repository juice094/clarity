//! Synchronous (non-streaming) agent execution loop.

use crate::agent::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, Message, MessageRole};
use clarity_wire::WireMessage;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use super::loop_helpers::{
    fast_trim_tool_results, is_context_overflow_error, messages_contain_vision,
};

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

        let mut final_response = String::new();
        let mut completed = false;
        let mut tool_names = Vec::new();

        for iteration in 0..self.config.max_iterations {
            debug!("Iteration {}/{}", iteration + 1, self.config.max_iterations);

            if cancel_token.is_cancelled() {
                warn!("Agent run cancelled");
                return Err(AgentError::Cancelled);
            }

            self.maybe_compact_turn(messages, llm.as_ref()).await;

            let hooks_opt = self.inner.read().unwrap().hook_registry.clone();
            if let Some(hooks) = hooks_opt {
                hooks.on_llm_input(messages).await;
            }

            let prompt_tokens =
                crate::agent::compaction_service::CompactionService::estimate_tokens(messages)
                    as u32;

            // Budget check before LLM call
            if let Some(pricing) = llm.capabilities().pricing {
                let estimated_completion = prompt_tokens / 2;
                let estimated_cost = pricing.estimate_cost(prompt_tokens, estimated_completion);
                self.check_budget(estimated_cost)?;
            }

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
                        warn!("Context overflow detected in sync loop, trimming tool results and retrying");
                        fast_trim_tool_results(messages);
                        let retry_prompt_tokens =
                            crate::agent::compaction_service::CompactionService::estimate_tokens(
                                messages,
                            ) as u32;
                        if let Some(pricing) = llm.capabilities().pricing {
                            let estimated_completion = retry_prompt_tokens / 2;
                            let estimated_cost =
                                pricing.estimate_cost(retry_prompt_tokens, estimated_completion);
                            self.check_budget(estimated_cost)?;
                        }
                        // Retry once after trimming.
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
                        return Err(AgentError::Llm("LLM request timed out after 45s".into()))
                    }
                };

            // Record actual cost
            if let Some(pricing) = llm.capabilities().pricing {
                let actual_cost =
                    pricing.estimate_cost(actual_prompt_tokens, actual_completion_tokens);
                self.record_cost(actual_cost);
            }

            // O4: Character-count/4 is a rough heuristic; replace with real tokenizer
            // count when clarity-core gains a tokenizer abstraction.
            self.accumulate_usage(actual_prompt_tokens, actual_completion_tokens);
            final_response = response.content.clone();

            // Fallback: parse tool calls from content if native tool_calls are empty
            let tool_calls = if response.tool_calls.is_empty() && !response.content.is_empty() {
                if let Some(format) = crate::agent::tool_parser::detect_tool_format(&response.content) {
                    info!(
                        "Detected tool format {:?}, parsing tool calls from content",
                        format
                    );
                    crate::agent::tool_parser::parse_tool_calls(&response.content, format)
                } else {
                    Vec::new()
                }
            } else {
                response.tool_calls.clone()
            };

            if tool_calls.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: response.content.clone(),
                });
                info!("Agent loop completed after {} iterations", iteration + 1);
                completed = true;
                break;
            }

            if !response.content.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: response.content.clone(),
                });
            }

            messages.push(Message {
                role: MessageRole::Assistant,
                content: response.content,
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            });

            tool_names.extend(self.dispatch_tool_calls(&tool_calls, messages).await?);
        }

        Ok((final_response, completed, tool_names))
    }
}
