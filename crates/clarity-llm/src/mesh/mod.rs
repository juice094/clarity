//! LLM Mesh — multi-provider routing with circuit-breaker failover.
//!
//! Wraps multiple `LlmProvider` instances behind a single `LlmProvider`
//! interface.  Calls are routed through a fallback chain; a provider
//! that fails 5 consecutive times is temporarily bypassed.

pub mod circuit;
pub mod registry;
pub mod router;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;
use tracing::{info, warn};

use crate::{LlmProvider, LlmResponse, ProviderCapabilities, StreamDelta};
use clarity_contract::{AgentError, Message};
use serde_json::Value;

use circuit::CircuitBreaker;
use registry::MeshRegistry;
use router::MeshRouter;

/// Multi-provider LLM wrapper with circuit-breaker failover.
pub struct MeshLlmProvider {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    router: MeshRouter,
    breakers: HashMap<String, Arc<CircuitBreaker>>,
}

impl MeshLlmProvider {
    /// Build a mesh from environment variables.
    ///
    /// - `CLARITY_MESH_PROVIDERS` — comma-separated provider names to load.
    /// - `CLARITY_MESH_FALLBACK`  — ordered fallback chain (defaults to providers list order).
    pub async fn from_env() -> Result<Self, AgentError> {
        let providers = MeshRegistry::from_env().await?;
        let mut router = MeshRouter::from_env();

        if router.fallback_chain.is_empty() {
            router.fallback_chain = providers.keys().cloned().collect();
        }

        let mut breakers = HashMap::new();
        for name in providers.keys() {
            breakers.insert(name.clone(), Arc::new(CircuitBreaker::default()));
        }

        Ok(Self {
            providers,
            router,
            breakers,
        })
    }

    /// Build a mesh from an explicit list of provider names.
    pub async fn from_names(names: Vec<String>) -> Result<Self, AgentError> {
        let mut providers = HashMap::new();
        for name in &names {
            match crate::LlmFactory::create_arc(name).await {
                Ok(provider) => {
                    tracing::info!("Mesh loaded provider: {}", name);
                    providers.insert(name.clone(), provider);
                }
                Err(e) => {
                    tracing::warn!("Mesh failed to load provider '{}': {}", name, e);
                }
            }
        }
        if providers.is_empty() {
            return Err(AgentError::Llm(
                "No mesh providers could be loaded from the given names".into(),
            ));
        }
        let router = MeshRouter::with_chain(names);
        Ok(Self::new(providers, router))
    }

    /// Build a mesh from an explicit provider map and fallback chain.
    pub fn new(providers: HashMap<String, Arc<dyn LlmProvider>>, router: MeshRouter) -> Self {
        let mut breakers = HashMap::new();
        for name in providers.keys() {
            breakers.insert(name.clone(), Arc::new(CircuitBreaker::default()));
        }
        Self {
            providers,
            router,
            breakers,
        }
    }

    /// Return the names of providers currently loaded.
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Return the current circuit state for each provider.
    pub fn circuit_states(&self) -> HashMap<String, circuit::CircuitState> {
        self.breakers
            .iter()
            .map(|(k, v)| (k.clone(), v.state()))
            .collect()
    }
}

#[async_trait]
impl LlmProvider for MeshLlmProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let mut last_err: Option<AgentError> = None;

        for name in &self.router.fallback_chain {
            let provider = match self.providers.get(name) {
                Some(p) => p.clone(),
                None => continue,
            };

            let breaker = self
                .breakers
                .get(name)
                .cloned()
                .unwrap_or_else(|| Arc::new(CircuitBreaker::default()));
            if !breaker.allow() {
                warn!("Mesh: provider {} circuit open, skipping", name);
                continue;
            }

            match provider.complete(messages, tools).await {
                Ok(resp) => {
                    breaker.record_success();
                    info!("Mesh: request served by {}", name);
                    return Ok(resp);
                }
                Err(e) => {
                    warn!("Mesh: provider {} failed: {}", name, e);
                    breaker.record_failure();
                    last_err = Some(e);
                }
            }
        }

        Err(AgentError::Llm(format!(
            "All mesh providers exhausted. Last error: {}",
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "none".into())
        )))
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let mut last_err: Option<AgentError> = None;

        for name in &self.router.fallback_chain {
            let provider = match self.providers.get(name) {
                Some(p) => p.clone(),
                None => continue,
            };

            let breaker = self
                .breakers
                .get(name)
                .cloned()
                .unwrap_or_else(|| Arc::new(CircuitBreaker::default()));
            if !breaker.allow() {
                continue;
            }

            match provider.stream(messages, tools) {
                Ok(rx) => {
                    breaker.record_success();
                    return Ok(rx);
                }
                Err(e) => {
                    breaker.record_failure();
                    last_err = Some(e);
                }
            }
        }

        Err(AgentError::Llm(format!(
            "All mesh providers exhausted (stream). Last error: {}",
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "none".into())
        )))
    }

    fn set_prompt_cache_key(&self, key: &str) {
        for (name, provider) in &self.providers {
            provider.set_prompt_cache_key(key);
            tracing::debug!("Mesh: set prompt cache key on {}", name);
        }
    }

    fn clear_cache(&self) {
        for provider in self.providers.values() {
            provider.clear_cache();
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Return the union of all provider capabilities.
        let mut caps = ProviderCapabilities::default();
        for provider in self.providers.values() {
            let p = provider.capabilities();
            if p.native_tool_calling {
                caps.native_tool_calling = true;
            }
            if p.vision {
                caps.vision = true;
            }
            if p.prompt_caching {
                caps.prompt_caching = true;
            }
            if p.pricing.is_some() {
                caps.pricing = p.pricing;
            }
        }
        caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{AgentError, Message};
    use serde_json::json;

    struct MockProvider {
        name: String,
        fail: bool,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<LlmResponse, AgentError> {
            if self.fail {
                Err(AgentError::Llm(format!("{} failed", self.name)))
            } else {
                Ok(LlmResponse {
                    content: format!("from {}", self.name),
                    tool_calls: vec![],
                    is_complete: true,
                })
            }
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<Receiver<Result<StreamDelta, AgentError>>, AgentError> {
            if self.fail {
                Err(AgentError::Llm(format!("{} stream failed", self.name)))
            } else {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                let name = self.name.clone();
                tokio::spawn(async move {
                    let _ = tx.send(Ok(StreamDelta {
                        content: Some(format!("stream from {}", name)),
                        tool_calls: vec![],
                    })).await;
                });
                Ok(rx)
            }
        }

        fn set_prompt_cache_key(&self, _key: &str) {}
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::default()
        }
    }

    fn build_mesh(providers: Vec<(String, bool)>, chain: Vec<String>) -> MeshLlmProvider {
        let mut map = HashMap::new();
        for (name, fail) in providers {
            map.insert(name.clone(), Arc::new(MockProvider { name, fail }) as Arc<dyn LlmProvider>);
        }
        MeshLlmProvider::new(map, MeshRouter::with_chain(chain))
    }

    #[tokio::test]
    async fn test_mesh_first_succeeds() {
        let mesh = build_mesh(
            vec![("a".into(), false), ("b".into(), true)],
            vec!["a".into(), "b".into()],
        );
        let resp = mesh.complete(&[], &json!({})).await.unwrap();
        assert_eq!(resp.content, "from a");
    }

    #[tokio::test]
    async fn test_mesh_fallback_on_failure() {
        let mesh = build_mesh(
            vec![("a".into(), true), ("b".into(), false)],
            vec!["a".into(), "b".into()],
        );
        let resp = mesh.complete(&[], &json!({})).await.unwrap();
        assert_eq!(resp.content, "from b");
    }

    #[tokio::test]
    async fn test_mesh_all_fail() {
        let mesh = build_mesh(
            vec![("a".into(), true), ("b".into(), true)],
            vec!["a".into(), "b".into()],
        );
        let err = mesh.complete(&[], &json!({})).await.unwrap_err();
        assert!(err.to_string().contains("All mesh providers exhausted"));
    }

    #[tokio::test]
    async fn test_mesh_circuit_opens_after_threshold() {
        let mut mesh = build_mesh(
            vec![("a".into(), true), ("b".into(), false)],
            vec!["a".into(), "b".into()],
        );
        // Override breaker to threshold 1 for fast testing
        mesh.breakers.insert("a".into(), Arc::new(CircuitBreaker::new(1, 60)));

        // First call fails and opens circuit
        let _ = mesh.complete(&[], &json!({})).await;
        assert_eq!(mesh.breakers.get("a").unwrap().state(), circuit::CircuitState::Open);

        // Second call should skip 'a' and go straight to 'b'
        let resp = mesh.complete(&[], &json!({})).await.unwrap();
        assert_eq!(resp.content, "from b");
    }

    #[tokio::test]
    async fn test_mesh_stream_fallback() {
        let mesh = build_mesh(
            vec![("a".into(), true), ("b".into(), false)],
            vec!["a".into(), "b".into()],
        );
        let mut rx = mesh.stream(&[], &json!({})).unwrap();
        let delta = rx.recv().await.unwrap().unwrap();
        assert_eq!(delta.content, Some("stream from b".into()));
    }
}
