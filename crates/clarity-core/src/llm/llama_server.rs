//! Local LLM inference via llama.cpp server (HTTP bridge)
//!
//! Connects to a locally-running `llama-server` instance via its
//! OpenAI-compatible `/v1/chat/completions` endpoint.
//!
//! ## Quick start
//!
//! 1. Download a GGUF model (e.g., Qwen2.5-7B-Instruct.Q4_K_M.gguf)
//! 2. Start llama-server:
//!    ```bash
//!    llama-server -m model.gguf -ngl 99 --port 8080
//!    ```
//! 3. Configure in `models.toml`:
//!    ```toml
//!    [providers.local-llama]
//!    protocol = "llama_server"
//!    base_url = "http://localhost:8080"
//!
//!    [[models]]
//!    alias = "local-qwen"
//!    provider = "local-llama"
//!    model_id = "qwen2.5-7b-instruct"
//!    ```
//!
//! ## Why not Ollama?
//!
//! `llama-server` is the raw llama.cpp HTTP endpoint. It exposes more
//! low-level controls (context size, GPU layers, draft model for speculative
//! decoding) than Ollama's higher-level abstraction. For agent frameworks
//! that need fine-grained control, talking directly to llama-server is
//! preferable.

use super::{OpenAiCompatibleLlm, StreamDelta};
use crate::error::AgentError;
use crate::llm::api::{LlmProvider, LlmResponse, Message};
use async_trait::async_trait;
use serde_json::Value;

/// Provider that talks to a local llama.cpp server instance.
///
/// Internally wraps `OpenAiCompatibleLlm` (since llama-server exposes
/// an OpenAI-compatible API), but adds llama-specific diagnostics and
/// a more helpful error message when the server is unreachable.
#[derive(Debug, Clone)]
pub struct LlamaServerProvider {
    inner: OpenAiCompatibleLlm,
    base_url: String,
}

impl LlamaServerProvider {
    /// Create a new provider pointing at a llama-server instance.
    ///
    /// `base_url` should be the root URL, e.g. `http://localhost:8080`.
    /// `model` is the model ID used in the request body (llama-server
    /// usually ignores this, but it is required for OpenAI compatibility).
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let base_url = base_url.into();
        let inner = OpenAiCompatibleLlm::new("no-key-required", &base_url, model);
        Self { inner, base_url }
    }

    /// Attempt a lightweight health check against the server.
    ///
    /// Returns `Ok(())` if the server responds, or an error with a
    /// helpful message if it does not.
    pub async fn health_check(&self) -> Result<(), AgentError> {
        let url = format!("{}/health", self.base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| AgentError::Llm(format!("Failed to build health-check client: {}", e)))?;

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(AgentError::Llm(format!(
                "llama-server at {} returned status {}. \
                 Ensure llama-server is running: llama-server -m model.gguf --port 8080",
                self.base_url,
                resp.status()
            ))),
            Err(e) => Err(AgentError::Llm(format!(
                "Cannot connect to llama-server at {}: {}. \
                 Ensure llama-server is running: llama-server -m model.gguf --port 8080",
                self.base_url, e
            ))),
        }
    }
}

#[async_trait]
impl LlmProvider for LlamaServerProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        self.inner.complete(messages, tools).await.map_err(|e| {
            let msg = format!("{}", e);
            if msg.contains("error sending request") || msg.contains("Connection refused") {
                AgentError::Llm(format!(
                    "llama-server unreachable at {}. \
                     Start it with: llama-server -m model.gguf -ngl 99 --port 8080\n\
                     Original error: {}",
                    self.base_url, e
                ))
            } else {
                e
            }
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }

    fn capabilities(&self) -> crate::llm::api::ProviderCapabilities {
        crate::llm::api::ProviderCapabilities {
            native_tool_calling: true,
            prompt_caching: true,
            vision: false,
            pricing: None,
        }
    }
}
