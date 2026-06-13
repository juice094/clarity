//! LLM Provider trait and response types for the Clarity contract layer.
//!
//! These types define the interface between the Agent and LLM providers.
//! They are designed to be implementation-agnostic and shared across all
//! crates in the workspace.

use crate::{AgentError, Message, StreamDelta, ToolCall};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Pricing info for cost estimation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Pricing {
    /// Price per 1M input tokens in USD.
    pub input_per_1m: f64,
    /// Price per 1M output tokens in USD.
    pub output_per_1m: f64,
}

impl Pricing {
    /// Estimate cost in USD for a given number of prompt and completion tokens.
    ///
    /// Costs are computed from per-1M-token rates.
    pub fn estimate_cost(&self, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        (prompt_tokens as f64 * self.input_per_1m + completion_tokens as f64 * self.output_per_1m)
            / 1_000_000.0
    }
}

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
    /// Optional pricing info for cost estimation.
    pub pricing: Option<Pricing>,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            native_tool_calling: true,
            vision: false,
            prompt_caching: false,
            pricing: None,
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
    fn set_prompt_cache_key(&self, key: &str);

    /// Clear any provider-side cache (e.g., local KV cache).
    /// Default is a no-op; providers with local state should override.
    fn clear_cache(&self) {}

    /// Provider self-reported capabilities.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: true,
            vision: false,
            prompt_caching: false,
            pricing: None,
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
        assert!(caps.pricing.is_none());
    }

    #[test]
    fn test_provider_capabilities_custom() {
        let caps = ProviderCapabilities {
            native_tool_calling: false,
            vision: true,
            prompt_caching: true,
            pricing: Some(Pricing {
                input_per_1m: 1.0,
                output_per_1m: 2.0,
            }),
        };
        assert!(!caps.native_tool_calling);
        assert!(caps.vision);
        assert!(caps.prompt_caching);
        assert_eq!(caps.pricing.unwrap().input_per_1m, 1.0);
    }

    #[test]
    fn test_pricing_estimate_cost() {
        let pricing = Pricing {
            input_per_1m: 3.0,
            output_per_1m: 15.0,
        };
        // 1M input + 0 output = $3.0
        assert!((pricing.estimate_cost(1_000_000, 0) - 3.0).abs() < f64::EPSILON);
        // 0 input + 1M output = $15.0
        assert!((pricing.estimate_cost(0, 1_000_000) - 15.0).abs() < f64::EPSILON);
        // 500k input + 200k output = 1.5 + 3.0 = 4.5
        assert!((pricing.estimate_cost(500_000, 200_000) - 4.5).abs() < f64::EPSILON);
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
            fn set_prompt_cache_key(&self, _key: &str) {}
        }

        let provider = DummyProvider;
        let caps = provider.capabilities();
        assert!(caps.native_tool_calling);
        assert!(!caps.vision);
        assert!(!caps.prompt_caching);
        assert!(caps.pricing.is_none());
    }
}
