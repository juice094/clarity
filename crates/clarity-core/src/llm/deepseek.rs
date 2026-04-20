//! DeepSeek LLM Provider
//!
//! DeepSeek (https://deepseek.com) provides OpenAI-compatible API.
//! This module wraps the generic OpenAI-compatible provider with DeepSeek-specific defaults.
//!
//! ## Configuration
//!
//! ```bash
//! export DEEPSEEK_API_KEY="sk-your-key"
//! export DEEPSEEK_MODEL="deepseek-chat"  # or "deepseek-reasoner"
//! ```

use crate::error::AgentError;
use crate::llm::api::{LlmProvider, LlmResponse, Message};
use crate::llm::OpenAiCompatibleLlm;
use async_trait::async_trait;
use serde_json::Value;
use std::env;

/// DeepSeek LLM Provider
///
/// Uses OpenAI-compatible API at https://api.deepseek.com/v1
#[derive(Debug, Clone)]
pub struct DeepSeekProvider {
    inner: OpenAiCompatibleLlm,
}

impl DeepSeekProvider {
    /// Create from environment variables
    ///
    /// Required: `DEEPSEEK_API_KEY`
    /// Optional: `DEEPSEEK_MODEL` (default: "deepseek-chat")
    /// Optional: `DEEPSEEK_BASE_URL` (default: "https://api.deepseek.com/v1")
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("DEEPSEEK_API_KEY")
            .map_err(|_| AgentError::Llm("DEEPSEEK_API_KEY not set".into()))?;

        let base_url =
            env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());

        let model = env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create with explicit parameters
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new(api_key, base_url, model),
        }
    }

    /// Quick constructor for DeepSeek Chat (V3)
    pub fn chat(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.deepseek.com/v1", "deepseek-chat")
    }

    /// Quick constructor for DeepSeek Reasoner (R1)
    pub fn reasoner(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.deepseek.com/v1", "deepseek-reasoner")
    }

    pub fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

#[async_trait]
impl LlmProvider for DeepSeekProvider {
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
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>, AgentError>
    {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

/// Available DeepSeek models
pub mod models {
    /// DeepSeek Chat (V3) - General purpose conversation
    pub const DEEPSEEK_CHAT: &str = "deepseek-chat";
    /// DeepSeek Reasoner (R1) - Advanced reasoning capabilities
    pub const DEEPSEEK_REASONER: &str = "deepseek-reasoner";
    /// DeepSeek Coder - Code generation and analysis
    pub const DEEPSEEK_CODER: &str = "deepseek-coder";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deepseek_provider_creation() {
        let _provider = DeepSeekProvider::chat("test-key");
    }

    #[test]
    fn test_reasoner_provider() {
        let _provider = DeepSeekProvider::reasoner("test-key");
    }

    #[test]
    fn test_models_constants() {
        assert_eq!(models::DEEPSEEK_CHAT, "deepseek-chat");
        assert_eq!(models::DEEPSEEK_REASONER, "deepseek-reasoner");
        assert_eq!(models::DEEPSEEK_CODER, "deepseek-coder");
    }
}
