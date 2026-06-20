//! ReliableProvider — retry + fallback wrapper for LLM providers.
//!
//! Wraps one or more providers and handles:
//! - Exponential backoff retries for recoverable errors
//! - Rate-limit honouring (`Retry-After` header hints in error text)
//! - Context-window truncation and one-shot re-try
//! - Empty-completion re-roll
//! - Fallback chain when the primary provider fails

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::{
    AgentError, LlmProvider, LlmResponse, Message, MessageRole, ProviderCapabilities, StreamDelta,
};

/// Maximum delay between retries.
const MAX_BACKOFF_SECONDS: u64 = 10;

/// Classification of an LLM failure used to decide whether / how to retry.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ErrorClass {
    /// Generic transient error (network, timeout, 5xx, etc.)
    Retryable,
    /// Rate limited; includes an optional hint for how long to wait.
    RateLimited(Option<Duration>),
    /// Context window exceeded; truncate history and retry once.
    ContextWindowExceeded,
    /// Authentication / authorization error; do not retry the same key.
    Auth,
    /// Non-retryable error.
    NonRetryable,
}

/// Classification of a successful-but-useless response.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResponseClass {
    Normal,
    EmptyCompletion,
}

fn classify_error(err: &AgentError) -> ErrorClass {
    if !err.is_recoverable() {
        return ErrorClass::NonRetryable;
    }

    let msg = err.to_string().to_lowercase();

    if msg.contains("invalid api key")
        || msg.contains("unauthorized")
        || msg.contains("authentication")
        || msg.contains("auth error")
        || msg.contains("incorrect api key")
    {
        return ErrorClass::Auth;
    }

    if msg.contains("context length")
        || msg.contains("context window")
        || msg.contains("maximum context")
        || msg.contains("too many tokens")
        || msg.contains("token limit")
        || msg.contains("context overflow")
        || msg.contains("total message size")
        || msg.contains("exceeds limit")
    {
        return ErrorClass::ContextWindowExceeded;
    }

    // Try to extract a Retry-After hint from the error text.
    let retry_after = extract_retry_after_seconds(&msg).map(Duration::from_secs);
    if retry_after.is_some()
        || msg.contains("rate limit")
        || msg.contains("too many requests")
        || msg.contains("429")
    {
        return ErrorClass::RateLimited(retry_after);
    }

    ErrorClass::Retryable
}

fn extract_retry_after_seconds(text: &str) -> Option<u64> {
    // Simple heuristic: look for "retry-after: <digits>" anywhere in the text.
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find("retry-after") {
        let tail = &lower[pos..];
        if let Some(colon) = tail.find(':') {
            let after = &tail[colon + 1..];
            let num: String = after
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = num.parse::<u64>() {
                return Some(n);
            }
        }
    }
    None
}

fn classify_response(response: &LlmResponse) -> ResponseClass {
    let content_empty = response.content.trim().is_empty();
    let no_tool_calls = response.tool_calls.is_empty();
    if content_empty && no_tool_calls {
        ResponseClass::EmptyCompletion
    } else {
        ResponseClass::Normal
    }
}

/// Drop the oldest non-system message to make room in the context window.
fn truncate_oldest_non_system(messages: &[Message]) -> Vec<Message> {
    let system_count = messages
        .iter()
        .take_while(|m| m.role == MessageRole::System)
        .count();
    if messages.len() <= system_count + 1 {
        // Nothing to drop without losing the system prompt or the latest turn.
        return messages.to_vec();
    }
    let mut truncated = messages[..system_count].to_vec();
    truncated.extend_from_slice(&messages[system_count + 1..]);
    truncated
}

/// Retry an async operation with exponential backoff.
///
/// Only retries when the error is recoverable. Stops immediately on auth or
/// non-retryable errors. Respects `Retry-After` hints up to 10 s.
async fn retry_with_backoff<F, Fut, T>(mut f: F, max_retries: u32) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut retries = 0;
    loop {
        match f().await {
            Ok(output) => return Ok(output),
            Err(err) => {
                let class = classify_error(&err);
                match class {
                    ErrorClass::NonRetryable | ErrorClass::Auth => return Err(err),
                    ErrorClass::ContextWindowExceeded => {
                        // Context-window errors are handled one layer up by
                        // truncating history; don't burn retries here.
                        return Err(err);
                    }
                    ErrorClass::RateLimited(delay) => {
                        if retries >= max_retries {
                            return Err(err);
                        }
                        retries += 1;
                        let base = 2_u64.pow(retries - 1);
                        let delay_secs = delay
                            .map(|d| d.as_secs().min(MAX_BACKOFF_SECONDS))
                            .unwrap_or(base.min(MAX_BACKOFF_SECONDS));
                        tracing::warn!(
                            "LLM rate limited, retrying in {}s (attempt {}/{})",
                            delay_secs,
                            retries,
                            max_retries
                        );
                        tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                    }
                    ErrorClass::Retryable => {
                        if retries >= max_retries {
                            return Err(err);
                        }
                        retries += 1;
                        let delay = 2_u64.pow(retries - 1).min(MAX_BACKOFF_SECONDS);
                        tracing::warn!(
                            "LLM call failed with recoverable error, retrying in {}s (attempt {}/{})",
                            delay,
                            retries,
                            max_retries
                        );
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                    }
                }
            }
        }
    }
}

/// A provider that retries each provider and falls back through a chain.
///
/// The first provider is the primary; subsequent providers are fallbacks.
/// Each provider is retried with exponential backoff before moving to the next.
pub struct ReliableProvider {
    providers: Vec<Arc<dyn LlmProvider>>,
    current_index: AtomicUsize,
    max_retries_per_provider: u32,
    /// Whether to truncate history and retry once on context-window errors.
    truncate_on_context_window: bool,
    /// Whether to re-roll once on empty completions.
    reroll_on_empty_completion: bool,
}

impl ReliableProvider {
    /// Create a new ReliableProvider with a chain of providers.
    pub fn new(providers: Vec<Arc<dyn LlmProvider>>) -> Self {
        Self {
            providers,
            current_index: AtomicUsize::new(0),
            max_retries_per_provider: 3,
            truncate_on_context_window: true,
            reroll_on_empty_completion: true,
        }
    }

    /// Set the maximum retries per provider before falling back.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries_per_provider = max_retries;
        self
    }

    /// Disable context-window truncation retries.
    pub fn without_context_truncation(mut self) -> Self {
        self.truncate_on_context_window = false;
        self
    }

    /// Disable empty-completion re-rolls.
    pub fn without_empty_reroll(mut self) -> Self {
        self.reroll_on_empty_completion = false;
        self
    }

    async fn complete_with_provider(
        &self,
        provider: &Arc<dyn LlmProvider>,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let response = if self.max_retries_per_provider == 0 {
            provider.complete(messages, tools).await?
        } else {
            retry_with_backoff(
                || provider.complete(messages, tools),
                self.max_retries_per_provider,
            )
            .await?
        };

        if self.reroll_on_empty_completion
            && classify_response(&response) == ResponseClass::EmptyCompletion
        {
            tracing::warn!("LLM returned empty completion; re-rolling once");
            if self.max_retries_per_provider == 0 {
                provider.complete(messages, tools).await
            } else {
                retry_with_backoff(
                    || provider.complete(messages, tools),
                    self.max_retries_per_provider,
                )
                .await
            }
        } else {
            Ok(response)
        }
    }
}

#[async_trait]
impl LlmProvider for ReliableProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let mut last_err = None;
        for (idx, provider) in self.providers.iter().enumerate() {
            self.current_index.store(idx, Ordering::SeqCst);

            match self.complete_with_provider(provider, messages, tools).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    let class = classify_error(&err);
                    if class == ErrorClass::ContextWindowExceeded && self.truncate_on_context_window
                    {
                        let truncated = truncate_oldest_non_system(messages);
                        if truncated.len() < messages.len() {
                            tracing::warn!(
                                "Context window exceeded with provider {}; truncating oldest non-system message and retrying once",
                                idx
                            );
                            match self
                                .complete_with_provider(provider, &truncated, tools)
                                .await
                            {
                                Ok(response) => return Ok(response),
                                Err(retry_err) => {
                                    tracing::warn!(
                                        "Truncated retry also failed for provider {}: {}",
                                        idx,
                                        retry_err
                                    );
                                    last_err = Some(retry_err);
                                    continue;
                                }
                            }
                        }
                    }

                    tracing::warn!(
                        "Provider {} failed (attempt {} in chain): {}",
                        idx,
                        idx + 1,
                        err
                    );
                    last_err = Some(err);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| AgentError::Llm("All providers failed".into())))
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let mut last_err = None;
        for (idx, provider) in self.providers.iter().enumerate() {
            self.current_index.store(idx, Ordering::SeqCst);
            match provider.stream(messages, tools) {
                Ok(rx) => return Ok(rx),
                Err(err) => {
                    tracing::warn!(
                        "Provider {} failed in stream (attempt {} in chain): {}",
                        idx,
                        idx + 1,
                        err
                    );
                    last_err = Some(err);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| AgentError::Llm("All providers failed in stream".into())))
    }

    fn set_prompt_cache_key(&self, key: &str) {
        for provider in &self.providers {
            provider.set_prompt_cache_key(key);
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.providers
            .first()
            .map(|p| p.capabilities())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FailingProvider {
        fail_count: AtomicUsize,
        success_after: usize,
        response: String,
    }

    #[async_trait]
    impl LlmProvider for FailingProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<LlmResponse, AgentError> {
            let current = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if current < self.success_after {
                Err(AgentError::Llm("fail".into()))
            } else {
                Ok(LlmResponse {
                    content: self.response.clone(),
                    tool_calls: vec![],
                    is_complete: true,
                })
            }
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
        {
            let current = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if current < self.success_after {
                Err(AgentError::Llm("fail".into()))
            } else {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                let response = self.response.clone();
                tokio::spawn(async move {
                    let _ = tx
                        .send(Ok(StreamDelta {
                            content: Some(response),
                            reasoning_content: None,
                            tool_calls: vec![],
                        }))
                        .await;
                });
                Ok(rx)
            }
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::default()
        }
        fn set_prompt_cache_key(&self, _key: &str) {}
    }

    #[tokio::test]
    async fn test_reliable_retries_then_succeeds() {
        let provider = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: 2,
            response: "ok".into(),
        });
        let reliable = ReliableProvider::new(vec![provider]).with_max_retries(3);
        let response = reliable
            .complete(&[], &Value::Null)
            .await
            .expect("should succeed after retries");
        assert_eq!(response.content, "ok");
    }

    #[tokio::test]
    async fn test_reliable_fallback_chain() {
        let primary = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: usize::MAX,
            response: "primary".into(),
        });
        let fallback = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: 0,
            response: "fallback".into(),
        });
        let reliable = ReliableProvider::new(vec![primary, fallback]).with_max_retries(1);
        let response = reliable
            .complete(&[], &Value::Null)
            .await
            .expect("fallback should succeed");
        assert_eq!(response.content, "fallback");
    }

    #[tokio::test]
    async fn test_empty_completion_reroll() {
        struct EmptyThenOk {
            count: AtomicUsize,
        }
        #[async_trait]
        impl LlmProvider for EmptyThenOk {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<LlmResponse, AgentError> {
                let current = self.count.fetch_add(1, Ordering::SeqCst);
                Ok(LlmResponse {
                    content: if current == 0 { "".into() } else { "ok".into() },
                    tool_calls: vec![],
                    is_complete: true,
                })
            }
            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
            {
                unimplemented!()
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities::default()
            }
            fn set_prompt_cache_key(&self, _key: &str) {}
        }
        let provider = Arc::new(EmptyThenOk {
            count: AtomicUsize::new(0),
        });
        let reliable = ReliableProvider::new(vec![provider]).with_max_retries(0);
        let response = reliable.complete(&[], &Value::Null).await.unwrap();
        assert_eq!(response.content, "ok");
    }

    #[tokio::test]
    async fn test_context_window_truncation() {
        struct ContextWindowProvider {
            seen: AtomicUsize,
        }
        #[async_trait]
        impl LlmProvider for ContextWindowProvider {
            async fn complete(
                &self,
                messages: &[Message],
                _tools: &Value,
            ) -> Result<LlmResponse, AgentError> {
                let call = self.seen.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    Err(AgentError::Llm("context length exceeded".into()))
                } else {
                    Ok(LlmResponse {
                        content: format!("len={}", messages.len()),
                        tool_calls: vec![],
                        is_complete: true,
                    })
                }
            }
            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
            {
                unimplemented!()
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities::default()
            }
            fn set_prompt_cache_key(&self, _key: &str) {}
        }
        let provider = Arc::new(ContextWindowProvider {
            seen: AtomicUsize::new(0),
        });
        let reliable = ReliableProvider::new(vec![provider]).with_max_retries(0);
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "sys".into(),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: MessageRole::User,
                content: "old".into(),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: MessageRole::User,
                content: "new".into(),
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        let response = reliable.complete(&messages, &Value::Null).await.unwrap();
        assert_eq!(response.content, "len=2");
    }
}
