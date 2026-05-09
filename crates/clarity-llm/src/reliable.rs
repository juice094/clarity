//! ReliableProvider — fallback chain wrapper for LLM providers.
//!
//! When the primary provider fails, automatically tries the next provider
//! in the chain until one succeeds or all fail.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use clarity_contract::AgentError;
use crate::api::{LlmProvider, LlmResponse, Message, ProviderCapabilities, StreamDelta};

/// Retry an async operation with exponential backoff.
///
/// Only retries when the error is recoverable (`is_recoverable() == true`).
/// Max retries = `max_retries`, with delays 1s, 2s, 4s, ...
async fn retry_with_backoff<F, Fut, T>(mut f: F, max_retries: u32) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut retries = 0;
    loop {
        match f().await {
            Ok(output) => return Ok(output),
            Err(err) if !err.is_recoverable() => return Err(err),
            Err(err) => {
                if retries >= max_retries {
                    return Err(err);
                }
                retries += 1;
                let delay = std::time::Duration::from_secs(2_u64.pow(retries - 1));
                tracing::warn!(
                    "LLM call failed with recoverable error, retrying in {:?} (attempt {}/{})",
                    delay,
                    retries,
                    max_retries
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// A provider that falls back through a chain of providers on failure.
///
/// The first provider is the primary; subsequent providers are fallbacks.
/// Each provider is retried with exponential backoff before moving to the next.
pub struct ReliableProvider {
    providers: Vec<Arc<dyn LlmProvider>>,
    current_index: AtomicUsize,
    max_retries_per_provider: u32,
}

impl ReliableProvider {
    /// Create a new ReliableProvider with a chain of providers.
    pub fn new(providers: Vec<Arc<dyn LlmProvider>>) -> Self {
        Self {
            providers,
            current_index: AtomicUsize::new(0),
            max_retries_per_provider: 3,
        }
    }

    /// Set the maximum retries per provider before falling back.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries_per_provider = max_retries;
        self
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
            let result = if self.max_retries_per_provider == 0 {
                provider.complete(messages, tools).await
            } else {
                retry_with_backoff(
                    || provider.complete(messages, tools),
                    self.max_retries_per_provider,
                )
                .await
            };
            match result {
                Ok(response) => return Ok(response),
                Err(err) => {
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
                            tool_calls: vec![],
                        }))
                        .await;
                });
                Ok(rx)
            }
        }

        fn set_prompt_cache_key(&self, _key: &str) {}
    }

    #[tokio::test]
    async fn test_reliable_provider_fallback_to_second() {
        let p1 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: usize::MAX,
            response: "p1".to_string(),
        });
        let p2 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: 0,
            response: "p2".to_string(),
        });

        let reliable = ReliableProvider::new(vec![p1, p2]).with_max_retries(0);
        let result = reliable.complete(&[], &Value::Null).await.unwrap();
        assert_eq!(result.content, "p2");
    }

    #[tokio::test]
    async fn test_reliable_provider_all_fail_returns_last_error() {
        let p1 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: usize::MAX,
            response: "p1".to_string(),
        });
        let p2 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: usize::MAX,
            response: "p2".to_string(),
        });

        let reliable = ReliableProvider::new(vec![p1, p2]).with_max_retries(0);
        let result = reliable.complete(&[], &Value::Null).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("fail"), "Expected last error, got: {}", err);
    }

    #[tokio::test]
    async fn test_reliable_provider_stream_fallback() {
        let p1 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: usize::MAX,
            response: "p1".to_string(),
        });
        let p2 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: 0,
            response: "stream_p2".to_string(),
        });

        let reliable = ReliableProvider::new(vec![p1, p2]).with_max_retries(0);
        let mut rx = reliable.stream(&[], &Value::Null).unwrap();
        let delta = rx.recv().await.unwrap().unwrap();
        assert_eq!(delta.content, Some("stream_p2".to_string()));
    }

    #[tokio::test]
    async fn test_reliable_provider_capabilities_returns_primary() {
        let p1 = Arc::new(FailingProvider {
            fail_count: AtomicUsize::new(0),
            success_after: 0,
            response: "p1".to_string(),
        });
        let reliable = ReliableProvider::new(vec![p1]).with_max_retries(0);
        let caps = reliable.capabilities();
        // FailingProvider does not override capabilities, so it uses default
        assert!(caps.native_tool_calling);
    }
}
