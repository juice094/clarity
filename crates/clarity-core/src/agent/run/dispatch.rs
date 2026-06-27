//! Tool call dispatch and compaction orchestration.

use crate::agent::Agent;
use crate::error::{AgentError, ToolError};
use crate::types::ToolCall;
use clarity_contract::{LlmProvider, Message};
use clarity_wire::WireMessage;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::loop_helpers::scrub_credentials;
use crate::agent::loop_detector::LoopDetection;
use tracing::{info, warn};

/// Maximum suffix length for the truncation notice.
const TRUNCATION_SUFFIX_MAX: usize = 200;

/// Truncate a tool result for LLM context injection.
///
/// When the result exceeds `max_chars`, the content is truncated at a
/// newline boundary near the limit, and a note is appended describing
/// how much was dropped and suggesting alternative tools for retrieval.
///
/// Wire-level delivery to frontends is unaffected — only the LLM sees
/// the truncation.
fn truncate_for_context(result: &str, tool_name: &str, max_chars: usize) -> String {
    if result.len() <= max_chars {
        return result.to_string();
    }

    // Try to truncate at a newline boundary to avoid splitting mid-line.
    let cutoff = max_chars - TRUNCATION_SUFFIX_MAX;
    let truncation_point = result[..cutoff].rfind('\n').unwrap_or(cutoff);

    let truncated = &result[..truncation_point];
    let dropped_chars = result.len() - truncation_point;
    let dropped_lines = result[truncation_point..].lines().count();

    let note = format!(
        "\n\n[Output truncated: {} characters / ~{} lines dropped. \
         Use '{}' with offset/limit (for files) or more specific parameters to retrieve the omitted portion.]",
        dropped_chars, dropped_lines, tool_name
    );

    let mut output = String::with_capacity(truncated.len() + note.len());
    output.push_str(truncated);
    output.push_str(&note);
    output
}

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
    /// `AgentError::ToolExecutionFailed` immediately after the batch completes,
    /// preventing the LLM from entering an infinite retry loop.
    ///
    /// R1: Recoverable errors (IoError/Timeout/Unavailable) are intentionally NOT
    /// fatal on first failure — the LLM may retry with a different strategy.
    /// To prevent infinite loops, a per-turn circuit breaker upgrades to fatal
    /// after the SAME tool fails recoverably 3 times in a single turn.
    ///
    /// ponytail: executes tool calls sequentially. Concurrent execution was
    /// attempted but removed to avoid approval-deadlock races; revisit only if
    /// batch latency becomes a measured bottleneck and a concurrency policy is
    /// designed.
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

        // Process each tool call sequentially: hooks -> begin -> execute -> result.
        let mut fatal: Option<(String, String)> = None;

        for tool_call in tool_calls {
            if cancel_token.is_cancelled() {
                tracing::warn!("dispatch_tool_calls cancelled while processing tool calls");
                return Err(AgentError::Cancelled);
            }

            let mut tc = tool_call.clone();
            let cancel_error: Option<ToolError> = {
                let hooks_opt = {
                    let inner = self.inner.read();
                    inner.hook_registry.clone()
                };
                if let Some(hooks) = hooks_opt {
                    let registry = hooks.read().await;
                    match registry.before_tool_call(&mut tc).await {
                        crate::agent::hooks::HookResult::Cancel(e) => {
                            Some(ToolError::execution_failed(e.to_string()))
                        }
                        crate::agent::hooks::HookResult::Replace(_) => None,
                        crate::agent::hooks::HookResult::Continue => None,
                    }
                } else {
                    None
                }
            };

            tool_names.push(tc.function.name.clone());
            self.send_wire_message(WireMessage::StepBegin {
                turn_id: String::new(),
                tool_name: tc.function.name.clone(),
            });

            let args: Value = serde_json::from_str(&tc.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));
            self.send_wire_message(WireMessage::ToolCall {
                turn_id: String::new(),
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                arguments: args,
            });

            let result: Result<Value, ToolError> = match cancel_error {
                Some(err) => Err(err),
                None => self.execute_tool_call(&tc).await,
            };
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

            // Format the display string server-side via Tool::format_output().
            let display_result = match &sanitized {
                Ok(v) => self
                    .registry
                    .get(&tc.function.name)
                    .ok()
                    .flatten()
                    .map(|tool| tool.format_output(v)),
                Err(_) => None, // errors don't get formatted
            };

            self.send_wire_message(WireMessage::ToolResult {
                turn_id: String::new(),
                id: tool_call.id.clone(),
                result: result_content.clone(),
                display_result,
            });

            let scrubbed = scrub_credentials(&result_content);
            // Truncate large tool results before injecting into LLM context.
            // Frontends receive the full result via WireMessage::ToolResult above.
            // Per-tool limit takes precedence over the global AgentConfig default.
            let max_chars = self
                .registry
                .get(&tc.function.name)
                .ok()
                .flatten()
                .and_then(|tool| tool.max_output_chars())
                .unwrap_or(self.config.max_tool_result_chars);
            let context_content =
                truncate_for_context(&scrubbed, tool_call.function.name.as_str(), max_chars);
            let wrapped = format!(
                "<tool_result name=\"{}\">{}</tool_result>",
                tool_call.function.name, context_content
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

#[cfg(test)]
mod tests {
    use super::truncate_for_context;

    #[test]
    fn truncate_short_result_passes_through() {
        let result = "short output";
        let truncated = truncate_for_context(result, "read", 30_000);
        assert_eq!(truncated, result);
    }

    #[test]
    fn truncate_exact_boundary_passes_through() {
        let result = "a".repeat(200);
        let truncated = truncate_for_context(&result, "read", 200);
        assert_eq!(truncated, result);
    }

    #[test]
    fn truncate_large_result_adds_note() {
        let lines: Vec<String> = (0..5000).map(|i| format!("line {}", i)).collect();
        let result = lines.join("\n");
        let truncated = truncate_for_context(&result, "grep", 1000);
        assert!(truncated.len() <= 1000 + 50); // padding for the note
        assert!(truncated.contains("[Output truncated:"));
        assert!(truncated.contains("grep"));
        assert!(!truncated.contains("line 4000")); // later lines should be cut
    }

    #[test]
    fn truncate_no_newline_boundary_falls_back_to_cutoff() {
        let result = "x".repeat(5000); // no newlines
        let truncated = truncate_for_context(&result, "read", 1000);
        assert!(truncated.len() <= 1000 + 50);
        assert!(truncated.contains("[Output truncated:"));
    }

    #[test]
    fn truncate_newline_boundary_splits_nicely() {
        let mut result = String::new();
        for i in 0..100 {
            result.push_str(&format!("Line {:03}: some content here\n", i));
        }
        let truncated = truncate_for_context(&result, "read", 500);
        // Should end with a complete line + truncation note
        assert!(truncated.ends_with("portion.]"));
        assert!(!truncated.ends_with("mid-"));
    }
}
