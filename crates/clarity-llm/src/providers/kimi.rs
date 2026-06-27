//! Kimi (Moonshot) LLM provider.
//!
//! Thin wrapper around the generic OpenAI-compatible provider, tuned for
//! Moonshot's API endpoints and pricing model.

use crate::api::{LlmProvider, LlmResponse, Message, ProviderCapabilities, StreamDelta};
use crate::providers::OpenAiCompatibleLlm;
use async_trait::async_trait;
use clarity_contract::AgentError;
use serde_json::Value;
use std::env;

/// Kimi (Moonshot) LLM provider.
#[derive(Debug, Clone)]
pub struct KimiLlm {
    inner: OpenAiCompatibleLlm,
}

impl KimiLlm {
    /// Create from environment variables.
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key =
            env::var("KIMI_API_KEY").map_err(|_| AgentError::Llm("KIMI_API_KEY not set".into()))?;

        let base_url =
            env::var("KIMI_BASE_URL").unwrap_or_else(|_| "https://api.moonshot.ai/v1".into());

        let model = env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2.6".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create with explicit parameters.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new(api_key, base_url, model),
        }
    }
}

#[async_trait]
impl LlmProvider for KimiLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        self.inner.complete(messages, tools).await
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

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: true,
            prompt_guided_tool_calling: false,
            prompt_caching: true,
            vision: false,
            pricing: None,
        }
    }
}
