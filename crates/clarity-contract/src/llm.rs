//! LLM Provider trait and response types for the Clarity contract layer.
//!
//! These types define the interface between the Agent and LLM providers.
//! They are designed to be implementation-agnostic and shared across all
//! crates in the workspace.

use crate::{AgentError, StreamDelta, Message, ToolCall};
use async_trait::async_trait;
use serde_json::Value;

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
}
