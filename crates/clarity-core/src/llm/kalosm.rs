//! Kalosm Local LLM Provider (skeleton)
//!
//! This module provides a local LLM provider using the kalosm ecosystem.
//! It is currently a skeleton implementation to establish the `LlmProvider`
//! trait boundary while the real kalosm/candle integration is being prepared
//! by the agri-paper project.
//!
//! When the `kalosm` cargo feature is enabled in the future, this skeleton
//! will be filled with actual `kalosm::Model` loading and inference logic.

use crate::agent::{LlmProvider, LlmResponse, Message};
use crate::error::AgentError;
use crate::llm::StreamDelta;
use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, warn};

/// Local LLM provider backed by kalosm
#[derive(Debug, Clone)]
pub struct KalosmProvider {
    model_path: Option<std::path::PathBuf>,
    prompt_cache_key: Option<String>,
}

impl KalosmProvider {
    /// Create a new skeleton provider
    pub fn new() -> Self {
        Self {
            model_path: None,
            prompt_cache_key: None,
        }
    }

    /// Create a provider with a specific model path
    pub fn with_model_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            model_path: Some(path.into()),
            prompt_cache_key: None,
        }
    }
}

impl Default for KalosmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for KalosmProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        info!("[KalosmProvider] complete() called (skeleton mode)");

        if let Some(path) = &self.model_path {
            info!("[KalosmProvider] Model path: {:?}", path);
        } else {
            warn!("[KalosmProvider] No model path configured");
        }

        // Skeleton: return a mock response indicating the provider is not yet
        // backed by a real kalosm model.
        Ok(LlmResponse {
            content: "[KalosmProvider skeleton] Local inference not yet implemented.".to_string(),
            tool_calls: Vec::new(),
            is_complete: true,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        info!("[KalosmProvider] stream() called (skeleton mode)");

        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let content = if self.model_path.is_some() {
            "[KalosmProvider skeleton] Streaming local inference not yet implemented."
        } else {
            "[KalosmProvider skeleton] No model path configured."
        };

        let delta = StreamDelta {
            content: Some(content.to_string()),
            tool_calls: Vec::new(),
        };

        // Send a single delta and close the channel
        tx.try_send(Ok(delta))
            .map_err(|e| AgentError::Llm(format!("Failed to send stream delta: {}", e)))?;

        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.prompt_cache_key = Some(key.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kalosm_provider_complete_skeleton() {
        let provider = KalosmProvider::with_model_path("/tmp/mock.gguf");
        let result = provider.complete(&[], &serde_json::json!({})).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.content.contains("skeleton"));
        assert!(response.tool_calls.is_empty());
        assert!(response.is_complete);
    }

    #[tokio::test]
    async fn test_kalosm_provider_stream_skeleton() {
        let provider = KalosmProvider::new();
        let mut rx = provider.stream(&[], &serde_json::json!({})).unwrap();

        let delta = rx.recv().await.unwrap().unwrap();
        assert!(delta.content.as_ref().unwrap().contains("skeleton"));
        assert!(delta.tool_calls.is_empty());

        // Channel should close after the single delta
        assert!(rx.recv().await.is_none());
    }
}
