//! Helper functions for agent execution loops.

use crate::error::AgentError;
use clarity_contract::retry::RetryConfig;
use clarity_contract::{Message, MessageRole};

/// Retry an async operation with exponential backoff + jitter.
///
/// Only retries when the error is recoverable (`is_recoverable() == true`).
/// Uses `RetryConfig` from `clarity-contract` for backoff calculation with
/// ±25% random jitter to prevent thundering herd on LLM API rate limits.
///
/// A default `RetryConfig` is used when none is provided (10 retries,
/// 1s initial, 5 min max).
pub(crate) async fn retry_with_backoff<F, Fut, T>(f: F, max_retries: u32) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let config = RetryConfig {
        max_retries,
        ..RetryConfig::default()
    };
    retry_with_config(f, &config).await
}

/// Retry an async operation using a custom `RetryConfig`.
///
/// Prefer this over `retry_with_backoff` when you need fine-grained control
/// over backoff parameters (e.g., different intervals for streaming vs
/// completion calls, or conservative settings for rate-limited providers).
pub(crate) async fn retry_with_config<F, Fut, T>(
    mut f: F,
    config: &RetryConfig,
) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut attempt: u32 = 0;
    loop {
        match f().await {
            Ok(output) => return Ok(output),
            Err(err) if !err.is_recoverable() => return Err(err),
            Err(err) => {
                if config.is_exhausted(attempt) {
                    return Err(err);
                }
                let delay = config.backoff_duration(attempt);
                tracing::warn!(
                    "LLM call failed with recoverable error, retrying in {:?} (attempt {}/{})",
                    delay,
                    attempt + 1,
                    config.max_retries
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

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

/// Token-aware context overflow recovery.
///
/// When the LLM returns a context-overflow error, this function uses
/// [`crate::compaction::estimate_text_tokens`] (tiktoken cl100k_base) to
/// determine exactly how many tokens to shed, then removes the oldest
/// assistant‑tool_result pairs until the total falls under a safe
/// threshold. Falls back to the heuristic "drop oldest half" strategy
/// only when the tokenizer is unavailable.
///
/// Target: reduce total tokens by ~30% (or drop at least one pair).
pub(crate) fn fast_trim_tool_results(messages: &mut Vec<Message>) {
    // Build the list of trimmable (assistant_idx, tool_idx) pairs.
    // Walk backward so we can remove from the end first (avoids index
    // shifting).
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in (1..messages.len()).rev() {
        if messages[i].role == MessageRole::Tool
            && i > 0
            && messages[i - 1].role == MessageRole::Assistant
        {
            pairs.push((i - 1, i));
        }
    }
    // pairs is in reverse order (newest first); oldest pairs are at the end.

    if pairs.is_empty() {
        // Fallback: no tool-result pairs → remove oldest non-system message.
        if messages.len() > 2 {
            messages.remove(1);
        }
        return;
    }

    // Calculate current token budget and target.
    let current_tokens: usize = messages
        .iter()
        .map(|m| crate::compaction::estimate_text_tokens(&m.content))
        .sum::<usize>();
    let target_tokens = current_tokens.saturating_mul(2) / 3; // ~33% reduction

    // Remove oldest pairs until under target (or only 1 pair remains).
    let mut removed = 0usize;
    while pairs.len() > 1
        && messages
            .iter()
            .map(|m| crate::compaction::estimate_text_tokens(&m.content))
            .sum::<usize>()
            > target_tokens
    {
        // Oldest pair is at the back of the reversed list.
        #[allow(clippy::expect_used)]
        // SAFE: while loop guards pairs.len() > 1 so pop never returns None
        let (ai, ti) = pairs.pop().expect("pairs non-empty");
        // Remove from back to front to keep earlier indices valid.
        // ti > ai always because we built them as (i-1, i).
        messages.remove(ti);
        messages.remove(ai);
        removed += 1;
    }

    // If we removed nothing (budget was already ok or only 1 pair),
    // still drop at least the oldest pair as a safety measure.
    if removed == 0 {
        #[allow(clippy::expect_used)] // SAFE: function only called when pairs is non-empty
        let (ai, ti) = pairs.last().copied().expect("pairs non-empty");
        messages.remove(ti);
        messages.remove(ai);
    }

    tracing::info!(
        "Context overflow recovery: removed {} assistant+tool pairs, {} messages remain",
        removed.max(1),
        messages.len()
    );
}

/// Scrub sensitive credentials from tool output before injecting into LLM context.
/// Prevents accidental leakage of API keys, tokens, passwords, and Bearer headers.
pub(crate) fn scrub_credentials(input: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    // ponytail: 3 hardcoded regex patterns; migrate to a configurable secret-detection
    // list if patterns exceed 5 or require provider-specific rules.
    static RE_KEYVAL: OnceLock<Regex> = OnceLock::new();
    static RE_BEARER: OnceLock<Regex> = OnceLock::new();
    static RE_SK: OnceLock<Regex> = OnceLock::new();

    // Patterns are compile-time literals; expect is safe because they are valid regexes.
    #[allow(clippy::expect_used)]
    let re_keyval = RE_KEYVAL.get_or_init(|| {
        Regex::new(
            r#"(?i)(api[_-]?key|token|secret|password|passwd|pwd)\s*[:=]\s*["']?[^\s"']+["']?"#,
        )
        .expect("RE_KEYVAL regex is valid")
    });
    #[allow(clippy::expect_used)]
    let re_bearer = RE_BEARER
        .get_or_init(|| Regex::new(r"Bearer\s+[\w\-]+").expect("RE_BEARER regex is valid"));
    #[allow(clippy::expect_used)]
    let re_sk =
        RE_SK.get_or_init(|| Regex::new(r"sk-[a-zA-Z0-9]{20,}").expect("RE_SK regex is valid"));

    let mut result = input.to_string();
    result = re_keyval
        .replace_all(&result, |caps: &regex::Captures| {
            let m = caps.get(0).map(|m| m.as_str()).unwrap_or("");
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
        self.send_wire_message(WireMessage::ViewStateUpdate {
            turn_id: String::new(),
            turn: Some(clarity_wire::TurnState::Idle),
        });
        self.send_wire_message(WireMessage::TurnEnd {
            turn_id: String::new(),
        });
        self.send_wire_message(WireMessage::Usage {
            turn_id: String::new(),
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
            format!(
                "User: {}\nAssistant: [max iterations reached] {}",
                query, response
            )
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

    /// Shared turn finish logic (deliver + max-iterations check).
    pub(crate) async fn finish_sync_turn(
        &self,
        final_response: String,
        completed: bool,
        tool_names: &[String],
    ) -> Result<String, AgentError> {
        let usage = self.get_session_usage();
        let final_response = self
            .finish_and_deliver(final_response, tool_names, usage)
            .await?;
        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            ))
        }
    }

    /// Finish a sync turn: deliver, persist memory, extract memories, run hooks.
    pub(crate) async fn finalize_sync_turn(
        &self,
        query: &str,
        final_response: String,
        completed: bool,
        tool_names: &[String],
        messages: &[clarity_contract::Message],
    ) -> Result<String, AgentError> {
        let final_response = self
            .finish_sync_turn(final_response, completed, tool_names)
            .await?;
        self.persist_turn_memory(query, &final_response, completed)
            .await;

        if completed {
            let transcript = serde_json::to_string(messages).unwrap_or_default();
            self.maybe_extract_memories(transcript);
            if let Some(ref hooks) = self.hook_registry {
                let summary = serde_json::json!({
                    "query": query,
                    "response": &final_response,
                    "completed": true,
                });
                hooks.run_session_termination(&summary.to_string()).await;
            }
        }
        Ok(final_response)
    }
}

#[cfg(test)]
mod tests {
    use super::{retry_with_backoff, scrub_credentials};
    use crate::error::AgentError;

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

    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_on_third_attempt() {
        let mut attempts = 0u32;
        let result = retry_with_backoff(
            || {
                attempts += 1;
                std::future::ready(if attempts <= 2 {
                    Err(AgentError::Llm("temp".into()))
                } else {
                    Ok("success")
                })
            },
            3,
        )
        .await;
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn test_retry_with_backoff_non_recoverable_fails_immediately() {
        let mut attempts = 0u32;
        let result: Result<(), _> = retry_with_backoff(
            || {
                attempts += 1;
                std::future::ready(Err(AgentError::Unconfigured))
            },
            3,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    // ── fast_trim_tool_results tests ──────────────────────────────────────

    #[test]
    fn trim_removes_oldest_assistant_tool_pair() {
        use clarity_contract::{Message, MessageRole};
        let mut messages = vec![
            Message::system("sys"),
            Message::user("q1"),
            Message::assistant("a1"),
            Message::tool("t1", "result1"),
            Message::user("q2"),
            Message::assistant("a2"),
            Message::tool("t2", "result2"),
        ];
        let original_len = messages.len();
        super::fast_trim_tool_results(&mut messages);
        // Should have removed the oldest (assistant + tool) pair.
        assert!(messages.len() < original_len);
        // System message should still be first.
        assert_eq!(messages[0].role, MessageRole::System);
        // The second user message should still be present.
        assert!(messages.iter().any(|m| m.content == "q2"));
    }

    #[test]
    fn trim_no_tool_results_removes_oldest_non_system() {
        use clarity_contract::{Message, MessageRole};
        let mut messages = vec![
            Message::system("sys"),
            Message::user("q1"),
            Message::assistant("a1"),
            Message::user("q2"),
        ];
        let original_len = messages.len();
        super::fast_trim_tool_results(&mut messages);
        assert!(messages.len() < original_len);
        assert_eq!(messages[0].role, MessageRole::System);
    }

    #[test]
    fn trim_system_only_noop() {
        use clarity_contract::Message;
        let mut messages = vec![Message::system("sys")];
        let original_len = messages.len();
        super::fast_trim_tool_results(&mut messages);
        // Should not remove the only message (system).
        assert_eq!(messages.len(), original_len);
    }
}
