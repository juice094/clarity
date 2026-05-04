//! Streaming agent execution loop.

use crate::agent::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, LlmResponse, Message, MessageRole};
use crate::types::ToolCall;
use clarity_wire::{DraftEvent, WireMessage};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use super::loop_helpers::{fast_trim_tool_results, is_context_overflow_error};

impl Agent {
    async fn run_streaming_loop<F>(
        &self,
        messages: &mut Vec<Message>,
        tools: &serde_json::Value,
        llm: Arc<dyn LlmProvider>,
        on_chunk: &mut F,
        cancel_token: &CancellationToken,
    ) -> Result<(String, bool), AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let mut final_response = String::new();
        let mut completed = false;

        for iteration in 0..self.config.max_iterations {
            debug!("Iteration {}/{}", iteration + 1, self.config.max_iterations);

            if cancel_token.is_cancelled() {
                warn!("Agent run streaming cancelled");
                return Err(AgentError::Cancelled);
            }

            // Proactive compaction via CompactionService
            if let Some(ref service) = self.compaction_service {
                let needs = service.needs_compaction(messages);
                if needs {
                    self.send_wire_message(WireMessage::CompactionBegin);
                }
                if let Err(e) = service.maybe_compact(messages, llm.as_ref()).await {
                    warn!("Compaction failed: {}", e);
                }
                if needs {
                    self.send_wire_message(WireMessage::CompactionEnd);
                }
            }

            let hooks_opt = self.inner.read().unwrap().hook_registry.clone();
            if let Some(hooks) = hooks_opt {
                hooks.on_llm_input(messages).await;
            }

            // Stream-first: try streaming, fall back to complete() if unsupported or errors.
            let mut turn_response: Option<LlmResponse> = None;
            let mut prompt_tokens = 0u32;
            let mut completion_tokens = 0u32;

            // Pre-stream budget check
            let pre_stream_prompt_tokens =
                crate::agent::compaction_service::CompactionService::estimate_tokens(messages)
                    as u32;
            if let Some(pricing) = llm.capabilities().pricing {
                let estimated_completion = pre_stream_prompt_tokens / 2;
                let estimated_cost =
                    pricing.estimate_cost(pre_stream_prompt_tokens, estimated_completion);
                self.check_budget(estimated_cost)?;
            }

            match llm.stream(messages, tools) {
                Ok(mut stream_rx) => {
                    prompt_tokens = pre_stream_prompt_tokens;
                    // Send progress indicator before first chunk arrives
                    self.send_wire_message(WireMessage::DraftEvent {
                        event: DraftEvent::Progress {
                            text: "thinking...".to_string(),
                        },
                    });
                    let mut accumulated = String::new();
                    let mut tool_calls: Vec<ToolCall> = Vec::new();
                    let mut stream_ok = true;
                    let mut draft_cleared = false;
                    while let Some(chunk_result) = stream_rx.recv().await {
                        match chunk_result {
                            Ok(delta) => {
                                if let Some(content) = delta.content {
                                    accumulated.push_str(&content);
                                    on_chunk(&content);
                                    if !draft_cleared {
                                        self.send_wire_message(WireMessage::DraftEvent {
                                            event: DraftEvent::Clear,
                                        });
                                        draft_cleared = true;
                                    }
                                    self.send_wire_message(WireMessage::DraftEvent {
                                        event: DraftEvent::Content { text: content },
                                    });
                                }
                                for call in delta.tool_calls {
                                    tool_calls.push(call);
                                }
                            }
                            Err(e) => {
                                warn!("Stream error: {}, falling back to complete()", e);
                                stream_ok = false;
                                break;
                            }
                        }
                    }
                    // If no content was received but we have tool_calls, clear the progress indicator
                    if !draft_cleared && !tool_calls.is_empty() {
                        self.send_wire_message(WireMessage::DraftEvent {
                            event: DraftEvent::Clear,
                        });
                    }
                    if stream_ok {
                        completion_tokens = accumulated.len().div_ceil(4) as u32;
                        turn_response = Some(LlmResponse {
                            content: accumulated,
                            tool_calls,
                            is_complete: true,
                        });
                        // Record actual cost for successful stream
                        if let Some(pricing) = llm.capabilities().pricing {
                            let actual_cost =
                                pricing.estimate_cost(prompt_tokens, completion_tokens);
                            self.record_cost(actual_cost);
                        }
                    }
                }
                Err(e) => {
                    debug!(
                        "Streaming not supported or failed: {}, falling back to complete()",
                        e
                    );
                }
            }

            let was_streamed = turn_response.is_some();
            let response = match turn_response {
                Some(r) => {
                    debug!(
                        "Using streamed response (len={}, tool_calls={})",
                        r.content.len(),
                        r.tool_calls.len()
                    );
                    r
                }
                None => {
                    let mut fallback_prompt_tokens =
                        crate::agent::compaction_service::CompactionService::estimate_tokens(
                            messages,
                        ) as u32;
                    if let Some(pricing) = llm.capabilities().pricing {
                        let estimated_completion = fallback_prompt_tokens / 2;
                        let estimated_cost =
                            pricing.estimate_cost(fallback_prompt_tokens, estimated_completion);
                        self.check_budget(estimated_cost)?;
                    }
                    let r = match llm.complete(messages, tools).await {
                        Ok(r) => r,
                        Err(e) if is_context_overflow_error(&e) => {
                            warn!("Context overflow detected in streaming loop, trimming tool results and retrying");
                            fast_trim_tool_results(messages);
                            fallback_prompt_tokens =
                                crate::agent::compaction_service::CompactionService::estimate_tokens(
                                    messages,
                                ) as u32;
                            if let Some(pricing) = llm.capabilities().pricing {
                                let estimated_completion = fallback_prompt_tokens / 2;
                                let estimated_cost = pricing
                                    .estimate_cost(fallback_prompt_tokens, estimated_completion);
                                self.check_budget(estimated_cost)?;
                            }
                            llm.complete(messages, tools).await?
                        }
                        Err(e) => return Err(e),
                    };
                    debug!(
                        "Using complete() response (len={}, tool_calls={})",
                        r.content.len(),
                        r.tool_calls.len()
                    );
                    prompt_tokens = fallback_prompt_tokens;
                    completion_tokens = r.content.len().div_ceil(4) as u32;
                    // Record actual cost for fallback complete
                    if let Some(pricing) = llm.capabilities().pricing {
                        let actual_cost = pricing.estimate_cost(prompt_tokens, completion_tokens);
                        self.record_cost(actual_cost);
                    }
                    r
                }
            };

            self.accumulate_usage(prompt_tokens, completion_tokens);

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
                // No tool calls: final answer.
                // If we arrived here via fallback (turn_response was None), simulate
                // streaming from the complete() response for smooth UI.
                if !was_streamed && !response.content.is_empty() {
                    for c in response.content.chars() {
                        let chunk = c.to_string();
                        on_chunk(&chunk);
                        self.send_wire_message(WireMessage::ContentPart {
                            text: chunk.clone(),
                        });
                    }
                }

                final_response = response.content.clone();
                if final_response.is_empty() {
                    warn!(
                        "Empty final response on iteration {} (was_streamed={})",
                        iteration + 1,
                        was_streamed
                    );
                }
                info!("Agent loop completed after {} iterations", iteration + 1);
                completed = true;
                break;
            }

            // Tool-calling round: send assistant content (if any)
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

            if let Err(e) = self.dispatch_tool_calls(&tool_calls, messages).await {
                warn!("Tool execution failed in stream: {}", e);
                // Emit the error as content so the user sees what happened,
                // then break the loop to prevent infinite retries.
                let error_text = format!("\n⚠️ Tool execution failed: {}\n", e);
                on_chunk(&error_text);
                self.send_wire_message(WireMessage::ContentPart {
                    text: error_text.clone(),
                });
                final_response = error_text;
                break;
            }
        }

        Ok((final_response, completed))
    }

    /// Shared orchestration for a streaming turn: setup → loop → teardown.
    pub(crate) async fn run_streaming_turn<F>(
        &self,
        messages: Vec<Message>,
        query_hint: &str,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        self.refresh_context().await;

        let cancel_token = self.begin_turn()?;

        // Discover project-local skills and activate those matching current file paths.
        if let Some(ref registry) = self.skill_registry() {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }

        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        info!("Starting streaming agent turn for query: {}", query_hint);

        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query_hint.to_string(),
        });

        let mut messages = messages;
        let mut on_chunk = on_chunk;
        let loop_result = self
            .run_streaming_loop(&mut messages, &tools, llm, &mut on_chunk, &cancel_token)
            .await;

        // Capture usage before finish_turn clears turn_context.
        let usage = self.get_session_usage();

        self.finish_turn();

        let (final_response, completed) = loop_result?;

        // Teardown
        self.send_wire_message(WireMessage::TurnEnd);
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        let memory_content = if completed {
            format!("User: {}\nAssistant: {}", query_hint, final_response)
        } else {
            format!(
                "User: {}\nAssistant: [max iterations reached] {}",
                query_hint, final_response
            )
        };
        self.store_conversation_memory(memory_content).await;

        if let Some(ref ticker) = self.memory_ticker {
            match ticker.notify_turn_and_wait("default").await {
                Some(Ok(results)) => {
                    info!(
                        "Memory ticker triggered, compilation results: {:?}",
                        results
                    );
                }
                Some(Err(e)) => {
                    warn!("Memory ticker compilation failed: {}", e);
                }
                None => {
                    debug!("Memory ticker not triggered yet");
                }
            }
        }

        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            ))
        }
    }
}
