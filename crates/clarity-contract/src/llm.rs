//! LLM Provider trait and response types for the Clarity contract layer.
//!
//! These types define the interface between the Agent and LLM providers.
//! They are designed to be implementation-agnostic and shared across all
//! crates in the workspace.

use crate::{AgentError, StreamDelta, Message, ToolCall};
use async_trait::async_trait;
use serde_json::Value;

/// Provider self-reported capabilities.
///
/// Used by the agent runtime to decide whether to use native tool calling
/// APIs or fall back to prompt-guided tool invocation.
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    /// Whether the provider supports native `tools` parameter in the API.
    pub native_tool_calling: bool,
    /// Whether the provider supports vision / image inputs.
    pub vision: bool,
    /// Whether the provider supports prompt caching.
    pub prompt_caching: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            native_tool_calling: true,
            vision: false,
            prompt_caching: false,
        }
    }
}

/// Response from an LLM inference request.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The text content of the response
    pub content: String,
    /// Tool calls to execute (if any)
    pub tool_calls: Vec<ToolCall>,
    /// Whether this is the final response
    pub is_complete: bool,
}

/// LLM Provider trait — implement this to integrate with different LLMs.
///
/// This trait lives in the contract layer so that downstream crates
/// (egui, gateway, headless, plugins) can accept `dyn LlmProvider`
/// without depending on `clarity-core`.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a response from the LLM.
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError>;

    /// Stream the response as text chunks.
    ///
    /// Returns a receiver that yields chunks of the response.
    /// The receiver closes when the stream ends.
    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>;

    /// Set a prompt cache key for provider-side cache routing.
    fn set_prompt_cache_key(&mut self, key: &str);

    /// Provider self-reported capabilities.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: true,
            vision: false,
            prompt_caching: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(caps.native_tool_calling);
        assert!(!caps.vision);
        assert!(!caps.prompt_caching);
    }

    #[test]
    fn test_provider_capabilities_custom() {
        let caps = ProviderCapabilities {
            native_tool_calling: false,
            vision: true,
            prompt_caching: true,
        };
        assert!(!caps.native_tool_calling);
        assert!(caps.vision);
        assert!(caps.prompt_caching);
    }

    #[test]
    fn test_llm_provider_default_capabilities() {
        struct DummyProvider;
        #[async_trait]
        impl LlmProvider for DummyProvider {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<LlmResponse, AgentError> {
                unreachable!()
            }
            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
            {
                unreachable!()
            }
            fn set_prompt_cache_key(&mut self, _key: &str) {}
        }

        let provider = DummyProvider;
        let caps = provider.capabilities();
        assert!(caps.native_tool_calling);
        assert!(!caps.vision);
        assert!(!caps.prompt_caching);
    }
}
