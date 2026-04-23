//! Bridge between `clarity_core::llm::LlmProvider` and `clarity_memory::extractor::LlmClient`.

use crate::llm::api::LlmProvider;
use crate::llm::Message as LlmMessage;
use async_trait::async_trait;
use std::sync::Arc;

/// Bridges the core `LlmProvider` trait to the memory system's `LlmClient` trait.
///
/// The memory system's `MemoryCompiler` and `FactExtractor` expect a simple
/// `complete(prompt, model)` interface. This adapter wraps the core provider
/// (which expects a message list and tools JSON) into that simpler shape.
pub struct LlmProviderBridge {
    provider: Arc<dyn LlmProvider>,
}

impl LlmProviderBridge {
    /// Create a new bridge around the given LLM provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl clarity_memory::extractor::LlmClient for LlmProviderBridge {
    async fn complete(&self, prompt: &str, _model: &str) -> clarity_memory::Result<String> {
        let messages = vec![LlmMessage::user(prompt)];
        let tools = serde_json::Value::Null;

        match self.provider.complete(&messages, &tools).await {
            Ok(response) => {
                // `LlmResponse` is a struct with `content` and `tool_calls`.
                // During memory compilation we only care about the text content.
                Ok(response.content)
            }
            Err(e) => Err(clarity_memory::MemoryError::LlmClient(format!(
                "LLM provider error during memory compilation: {}",
                e
            ))),
        }
    }
}
