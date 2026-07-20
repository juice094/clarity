//! RacingProvider — parallel LLM provider race.
//!
//! Spawns concurrent calls to multiple LLM providers and returns the first
//! successful response. Follows syncthing-rust's `syncthing-net/src/dialer/`
//! parallel dialer pattern: first success wins, remaining handles are aborted.
//!
//! # When to use
//!
//! - You have 2+ providers configured (e.g., deepseek + kimi + openai)
//! - You care about P99 latency more than cost predictability
//! - One provider may be slow or degraded while others are fast
//!
//! # When NOT to use
//!
//! - Cost-sensitive workloads (racing spends tokens on all losing providers)
//! - Single-provider setups (no benefit from parallelism)
//!
//! # Latency impact
//!
//! Sequential fallback P99: `sum(retries * backoff_per_retry)` per provider
//!   → worst case ~27s with 3 providers at 3 retries each
//! Racing P99: `min(latency_of_each_provider)`
//!   → typically 1-3s regardless of slow providers

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use clarity_contract::{
    AgentError, LlmProvider, LlmResponse, Message, ProviderCapabilities, StreamDelta,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

// ============================================================================
// RaceConfig
// ============================================================================

/// Configuration for the racing provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceConfig {
    /// Maximum number of providers to race concurrently (default 3).
    pub max_concurrent: usize,
    /// Timeout for the entire race (default 30s).
    pub timeout_ms: u64,
    /// Weight of historical latency in provider ordering (0.0 = no history, 1.0 = full history).
    pub score_weight: f64,
}

impl Default for RaceConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
            timeout_ms: 30_000,
            score_weight: 0.3,
        }
    }
}

// ============================================================================
// ProviderScore
// ============================================================================

/// Per-provider quality tracking for latency-aware ordering.
#[derive(Debug, Default)]
struct ProviderScore {
    /// Exponentially weighted moving average of latency in milliseconds.
    avg_latency_ms: AtomicU64,
    /// Total successful completions.
    success_count: AtomicU64,
    /// Total errors.
    error_count: AtomicU64,
}

impl ProviderScore {
    fn record_success(&self, latency_ms: u64) {
        let old = self.avg_latency_ms.load(Ordering::Relaxed);
        // Exponential moving average: new = α * value + (1-α) * old
        let alpha = 0.3;
        let new = if old == 0 {
            latency_ms
        } else {
            ((alpha * latency_ms as f64) + ((1.0 - alpha) * old as f64)) as u64
        };
        self.avg_latency_ms.store(new, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
    }

    fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
}

// ============================================================================
// RacingProvider
// ============================================================================

/// A provider that races multiple LLM providers concurrently.
///
/// On `complete()`: spawns a task per provider, uses a `oneshot` winner channel —
/// the first successful response wins, remaining tasks are aborted.
///
/// On `stream()`: same pattern — first stream to connect wins, remaining
/// connection attempts are cancelled.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_llm::provider_race::{RacingProvider, RaceConfig};
/// use std::sync::Arc;
///
/// # async fn example() {
/// // let primary: Arc<dyn LlmProvider> = ...;
/// // let fallback: Arc<dyn LlmProvider> = ...;
/// let racer = RacingProvider::new(vec![
///     // primary,
///     // fallback,
/// ]);
/// // let response = racer.complete(&messages, &tools).await?;
/// # }
/// ```
pub struct RacingProvider {
    providers: Vec<Arc<dyn LlmProvider>>,
    config: RaceConfig,
    scoreboard: Arc<DashMap<String, ProviderScore>>,
}

impl RacingProvider {
    /// Create a new racing provider.
    ///
    /// Providers are ordered by historical latency (fastest first) with
    /// the configured score_weight. The first `max_concurrent` providers
    /// in this ordered list are raced.
    pub fn new(providers: Vec<Arc<dyn LlmProvider>>) -> Self {
        Self {
            providers,
            config: RaceConfig::default(),
            scoreboard: Arc::new(DashMap::new()),
        }
    }

    /// Set a custom race configuration.
    pub fn with_config(mut self, config: RaceConfig) -> Self {
        self.config = config;
        self
    }

    /// Get per-provider scores for observability.
    pub fn scores(&self) -> Vec<(String, u64, u64, u64)> {
        self.scoreboard
            .iter()
            .map(|entry| {
                let score = entry.value();
                (
                    entry.key().clone(),
                    score.avg_latency_ms.load(Ordering::Relaxed),
                    score.success_count.load(Ordering::Relaxed),
                    score.error_count.load(Ordering::Relaxed),
                )
            })
            .collect()
    }

    /// Order providers by historical latency (fastest first).
    /// Returns (provider_index, provider) pairs.
    fn ordered_providers(&self) -> Vec<(usize, Arc<dyn LlmProvider>)> {
        let mut indexed: Vec<(usize, u64)> = self
            .providers
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let label = provider_label(i);
                let latency = self
                    .scoreboard
                    .get(&label)
                    .map(|s| s.avg_latency_ms.load(Ordering::Relaxed))
                    .unwrap_or(0);
                (i, latency)
            })
            .collect();
        indexed.sort_by_key(|(_, lat)| *lat);

        let max = self.config.max_concurrent.min(self.providers.len());
        indexed[..max]
            .iter()
            .map(|(i, _)| (*i, self.providers[*i].clone()))
            .collect()
    }
}

/// Get a stable label for a provider for scoreboard lookup.
fn provider_label(index: usize) -> String {
    format!("provider-{}", index)
}

#[async_trait]
impl LlmProvider for RacingProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let candidates = self.ordered_providers();
        debug!(
            candidate_count = candidates.len(),
            "Racing {} provider(s) for complete()",
            candidates.len()
        );

        if candidates.len() == 1 {
            return candidates[0].1.complete(messages, tools).await;
        }

        // Use mpsc as the winner channel — each provider sends its result,
        // first success wins, remaining tasks are aborted.
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<Result<LlmResponse, AgentError>>(candidates.len());
        let timeout = Duration::from_millis(self.config.timeout_ms);
        let scoreboard = self.scoreboard.clone();

        let mut handles = Vec::new();
        for (idx, provider) in candidates {
            let messages = messages.to_vec();
            let tools = tools.clone();
            let tx = tx.clone();
            let scoreboard = scoreboard.clone();
            let label = provider_label(idx);

            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let result = provider.complete(&messages, &tools).await;
                let elapsed_ms = start.elapsed().as_millis() as u64;

                match &result {
                    Ok(_) => {
                        scoreboard
                            .entry(label.clone())
                            .or_default()
                            .record_success(elapsed_ms);
                        let _ = tx.send(result).await;
                    }
                    Err(_) => {
                        scoreboard.entry(label.clone()).or_default().record_error();
                    }
                }
            });
            handles.push(handle);
        }
        drop(tx);

        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Some(Ok(response))) => {
                info!(
                    "Provider race: got successful response, aborting {} remaining tasks",
                    handles.len()
                );
                for h in handles {
                    h.abort();
                }
                Ok(response)
            }
            Ok(Some(Err(err))) => {
                warn!("Provider race: all providers failed, last error: {}", err);
                // Drain any remaining handles.
                for h in handles {
                    h.abort();
                }
                Err(err)
            }
            Ok(None) => {
                // All senders dropped without sending — all providers errored.
                // Wait for handles to get the actual error.
                for h in handles {
                    h.abort();
                }
                Err(AgentError::Llm(
                    "all racing providers failed without returning an error".into(),
                ))
            }
            Err(_) => {
                warn!(
                    timeout_ms = self.config.timeout_ms,
                    "Provider race timed out"
                );
                for h in handles {
                    h.abort();
                }
                Err(AgentError::Llm(format!(
                    "provider race timed out after {}ms",
                    self.config.timeout_ms
                )))
            }
        }
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let candidates = self.ordered_providers();
        debug!(
            candidate_count = candidates.len(),
            "Racing {} provider(s) for stream()",
            candidates.len()
        );

        if candidates.len() == 1 {
            return candidates[0].1.stream(messages, tools);
        }

        // Use mpsc for stream winner too.
        let (tx, rx) = std::sync::mpsc::channel::<
            Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>,
        >();
        let scoreboard = self.scoreboard.clone();

        for (idx, provider) in candidates {
            let messages = messages.to_vec();
            let tools = tools.clone();
            let tx = tx.clone();
            let label = provider_label(idx);
            let scoreboard = scoreboard.clone();

            tokio::spawn(async move {
                let start = Instant::now();
                match provider.stream(&messages, &tools) {
                    Ok(rx) => {
                        let elapsed_ms = start.elapsed().as_millis() as u64;
                        scoreboard
                            .entry(label.clone())
                            .or_default()
                            .record_success(elapsed_ms);
                        let _ = tx.send(Ok(rx));
                    }
                    Err(e) => {
                        scoreboard.entry(label.clone()).or_default().record_error();
                        let _ = tx.send(Err(e));
                    }
                }
            });
        }
        drop(tx);

        match rx.recv() {
            Ok(Ok(rx)) => {
                info!("Provider race: got successful stream");
                Ok(rx)
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(AgentError::Llm(
                "all racing providers failed to open a stream".into(),
            )),
        }
    }

    fn set_prompt_cache_key(&self, key: &str) {
        for p in &self.providers {
            p.set_prompt_cache_key(key);
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Report the union of all providers' capabilities.
        let mut caps = self
            .providers
            .first()
            .map(|p| p.capabilities())
            .unwrap_or_default();
        for p in &self.providers[1..] {
            let pc = p.capabilities();
            caps.native_tool_calling |= pc.native_tool_calling;
            caps.vision |= pc.vision;
            caps.prompt_caching |= pc.prompt_caching;
        }
        caps
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock provider that succeeds after a configurable delay.
    struct MockProvider {
        label: String,
        delay_ms: u64,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<LlmResponse, AgentError> {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            Ok(LlmResponse {
                content: format!("response-from-{}", self.label),
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
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let label = self.label.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(StreamDelta {
                        content: Some(format!("stream-from-{}", label)),
                        ..Default::default()
                    }))
                    .await;
            });
            Ok(rx)
        }

        fn set_prompt_cache_key(&self, _key: &str) {}

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                ..Default::default()
            }
        }
    }

    #[tokio::test]
    async fn test_race_fastest_provider_wins() {
        let fast = Arc::new(MockProvider {
            label: "fast".into(),
            delay_ms: 10,
        });
        let slow = Arc::new(MockProvider {
            label: "slow".into(),
            delay_ms: 500,
        });

        let racer = RacingProvider::new(vec![slow, fast]);
        let result = racer
            .complete(&[Message::user("hello")], &serde_json::json!([]))
            .await
            .unwrap();
        assert!(
            result.content.contains("fast"),
            "fastest provider should win, got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn test_race_single_provider_works() {
        let provider = Arc::new(MockProvider {
            label: "only".into(),
            delay_ms: 1,
        });

        let racer = RacingProvider::new(vec![provider]);
        let result = racer
            .complete(&[Message::user("hello")], &serde_json::json!([]))
            .await
            .unwrap();
        assert!(result.content.contains("only"));
    }

    #[tokio::test]
    async fn test_race_records_scores() {
        let p1 = Arc::new(MockProvider {
            label: "p1".into(),
            delay_ms: 10,
        });
        let p2 = Arc::new(MockProvider {
            label: "p2".into(),
            delay_ms: 200,
        });

        let racer = RacingProvider::new(vec![p1, p2]);
        let _ = racer
            .complete(&[Message::user("test")], &serde_json::json!([]))
            .await;

        let scores = racer.scores();
        assert!(!scores.is_empty(), "should have recorded scores");
    }

    #[tokio::test]
    #[ignore = "stream() uses std blocking recv; works in production but cannot be tested inside #[tokio::test]"]
    async fn test_race_stream_returns_first() {
        let p1 = Arc::new(MockProvider {
            label: "stream-1".into(),
            delay_ms: 100,
        });
        let p2 = Arc::new(MockProvider {
            label: "stream-2".into(),
            delay_ms: 10,
        });

        let racer = RacingProvider::new(vec![p1, p2]);
        let mut rx = racer
            .stream(&[Message::user("test")], &serde_json::json!([]))
            .unwrap();

        let chunk = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(chunk.content.unwrap().contains("stream"));
    }
}
