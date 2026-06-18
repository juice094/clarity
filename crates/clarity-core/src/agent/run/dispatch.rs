//! Tool call dispatch and compaction orchestration.

use crate::agent::Agent;
use crate::error::AgentError;
use crate::types::ToolCall;
use clarity_llm::api::{LlmProvider, Message};
use clarity_wire::WireMessage;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use super::loop_helpers::scrub_credentials;
use crate::agent::loop_detector::LoopDetection;
use tracing::{info, warn};

/// Output of dispatching a batch of tool calls.
pub(crate) struct DispatchOutput {
    pub tool_names: Vec<String>,
    /// If `ask_user` was invoked successfully, the question text to display.
    pub ask_user_question: Option<String>,
}

impl Agent {
    /// Run proactive compaction and threshold-based compaction if needed.
    pub(crate) async fn maybe_compact_turn(
        &self,
        messages: &mut Vec<Message>,
        llm: &dyn LlmProvider,
    ) {
        let mut did_compact = false;
        if let Some(ref service) = self.compaction_service {
            if service.needs_compaction(messages) {
                self.send_wire_message(WireMessage::ViewStateUpdate {
                    turn_id: String::new(),
                    turn: Some(clarity_wire::TurnState::Compacting),
                });
                self.send_wire_message(WireMessage::StatusUpdate {
                    turn_id: String::new(),
                    message: "Compacting context...".to_string(),
                });
                self.send_wire_message(WireMessage::CompactionBegin {
                    turn_id: String::new(),
                });
                did_compact = true;
            }
            if let Err(e) = service.maybe_compact(messages, llm).await {
                warn!("Compaction failed: {}", e);
            }
        }
        if self.should_compact(messages).await {
            if !did_compact {
                self.send_wire_message(WireMessage::CompactionBegin {
                    turn_id: String::new(),
                });
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
            self.send_wire_message(WireMessage::CompactionEnd {
                turn_id: String::new(),
            });
            self.send_wire_message(WireMessage::ViewStateUpdate {
                turn_id: String::new(),
                turn: Some(clarity_wire::TurnState::Loading),
            });
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
    pub(crate) async fn dispatch_tool_calls(
        &self,
        tool_calls: &[ToolCall],
        messages: &mut Vec<Message>,
        cancel_token: &CancellationToken,
    ) -> Result<DispatchOutput, AgentError> {
        if cancel_token.is_cancelled() {
            tracing::warn!("dispatch_tool_calls cancelled before starting");
            return Err(AgentError::Cancelled);
        }
        if !tool_calls.is_empty() {
            self.send_wire_message(WireMessage::StatusUpdate {
                turn_id: String::new(),
                message: format!("Executing {} tool(s)...", tool_calls.len()),
            });
        }
        let mut tool_names = Vec::new();
        let mut ask_user_question: Option<String> = None;
        let mut modified_tool_calls: Vec<ToolCall> = Vec::new();
        let mut execute_flags: Vec<bool> = Vec::new();
        #[allow(clippy::type_complexity)]
        let mut futures: Vec<
            Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<serde_json::Value, crate::error::ToolError>,
                        > + Send,
                >,
            >,
        > = Vec::new();

        // Phase 1: run before_tool_call hooks, emit begin messages, and start all tool executions concurrently.
        for tool_call in tool_calls {
            let mut tc = tool_call.clone();
            let hooks_opt = {
                let inner = self.inner.read();
                inner.hook_registry.clone()
            };
            let should_execute = if let Some(hooks) = hooks_opt {
                let registry = hooks.read().await;
                match registry.before_tool_call(&mut tc).await {
                    crate::agent::hooks::HookResult::Cancel(e) => {
                        let err = crate::error::ToolError::execution_failed(e.to_string());
                        futures.push(Box::pin(async move { Err(err) }));
                        false
                    }
                    crate::agent::hooks::HookResult::Replace(_) => true,
                    crate::agent::hooks::HookResult::Continue => true,
                }
            } else {
                true
            };

            tool_names.push(tc.function.name.clone());
            self.send_wire_message(WireMessage::StepBegin {
                turn_id: String::new(),
                tool_name: tc.function.name.clone(),
            });

            let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));
            self.send_wire_message(WireMessage::ToolCall {
                turn_id: String::new(),
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                arguments: args,
            });

            modified_tool_calls.push(tc);
            execute_flags.push(should_execute);
        }

        for (tc, flag) in modified_tool_calls.iter().zip(execute_flags.iter()) {
            if *flag {
                futures.push(Box::pin(self.execute_tool_call(tc)));
            }
        }

        // Phase 2: await all results sequentially to avoid concurrent approval deadlock.
        let mut results = Vec::new();
        for future in futures {
            if cancel_token.is_cancelled() {
                tracing::warn!("dispatch_tool_calls cancelled while awaiting results");
                return Err(AgentError::Cancelled);
            }
            results.push(future.await);
        }

        // Phase 3: emit results in original order, append to messages, and
        // detect non-recoverable failures.
        let mut fatal: Option<(String, String)> = None;

        for (tool_call, result) in modified_tool_calls.iter().zip(results) {
            let sanitized = result.map_err(|e| e.sanitize_paths());
            let mut result_value = match &sanitized {
                Ok(v) => v.clone(),
                Err(e) => serde_json::json!({"error": e.to_string()}),
            };

            let hooks_opt = {
                let inner = self.inner.read();
                inner.hook_registry.clone()
            };
            if let Some(hooks) = hooks_opt {
                let registry = hooks.read().await;
                registry.after_tool_call(tool_call, &mut result_value).await;
            }

            let result_content = result_value.to_string();

            self.send_wire_message(WireMessage::ToolResult {
                turn_id: String::new(),
                id: tool_call.id.clone(),
                result: result_content.clone(),
            });

            let scrubbed = scrub_credentials(&result_content);
            let wrapped = format!(
                "<tool_result name=\"{}\">{}</tool_result>",
                tool_call.function.name, scrubbed
            );
            messages.push(Message::tool(&tool_call.id, wrapped));

            if let Err(ref e) = sanitized {
                if !e.is_recoverable() {
                    fatal = Some((tool_call.function.name.clone(), e.to_string()));
                } else {
                    let mut inner = self.inner.write();
                    let ctx = match inner.turn_context.as_mut() {
                        Some(ctx) => ctx,
                        None => {
                            tracing::error!(
                                "turn_context missing during dispatch_tool_calls; treating as stalled"
                            );
                            return Err(AgentError::Stalled);
                        }
                    };
                    let count = ctx
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
            } else if tool_call.function.name == "ask_user" {
                // Detect successful ask_user invocation — pause the loop to wait for user reply.
                if result_value.get("asked").and_then(|v| v.as_bool()) == Some(true) {
                    ask_user_question = result_value
                        .get("question")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }

            // Check for repetitive output loops (only if no fatal error already).
            if fatal.is_none() {
                let mut inner = self.inner.write();
                let ctx = match inner.turn_context.as_mut() {
                    Some(ctx) => ctx,
                    None => {
                        tracing::error!(
                            "turn_context missing during dispatch_tool_calls; treating as stalled"
                        );
                        return Err(AgentError::Stalled);
                    }
                };
                match ctx.loop_detector.record(
                    &tool_call.function.name,
                    &tool_call.function.arguments,
                    &result_content,
                ) {
                    LoopDetection::Warning {
                        tool_name: _,
                        message,
                    } => {
                        messages.push(Message::system(message));
                    }
                    LoopDetection::Break { tool_name, message } => {
                        fatal = Some((tool_name, message));
                    }
                    LoopDetection::Ok => {}
                }
            }
        }

        if let Some((tool_name, error)) = fatal {
            return Err(AgentError::ToolExecutionFailed(tool_name, error));
        }

        Ok(DispatchOutput {
            tool_names,
            ask_user_question,
        })
    }
}
