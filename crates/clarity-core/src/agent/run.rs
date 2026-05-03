//! Agent execution loops: synchronous, streaming, and shared core.

use super::Agent;
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, LlmResponse, Message, MessageRole};
use crate::types::ToolCall;
use clarity_wire::WireMessage;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Scrub sensitive credentials from tool output before injecting into LLM context.
/// Prevents accidental leakage of API keys, tokens, passwords, and Bearer headers.
fn scrub_credentials(input: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static RE_KEYVAL: OnceLock<Regex> = OnceLock::new();
    static RE_BEARER: OnceLock<Regex> = OnceLock::new();
    static RE_SK: OnceLock<Regex> = OnceLock::new();

    let re_keyval = RE_KEYVAL.get_or_init(|| {
        Regex::new(r#"(?i)(api[_-]?key|token|secret|password|passwd|pwd)\s*[:=]\s*["']?[^\s"']+["']?"#)
            .unwrap()
    });
    let re_bearer = RE_BEARER.get_or_init(|| {
        Regex::new(r"Bearer\s+[\w\-]+").unwrap()
    });
    let re_sk = RE_SK.get_or_init(|| {
        Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap()
    });

    let mut result = input.to_string();
    result = re_keyval
        .replace_all(&result, |caps: &regex::Captures| {
            let m = caps.get(0).unwrap().as_str();
            if let Some(eq) = m.find('=') {
                format!("{}=[REDACTED]", &m[..eq])
            } else if let Some(colon) = m.find(':') {
                format!("{}: [REDACTED]", &m[..colon])
            } else {
                "[REDACTED]".to_string()
            }
        })
        .to_string();
    result = re_bearer.replace_all(&result, "Bearer [REDACTED]").to_string();
    result = re_sk.replace_all(&result, "[REDACTED]").to_string();

    result
}

impl Agent {
    /// Run proactive compaction and threshold-based compaction if needed.
    async fn maybe_compact_turn(&self, messages: &mut Vec<Message>, llm: &dyn LlmProvider) {
        let mut did_compact = false;
        if let Some(ref service) = self.compaction_service {
            if service.needs_compaction(messages) {
                self.send_wire_message(WireMessage::CompactionBegin);
                did_compact = true;
            }
            if let Err(e) = service.maybe_compact(messages, llm).await {
                warn!("Compaction failed: {}", e);
            }
        }
        if self.should_compact(messages).await {
            if !did_compact {
                self.send_wire_message(WireMessage::CompactionBegin);
                did_compact = true;
            }
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
        if did_compact {
            self.send_wire_message(WireMessage::CompactionEnd);
        }
    }

    /// Execute a batch of tool calls, sending wire lifecycle events and appending
    /// results to `messages`. Returns the ordered list of tool names for telemetry.
    ///
    /// If any tool fails with a **non-recoverable** error, the function returns
    /// `AgentError::ToolExecutionFailed` immediately after all concurrent calls
    /// complete, preventing the LLM from entering an infinite retry loop.
    ///
    /// R1: Recoverable errors (IoError/Timeout/Unavailable) are intentionally NOT
    /// fatal on first failure — the LLM may retry with a different strategy.
    /// To prevent infinite loops, a per-turn circuit breaker upgrades to fatal
    /// after the SAME tool fails recoverably 3 times in a single turn.
    async fn dispatch_tool_calls(
        &self,
        tool_calls: &[ToolCall],
        messages: &mut Vec<Message>,
    ) -> Result<Vec<String>, AgentError> {
        let mut tool_names = Vec::new();
        let mut futures = Vec::new();

        // Phase 1: emit begin messages and start all tool executions concurrently.
        for tool_call in tool_calls {
            tool_names.push(tool_call.function.name.clone());
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

            futures.push(self.execute_tool_call(tool_call));
        }

        // Phase 2: await all results sequentially to avoid concurrent approval deadlock.
        let mut results = Vec::new();
        for future in futures {
            results.push(future.await);
        }

        // Phase 3: emit results in original order, append to messages, and
        // detect non-recoverable failures.
        let mut fatal: Option<(String, String)> = None;

        for (tool_call, result) in tool_calls.iter().zip(results.into_iter()) {
            let sanitized = result.map_err(|e| e.sanitize_paths());
            let result_content = match &sanitized {
                Ok(value) => value.to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            };

            self.send_wire_message(WireMessage::ToolResult {
                id: tool_call.id.clone(),
                result: result_content.clone(),
            });

            let scrubbed = scrub_credentials(&result_content);
            messages.push(Message::tool(&tool_call.id, scrubbed));

            if let Err(ref e) = sanitized {
                if !e.is_recoverable() {
                    fatal = Some((tool_call.function.name.clone(), e.to_string()));
                } else {
                    let mut inner = self.inner.write().unwrap();
                    let count = inner
                        .recoverable_failure_counts
                        .entry(tool_call.function.name.clone())
                        .or_insert(0);
                    *count += 1;
                    if *count >= 3 {
                        fatal = Some((
                            tool_call.function.name.clone(),
                            format!(
                                "Tool '{}' failed {} times (recoverable errors exhausted)",
                                tool_call.function.name, *count
                            ),
                        ));
                    }
                }
            }
        }

        if let Some((tool_name, error)) = fatal {
            return Err(AgentError::ToolExecutionFailed(tool_name, error));
        }

        Ok(tool_names)
    }

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

            let prompt_tokens =
                crate::agent::compaction_service::CompactionService::estimate_tokens(messages)
                    as u32;
            let response = tokio::time::timeout(
                tokio::time::Duration::from_secs(45),
                llm.complete(messages, tools),
            )
            .await
            .map_err(|_| AgentError::Llm("LLM request timed out after 45s".into()))??;
            // O4: Character-count/4 is a rough heuristic; replace with real tokenizer
            // count when clarity-core gains a tokenizer abstraction.
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

            tool_names.extend(
                self.dispatch_tool_calls(&response.tool_calls, messages)
                    .await?,
            );
        }

        Ok((final_response, completed, tool_names))
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
        self.ensure_initialized().await?;

        // Plan mode: bypass the ReAct loop and use plan-driven execution.
        // This avoids the LLM "thinking step-by-step" and instead runs a
        // pre-generated structured plan step-by-step.
        let mode = self.inner.read().unwrap().approval_mode;
        if mode == crate::approval::ApprovalMode::Plan {
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
        if let Some(ref registry) = self.skill_registry() {
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

        if let Some(ref store) = self.memory_store() {
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

        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        self.finish_turn();

        // Auto-classify delivery tier based on tools used this turn
        let tier = crate::hooks::classify_delivery_tier(&tool_names);

        // Run PreDeliveryHook pipeline if configured
        let final_response = if let Some(ref hooks) = self.hook_registry {
            hooks.run_pre_delivery(&final_response, tier).await?
        } else {
            final_response
        };

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
            let transcript = serde_json::to_string(&messages).unwrap_or_default();
            self.maybe_extract_memories(transcript);

            // Run SessionTerminationHook if configured
            if let Some(ref hooks) = self.hook_registry {
                let summary = serde_json::json!({
                    "query": query.as_ref(),
                    "response": &final_response,
                    "completed": true,
                });
                hooks.run_session_termination(&summary.to_string()).await;
            }

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
        self.ensure_initialized().await?;

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

        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        self.finish_turn();

        // Auto-classify delivery tier based on tools used this turn
        let tier = crate::hooks::classify_delivery_tier(&tool_names);

        // Run PreDeliveryHook pipeline if configured
        let final_response = if let Some(ref hooks) = self.hook_registry {
            hooks.run_pre_delivery(&final_response, tier).await?
        } else {
            final_response
        };

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
        self.ensure_initialized().await?;

        let base_system_prompt = self.build_system_prompt();
        let mut system_prompt = base_system_prompt;

        if let Some(ref store) = self.memory_store() {
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

        self.run_streaming_turn(messages, query.as_ref(), on_chunk).await
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
        let query_hint = messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        self.run_streaming_turn(messages, &query_hint, on_chunk).await
    }

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

            // Stream-first: try streaming, fall back to complete() if unsupported or errors.
            let mut turn_response: Option<LlmResponse> = None;
            let mut prompt_tokens = 0u32;
            let mut completion_tokens = 0u32;

            match llm.stream(messages, tools) {
                Ok(mut stream_rx) => {
                    prompt_tokens =
                        crate::agent::compaction_service::CompactionService::estimate_tokens(
                            messages,
                        ) as u32;
                    // Send final content start notification
                    self.send_wire_message(WireMessage::ContentPart {
                        text: String::new(),
                    });
                    let mut accumulated = String::new();
                    let mut tool_calls: Vec<ToolCall> = Vec::new();
                    let mut stream_ok = true;
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
                                stream_ok = false;
                                break;
                            }
                        }
                    }
                    if stream_ok {
                        completion_tokens = accumulated.len().div_ceil(4) as u32;
                        turn_response = Some(LlmResponse {
                            content: accumulated,
                            tool_calls,
                            is_complete: true,
                        });
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
                    debug!("Using streamed response (len={}, tool_calls={})", r.content.len(), r.tool_calls.len());
                    r
                }
                None => {
                    prompt_tokens =
                        crate::agent::compaction_service::CompactionService::estimate_tokens(
                            messages,
                        ) as u32;
                    let r = llm.complete(messages, tools).await?;
                    debug!("Using complete() response (len={}, tool_calls={})", r.content.len(), r.tool_calls.len());
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

                final_response = response.content.clone();
                if final_response.is_empty() {
                    warn!("Empty final response on iteration {} (was_streamed={})", iteration + 1, was_streamed);
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
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None,
            });

            if let Err(e) = self
                .dispatch_tool_calls(&response.tool_calls, messages)
                .await
            {
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
    async fn run_streaming_turn<F>(
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

        info!(
            "Starting streaming agent turn for query: {}",
            query_hint
        );

        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query_hint.to_string(),
        });

        let mut messages = messages;
        let mut on_chunk = on_chunk;
        let loop_result = self
            .run_streaming_loop(
                &mut messages,
                &tools,
                llm,
                &mut on_chunk,
                &cancel_token,
            )
            .await;
        self.finish_turn();

        let (final_response, completed) = loop_result?;

        // Teardown
        self.send_wire_message(WireMessage::TurnEnd);

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

// B3: Uses `crate::types::PlanResult` directly since the type was moved out of
// the `agent` module to reduce coupling.
fn format_plan_results(results: &[crate::types::PlanResult]) -> String {
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

#[cfg(test)]
mod tests {
    use super::scrub_credentials;

    #[test]
    fn test_scrub_api_key_colon() {
        let input = "Response: api_key: sk-test12345\nMore text";
        let out = scrub_credentials(input);
        assert!(out.contains("api_key: [REDACTED]"));
        assert!(!out.contains("sk-test12345"));
    }

    #[test]
    fn test_scrub_api_key_equals() {
        let input = "config = { api_key=secret_value, other = 1 }";
        let out = scrub_credentials(input);
        assert!(out.contains("api_key=[REDACTED]"));
        assert!(!out.contains("secret_value"));
    }

    #[test]
    fn test_scrub_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let out = scrub_credentials(input);
        assert!(out.contains("Bearer [REDACTED]"));
        assert!(!out.contains("eyJhbGci"));
    }

    #[test]
    fn test_scrub_sk_key() {
        let input = "key: sk-abcdefghijklmnopqrstuvwxyz123456";
        let out = scrub_credentials(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("sk-abcdefghijklmnopqrstuvwxyz123456"));
    }

    #[test]
    fn test_scrub_password() {
        let input = "login with password: my_secret_pass!";
        let out = scrub_credentials(input);
        assert!(out.contains("password: [REDACTED]"));
        assert!(!out.contains("my_secret_pass"));
    }

    #[test]
    fn test_scrub_no_false_positive() {
        let input = "The api_key field is required but not provided in this response.";
        let out = scrub_credentials(input);
        assert_eq!(out, input);
    }

    #[test]
    fn test_scrub_multiple_secrets() {
        let input = "api_key=abc123\nBearer xyz789\npassword: hunter2";
        let out = scrub_credentials(input);
        assert!(out.contains("api_key=[REDACTED]"));
        assert!(out.contains("Bearer [REDACTED]"));
        assert!(out.contains("password: [REDACTED]"));
        assert!(!out.contains("abc123"));
        assert!(!out.contains("xyz789"));
        assert!(!out.contains("hunter2"));
    }
}
