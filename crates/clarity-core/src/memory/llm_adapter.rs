//! LLM adapter bridging `clarity_contract::LlmProvider` to `clarity_memory::extractor::LlmClient`.

use async_trait::async_trait;
use clarity_contract::{LlmProvider, Message};
use std::sync::Arc;

/// `LlmProviderAdapter` configuration/state.
pub struct LlmProviderAdapter {
    provider: Arc<dyn LlmProvider>,
}

impl LlmProviderAdapter {
    /// Create a new `LlmProviderAdapter`.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl clarity_memory::extractor::LlmClient for LlmProviderAdapter {
    async fn complete(&self, prompt: &str, _model: &str) -> clarity_memory::Result<String> {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user(prompt),
        ];
        let tools = serde_json::json!({ "functions": [] });
        let response = self
            .provider
            .complete(&messages, &tools)
            .await
            .map_err(|e| clarity_memory::MemoryError::LlmClient(e.to_string()))?;
        Ok(response.content)
    }
}
