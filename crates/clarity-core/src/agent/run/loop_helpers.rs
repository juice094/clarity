//! Helper functions for agent execution loops.

use crate::error::AgentError;
use crate::llm::api::{Message, MessageRole};

/// Check if any message in the conversation contains image/vision content.
pub(crate) fn messages_contain_vision(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| m.content.contains("<image>") || m.content.contains("!["))
}

/// Check if an LLM error is caused by context window overflow.
pub(crate) fn is_context_overflow_error(err: &AgentError) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("context length exceeded")
        || msg.contains("context window")
        || msg.contains("too many tokens")
        || msg.contains("maximum context length")
        || msg.contains("token limit")
        || msg.contains("contextoverflow")
}

/// Fast-trim tool results from messages to recover from context window overflow.
/// Removes the oldest non-system messages in pairs (assistant tool_call + tool result)
/// until under the budget or no more trimmable messages remain.
pub(crate) fn fast_trim_tool_results(messages: &mut Vec<Message>) {
    // Identify indices of tool-result messages (role=Tool).
    let tool_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == MessageRole::Tool)
        .map(|(i, _)| i)
        .collect();

    if tool_indices.is_empty() {
        // No tool results to trim; remove oldest user/assistant pair as last resort.
        if messages.len() > 2 {
            messages.remove(1); // oldest non-system
        }
        return;
    }

    // Remove up to half of the tool results (oldest first), keeping the most recent.
    let to_remove = (tool_indices.len() / 2).max(1);
    for &idx in tool_indices.iter().take(to_remove) {
        // Also remove the preceding assistant message that issued the tool_call,
        // if it exists and its tool_calls reference this result.
        if idx > 0 && messages[idx - 1].role == MessageRole::Assistant {
            messages.remove(idx - 1);
            // After removing idx-1, the tool result is now at idx-1.
            if idx - 1 < messages.len() && messages[idx - 1].role == MessageRole::Tool {
                messages.remove(idx - 1);
            }
        } else {
            messages.remove(idx);
        }
    }
}

/// Scrub sensitive credentials from tool output before injecting into LLM context.
/// Prevents accidental leakage of API keys, tokens, passwords, and Bearer headers.
pub(crate) fn scrub_credentials(input: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static RE_KEYVAL: OnceLock<Regex> = OnceLock::new();
    static RE_BEARER: OnceLock<Regex> = OnceLock::new();
    static RE_SK: OnceLock<Regex> = OnceLock::new();

    let re_keyval = RE_KEYVAL.get_or_init(|| {
        Regex::new(
            r#"(?i)(api[_-]?key|token|secret|password|passwd|pwd)\s*[:=]\s*["']?[^\s"']+["']?"#,
        )
        .unwrap()
    });
    let re_bearer = RE_BEARER.get_or_init(|| Regex::new(r"Bearer\s+[\w\-]+").unwrap());
    let re_sk = RE_SK.get_or_init(|| Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap());

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
    result = re_bearer
        .replace_all(&result, "Bearer [REDACTED]")
        .to_string();
    result = re_sk.replace_all(&result, "[REDACTED]").to_string();

    result
}

// B3: Uses `crate::types::PlanResult` directly since the type was moved out of
// the `agent` module to reduce coupling.
pub(crate) fn format_plan_results(results: &[crate::types::PlanResult]) -> String {
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

use crate::agent::Agent;
use clarity_wire::WireMessage;
use tracing::{debug, info, warn};

impl Agent {
    /// Build system prompt and append relevant memories if available.
    pub(crate) async fn build_system_prompt_with_memory(&self, query: &str) -> String {
        let mut prompt = self.build_system_prompt();
        if let Some(ref store) = self.memory_store() {
            if let Ok(memories) = store.search(query, 5).await {
                if !memories.is_empty() {
                    let text = memories.iter().map(|m| format!("- {}", m.content)).collect::<Vec<_>>().join("\n");
                    prompt.push_str(&format!("\n\n# Relevant Memories\n{}\n", text));
                }
            }
        }
        prompt
    }

    /// Finish turn, run delivery hooks, and emit usage wire message.
    pub(crate) async fn finish_and_deliver(
        &self,
        mut response: String,
        tool_names: &[String],
        usage: crate::agent::TokenUsage,
    ) -> Result<String, AgentError> {
        self.finish_turn();
        let tier = crate::hooks::classify_delivery_tier(tool_names);
        if let Some(ref hooks) = self.hook_registry {
            response = hooks.run_pre_delivery(&response, tier).await?;
        }
        self.send_wire_message(WireMessage::TurnEnd);
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });
        Ok(response)
    }

    /// Persist interaction to memory store and trigger memory ticker.
    pub(crate) async fn persist_turn_memory(&self, query: &str, response: &str, completed: bool) {
        let content = if completed {
            format!("User: {}\nAssistant: {}", query, response)
        } else {
            format!("User: {}\nAssistant: [max iterations reached] {}", query, response)
        };
        self.store_conversation_memory(content.clone()).await;
        self.maybe_extract_memories(content);
        if let Some(ref ticker) = self.memory_ticker {
            match ticker.notify_turn_and_wait("default").await {
                Some(Ok(r)) => info!("Memory ticker triggered, compilation results: {:?}", r),
                Some(Err(e)) => warn!("Memory ticker compilation failed: {}", e),
                None => debug!("Memory ticker not triggered yet"),
            }
        }
    }
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
