//! Agent execution loops: synchronous, streaming, and shared core.

use super::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, LlmResponse, Message, MessageRole};
use crate::types::ToolCall;
use clarity_wire::WireMessage;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

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
    ) -> Result<(String, bool), AgentError> {
        let mut final_response = String::new();
        let mut completed = false;

        for iteration in 0..self.config.max_iterations {
            debug!("Iteration {}/{}", iteration + 1, self.config.max_iterations);

            if cancel_token.is_cancelled() {
                warn!("Agent run cancelled");
                return Err(AgentError::Cancelled);
            }

            // Proactive compaction via CompactionService
            if let Some(ref service) = self.compaction_service {
                if let Err(e) = service.maybe_compact(messages, llm.as_ref()).await {
                    warn!("Compaction failed: {}", e);
                }
            }

            if self.should_compact(messages).await {
                match self.compact_messages(messages).await {
                    Ok(compacted) => {
                        info!(
                            "Context compacted: {} messages -> {} messages",
                            messages.len(),
                            compacted.len()
                        );
                        *messages = compacted;
                    }
                    Err(e) => {
                        warn!("Failed to compact messages: {}", e);
                    }
                }
            }

            let prompt_tokens =
                crate::agent::compaction_service::CompactionService::estimate_tokens(messages)
                    as u32;
            let response = tokio::time::timeout(
                tokio::time::Duration::from_secs(45),
                llm.complete(messages, tools),
            )
            .await
            .map_err(|_| AgentError::Llm("LLM request timed out after 45s".into()))??;
            let completion_tokens = response.content.len().div_ceil(4) as u32;
            self.accumulate_usage(prompt_tokens, completion_tokens);
            final_response = response.content.clone();

            if response.tool_calls.is_empty() {
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
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None,
            });

            for tool_call in &response.tool_calls {
                self.send_wire_message(WireMessage::StepBegin {
                    tool_name: tool_call.function.name.clone(),
                });

                let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                self.send_wire_message(WireMessage::ToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    arguments: args,
                });

                let result = self.execute_tool_call(tool_call).await;
                let result_content = match result {
                    Ok(value) => value.to_string(),
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                };

                self.send_wire_message(WireMessage::ToolResult {
                    id: tool_call.id.clone(),
                    result: result_content.clone(),
                });

                messages.push(Message::tool(&tool_call.id, result_content));
            }
        }

        Ok((final_response, completed))
    }

    /// Run the agent with a user query
    ///
    /// This is the main entry point that orchestrates the agent loop:
    /// 1. Sends user query to LLM with available tools
    /// 2. Processes any tool calls
    /// 3. Sends tool results back to LLM
    /// 4. Returns final response
    ///
    /// # Arguments
    ///
    /// * `query` - The user's request
    ///
    /// # Returns
    ///
    /// The final response from the agent
    pub async fn run(&self, query: impl AsRef<str>) -> Result<String, AgentError> {
        // Plan mode: bypass the ReAct loop and use plan-driven execution.
        // This avoids the LLM "thinking step-by-step" and instead runs a
        // pre-generated structured plan step-by-step.
        if self.approval_mode == crate::approval::ApprovalMode::Plan {
            let plan = self.plan(query.as_ref()).await?;
            self.send_wire_message(WireMessage::TurnBegin {
                user_input: query.as_ref().to_string(),
            });
            if !plan.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: format!("📋 Executing plan: {}\n{}", plan.title, plan.to_markdown()),
                });
            }
            let results = self.execute_plan(&plan).await?;
            let final_response = format_plan_results(&results);
            self.send_wire_message(WireMessage::TurnEnd);
            return Ok(final_response);
        }

        let cancel_token = self.begin_turn()?;

        // Discover project-local skills and activate those matching current file paths.
        if let Some(ref registry) = self.skill_registry {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }

        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        // Build system prompt with optional memory context
        let base_system_prompt = self.build_system_prompt();
        let mut system_prompt = base_system_prompt;

        if let Some(ref store) = self.memory_store {
            match store.search(query.as_ref(), 5).await {
                Ok(memories) => {
                    if !memories.is_empty() {
                        let memory_text = memories
                            .iter()
                            .map(|m| format!("- {}", m.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        system_prompt
                            .push_str(&format!("\n\n# Relevant Memories\n{}\n", memory_text));
                    }
                }
                Err(e) => {
                    warn!("Failed to retrieve memories: {}", e);
                }
            }
        }

        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(query.as_ref()),
        ];

        info!("Starting agent loop for query: {}", query.as_ref());

        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });

        let (final_response, completed) = self.run_sync_loop(&mut messages, &tools, llm, &cancel_token).await?;
        self.finish_turn();

        self.send_wire_message(WireMessage::TurnEnd);

        let usage = self.get_session_usage();
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        // Persist interaction to memory
        let memory_content = if completed {
            format!("User: {}\nAssistant: {}", query.as_ref(), final_response)
        } else {
            format!(
                "User: {}\nAssistant: [max iterations reached] {}",
                query.as_ref(),
                final_response
            )
        };
        self.store_conversation_memory(memory_content.clone()).await;
        self.maybe_extract_memories(memory_content);

        if let Some(ref ticker) = self.memory_ticker {
            match ticker.notify_turn_and_wait("default").await {
                Some(Ok(results)) => {
                    info!("Memory ticker triggered, compilation results: {:?}", results);
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
            let transcript = serde_json::to_string(&messages).unwrap_or_default();
            self.maybe_extract_memories(transcript);
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            ))
        }
    }

    /// Run a synchronous (non-streaming) agent loop with pre-built messages.
    /// Used by the Gateway for non-streaming chat completion requests.
    pub async fn run_with_messages_sync(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<String, AgentError> {
        let cancel_token = self.begin_turn()?;

        // Discover project-local skills and activate those matching current file paths.
        if let Some(ref registry) = self.skill_registry {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }

        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        let (final_response, completed) = self.run_sync_loop(&mut messages, &tools, llm, &cancel_token).await?;
        self.finish_turn();

        self.send_wire_message(WireMessage::TurnEnd);

        let usage = self.get_session_usage();
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            ))
        }
    }

    /// Run the agent with streaming response.
    ///
    /// Same as `run()`, but streams the final assistant response via `on_chunk`.
    /// Tool-calling rounds still use `complete()` internally; only the final
    /// text response is streamed.
    pub async fn run_streaming<F>(
        &self,
        query: impl AsRef<str>,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let cancel_token = self.begin_turn()?;

        // Discover project-local skills and activate those matching current file paths.
        if let Some(ref registry) = self.skill_registry {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }

        let llm = self.llm().ok_or(AgentError::Unconfigured)?;

        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        let base_system_prompt = self.build_system_prompt();
        let mut system_prompt = base_system_prompt;

        if let Some(ref store) = self.memory_store {
            match store.search(query.as_ref(), 5).await {
                Ok(memories) => {
                    if !memories.is_empty() {
                        let memory_text = memories
                            .iter()
                            .map(|m| format!("- {}", m.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        system_prompt
                            .push_str(&format!("\n\n# Relevant Memories\n{}\n", memory_text));
                    }
                }
                Err(e) => {
                    warn!("Failed to retrieve memories: {}", e);
                }
            }
        }

        let messages = vec![
            Message::system(system_prompt),
            Message::user(query.as_ref()),
        ];

        info!(
            "Starting streaming agent loop for query: {}",
            query.as_ref()
        );

        // Send TurnBegin message
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });

        let result = self
            .run_streaming_loop(messages, query.as_ref(), tools, llm.clone(), on_chunk, &cancel_token)
            .await;
        self.finish_turn();
        result
    }

    /// Run the streaming agent loop with a pre-built message list.
    pub(crate) async fn run_streaming_with_messages<F>(
        &self,
        messages: Vec<Message>,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let cancel_token = self.begin_turn()?;

        // Discover project-local skills and activate those matching current file paths.
        if let Some(ref registry) = self.skill_registry {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }

        let llm = self.llm().ok_or(AgentError::Unconfigured)?;

        let tools = self.registry.get_tool_schemas()?;

        let query_hint = messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        info!(
            "Starting streaming agent loop with {} messages",
            messages.len()
        );

        // Send TurnBegin message
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query_hint.clone(),
        });

        let result = self
            .run_streaming_loop(messages, &query_hint, tools, llm.clone(), on_chunk, &cancel_token)
            .await;
        self.finish_turn();
        result
    }

    async fn run_streaming_loop<F>(
        &self,
        mut messages: Vec<Message>,
        query_hint: &str,
        tools: serde_json::Value,
        llm: Arc<dyn LlmProvider>,
        mut on_chunk: F,
        cancel_token: &CancellationToken,
    ) -> Result<String, AgentError>
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
                if let Err(e) = service.maybe_compact(&mut messages, llm.as_ref()).await {
                    warn!("Compaction failed: {}", e);
                }
            }

            // Stream-first: try streaming, fall back to complete() if unsupported or errors.
            let mut turn_response: Option<LlmResponse> = None;
            let mut prompt_tokens = 0u32;
            let mut completion_tokens = 0u32;

            match llm.stream(&messages, &tools) {
                Ok(mut stream_rx) => {
                    prompt_tokens =
                        crate::agent::compaction_service::CompactionService::estimate_tokens(
                            &messages,
                        ) as u32;
                    // Send final content start notification
                    self.send_wire_message(WireMessage::ContentPart {
                        text: String::new(),
                    });
                    let mut accumulated = String::new();
                    let mut tool_calls: Vec<ToolCall> = Vec::new();
                    while let Some(chunk_result) = stream_rx.recv().await {
                        match chunk_result {
                            Ok(delta) => {
                                if let Some(content) = delta.content {
                                    accumulated.push_str(&content);
                                    on_chunk(&content);
                                    self.send_wire_message(WireMessage::ContentPart {
                                        text: content,
                                    });
                                }
                                for call in delta.tool_calls {
                                    tool_calls.push(call);
                                }
                            }
                            Err(e) => {
                                warn!("Stream error: {}, falling back to complete()", e);
                                accumulated.clear();
                                tool_calls.clear();
                                break;
                            }
                        }
                    }
                    completion_tokens = accumulated.len().div_ceil(4) as u32;
                    turn_response = Some(LlmResponse {
                        content: accumulated,
                        tool_calls,
                        is_complete: true,
                    });
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
                Some(r) => r,
                None => {
                    prompt_tokens =
                        crate::agent::compaction_service::CompactionService::estimate_tokens(
                            &messages,
                        ) as u32;
                    let r = llm.complete(&messages, &tools).await?;
                    completion_tokens = r.content.len().div_ceil(4) as u32;
                    r
                }
            };

            self.accumulate_usage(prompt_tokens, completion_tokens);

            if response.tool_calls.is_empty() {
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

                final_response = response.content;
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
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None,
            });

            for tool_call in &response.tool_calls {
                // Send StepBegin message
                self.send_wire_message(WireMessage::StepBegin {
                    tool_name: tool_call.function.name.clone(),
                });

                // Send ToolCall message
                let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                self.send_wire_message(WireMessage::ToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    arguments: args,
                });

                let result = self.execute_tool_call(tool_call).await;
                let result_content = match result {
                    Ok(value) => value.to_string(),
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                };

                // Send ToolResult message
                self.send_wire_message(WireMessage::ToolResult {
                    id: tool_call.id.clone(),
                    result: result_content.clone(),
                });

                messages.push(Message::tool(&tool_call.id, result_content));
            }
        }

        // Send TurnEnd message
        self.send_wire_message(WireMessage::TurnEnd);

        // Send usage report
        let usage = self.get_session_usage();
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
                    info!("Memory ticker triggered, compilation results: {:?}", results);
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

fn format_plan_results(results: &[crate::agent::PlanResult]) -> String {
    if results.is_empty() {
        return "Plan executed with no steps.".to_string();
    }
    let mut lines = vec!["Plan execution results:".to_string()];
    for r in results {
        let icon = if r.success { "✅" } else { "❌" };
        lines.push(format!("{} {}: {}", icon, r.step_id, r.output));
    }
    lines.join("\n")
}
