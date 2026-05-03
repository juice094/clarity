//! Local LLM inference via Kalosm (DEPRECATED)
//!
//! This module is kept for backward compatibility but has been superseded
//! by the native Candle implementation in `local_gguf.rs`.
//! All constructors now return an error directing users to `LocalGgufProvider`.

use super::{LlmProvider, LlmResponse, Message, StreamDelta};
use crate::error::AgentError;
use serde_json::Value;
use std::path::PathBuf;

/// Configuration for Kalosm local inference (deprecated).
#[derive(Debug, Clone)]
pub struct KalosmConfig {
    pub model_path: PathBuf,
    pub max_context_length: usize,
    pub temperature: f32,
}

impl Default for KalosmConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            max_context_length: 4096,
            temperature: 0.7,
        }
    }
}

impl KalosmConfig {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            ..Default::default()
        }
    }

    pub fn with_max_context_length(mut self, length: usize) -> Self {
        self.max_context_length = length;
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }
}

/// Stub Kalosm provider — always returns an error.
pub struct KalosmProvider {
    cache_key: Option<String>,
}

impl KalosmProvider {
    pub async fn new(_config: KalosmConfig) -> Result<Self, AgentError> {
        Err(AgentError::Llm(
            "Kalosm provider is deprecated and has been removed. \
             Please use LocalGgufProvider (via the `local-llm` feature) instead."
                .into(),
        ))
    }
}

#[async_trait::async_trait]
impl LlmProvider for KalosmProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        Err(AgentError::Llm(
            "Kalosm provider is deprecated. Use LocalGgufProvider instead.".into(),
        ))
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        Err(AgentError::Llm(
            "Kalosm provider is deprecated. Use LocalGgufProvider instead.".into(),
        ))
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.cache_key = Some(key.to_string());
    }

    fn capabilities(&self) -> crate::llm::api::ProviderCapabilities {
        crate::llm::api::ProviderCapabilities {
            native_tool_calling: true,
            ..Default::default()
        }
    }
}
