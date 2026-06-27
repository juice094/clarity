//! OAuth-backed LLM provider.
//!
//! Wraps an OpenAI-compatible LLM with automatic OAuth token refresh.
//! Supports any provider that uses OAuth 2.0 Device Authorization Grant.

use crate::api::{LlmProvider, LlmResponse, Message, ProviderCapabilities, StreamDelta};
use crate::auth::OAuthTokenManager;
use crate::providers::OpenAiCompatibleLlm;
use async_trait::async_trait;
use clarity_contract::AgentError;
use serde_json::Value;
use std::env;

/// OAuth-backed LLM provider.
///
/// Wraps an OpenAI-compatible LLM with automatic OAuth token refresh.
/// Supports any provider that uses OAuth 2.0 Device Authorization Grant.
#[derive(Debug, Clone)]
pub struct OAuthLlm {
    inner: OpenAiCompatibleLlm,
    token_manager: OAuthTokenManager,
}

impl OAuthLlm {
    /// Create with an explicit token manager.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        token_manager: OAuthTokenManager,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new(api_key, base_url, model),
            token_manager,
        }
    }

    /// Create with the default Kimi Code configuration (convenience alias).
    pub fn kimi_code() -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new("", "https://api.kimi.com/coding/v1", "kimi-k2.6"),
            token_manager: OAuthTokenManager::new(),
        }
    }

    /// Create from environment variables (backward-compatible with KimiCodeLlm::from_env).
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("KIMI_CODE_API_KEY")
            .map_err(|_| AgentError::Llm("KIMI_CODE_API_KEY not set".into()))?;
        let base_url = env::var("KIMI_CODE_BASE_URL")
            .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into());
        let model = env::var("KIMI_CODE_MODEL").unwrap_or_else(|_| "kimi-k2.6".into());
        Ok(Self::new(
            api_key,
            base_url,
            model,
            OAuthTokenManager::new(),
        ))
    }

    /// Set a key used to enable prompt caching for subsequent requests.
    pub fn set_prompt_cache_key(&self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

#[async_trait]
impl LlmProvider for OAuthLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        if let Ok(Some(token)) = self.token_manager.try_fresh().await {
            self.inner.update_api_key(token);
        }
        // If try_fresh returns None, fall back to the static api_key set at construction time.
        self.inner.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let inner = self.inner.clone();
        let token_manager = self.token_manager.clone();
        let messages = messages.to_vec();
        let tools = tools.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            if let Ok(Some(token)) = token_manager.try_fresh().await {
                inner.update_api_key(token);
            }
            match inner.stream(&messages, &tools) {
                Ok(mut inner_rx) => {
                    while let Some(item) = inner_rx.recv().await {
                        if tx.send(item).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                }
            }
        });

        Ok(rx)
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

/// Backward-compatible alias for the Kimi Code LLM provider.
pub type KimiCodeLlm = OAuthLlm;
