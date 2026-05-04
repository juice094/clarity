//! Streaming agent execution loop.

use crate::agent::run::loop_helpers::{fast_trim_tool_results, is_context_overflow_error};
use crate::agent::run::loop_steps::{
    check_budget, estimate_prompt_tokens, parse_tool_calls, record_cost, run_hooks,
};
use crate::agent::run::loop_trait::{
    run_loop_iterations, AgentLoop, DispatchOutcome, IterationResult, LoopOutcome,
};
use crate::agent::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, LlmResponse, Message};
use crate::types::ToolCall;
use clarity_wire::{DraftEvent, WireMessage};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

struct StreamingLoop<'a, F> {
    on_chunk: &'a mut F,
    was_streamed: bool,
}

#[async_trait::async_trait]
impl<F> AgentLoop for StreamingLoop<'_, F>
where
    F: FnMut(&str) + Send + 'static,
{
    async fn before_iteration(
        &mut self,
        agent: &Agent,
        messages: &mut Vec<Message>,
        llm: Arc<dyn LlmProvider>,
    ) {
        if let Some(ref service) = agent.compaction_service {
            let needs = service.needs_compaction(messages);
            if needs {
                agent.send_wire_message(WireMessage::CompactionBegin);
            }
            if let Err(e) = service.maybe_compact(messages, llm.as_ref()).await {
                warn!("Compaction failed: {}", e);
            }
            if needs {
                agent.send_wire_message(WireMessage::CompactionEnd);
            }
        }
    }

    async fn run_iteration(
        &mut self,
        agent: &Agent,
        messages: &mut Vec<Message>,
        tools: &serde_json::Value,
        llm: Arc<dyn LlmProvider>,
    ) -> Result<IterationResult, AgentError> {
        run_hooks(agent, messages).await;

        let mut turn_response: Option<LlmResponse> = None;
        let mut prompt_tokens = 0u32;
        let mut completion_tokens = 0u32;

        let pre_stream_prompt_tokens = estimate_prompt_tokens(messages);
        check_budget(agent, llm.as_ref(), pre_stream_prompt_tokens)?;

        match llm.stream(messages, tools) {
            Ok(mut stream_rx) => {
                prompt_tokens = pre_stream_prompt_tokens;
                agent.send_wire_message(WireMessage::DraftEvent {
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
                                (self.on_chunk)(&content);
                                if !draft_cleared {
                                    agent.send_wire_message(WireMessage::DraftEvent {
                                        event: DraftEvent::Clear,
                                    });
                                    draft_cleared = true;
                                }
                                agent.send_wire_message(WireMessage::DraftEvent {
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
                if !draft_cleared && !tool_calls.is_empty() {
                    agent.send_wire_message(WireMessage::DraftEvent {
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
                    record_cost(agent, llm.as_ref(), prompt_tokens, completion_tokens);
                }
            }
            Err(e) => {
                debug!(
                    "Streaming not supported or failed: {}, falling back to complete()",
                    e
                );
            }
        }

        self.was_streamed = turn_response.is_some();
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
                let mut fallback_prompt_tokens = estimate_prompt_tokens(messages);
                check_budget(agent, llm.as_ref(), fallback_prompt_tokens)?;
                let r = match llm.complete(messages, tools).await {
                    Ok(r) => r,
                    Err(e) if is_context_overflow_error(&e) => {
                        warn!(
                            "Context overflow detected in streaming loop, trimming tool results and retrying"
                        );
                        fast_trim_tool_results(messages);
                        fallback_prompt_tokens = estimate_prompt_tokens(messages);
                        check_budget(agent, llm.as_ref(), fallback_prompt_tokens)?;
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
                record_cost(agent, llm.as_ref(), prompt_tokens, completion_tokens);
                r
            }
        };

        agent.accumulate_usage(prompt_tokens, completion_tokens);
        let tool_calls = parse_tool_calls(&response);

        Ok(IterationResult {
            response_content: response.content,
            tool_calls,
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn handle_final_response(&mut self, agent: &Agent, response: &str, _iteration: usize) {
        if !self.was_streamed && !response.is_empty() {
            for c in response.chars() {
                let chunk = c.to_string();
                (self.on_chunk)(&chunk);
                agent.send_wire_message(WireMessage::ContentPart {
                    text: chunk.clone(),
                });
            }
        }
    }

    async fn dispatch_tool_calls(
        &mut self,
        agent: &Agent,
        tool_calls: &[ToolCall],
        messages: &mut Vec<Message>,
    ) -> DispatchOutcome {
        match agent.dispatch_tool_calls(tool_calls, messages).await {
            Ok(output) => {
                if let Some(question) = output.ask_user_question {
                    DispatchOutcome::Break {
                        final_response: question,
                        is_error: false,
                    }
                } else {
                    DispatchOutcome::Success(output.tool_names)
                }
            }
            Err(e) => {
                warn!("Tool execution failed in stream: {}", e);
                let error_text = format!("\n⚠️ Tool execution failed: {}\n", e);
                (self.on_chunk)(&error_text);
                agent.send_wire_message(WireMessage::ContentPart {
                    text: error_text.clone(),
                });
                DispatchOutcome::Break {
                    final_response: error_text,
                    is_error: true,
                }
            }
        }
    }
}

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
        let mut loop_impl = StreamingLoop {
            on_chunk,
            was_streamed: false,
        };
        let LoopOutcome {
            final_response,
            completed,
            ..
        } = run_loop_iterations(self, &mut loop_impl, messages, tools, llm, cancel_token).await?;

        // If the final response came from fallback (not streamed), simulate
        // streaming from the complete() response for smooth UI.
        if completed && !loop_impl.was_streamed && !final_response.is_empty() {
            for c in final_response.chars() {
                let chunk = c.to_string();
                (loop_impl.on_chunk)(&chunk);
                self.send_wire_message(WireMessage::ContentPart {
                    text: chunk.clone(),
                });
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
