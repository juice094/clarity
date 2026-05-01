//! LLM API types — shared contracts for LLM communication
//!
//! These types define the interface between the Agent and LLM providers.
//! They are kept separate from `agent/mod.rs` to avoid circular dependencies
//! (e.g., `llm/`, `compaction/`, and `subagents/` should not depend on `agent/`).
//!
//! ## Type origin
//!
//! `Message`, `MessageRole`, and `StreamDelta` are defined in `clarity-contract`
//! and re-exported here for backward compatibility. New code should import
//! directly from `clarity_contract`.

use crate::error::AgentError;
use async_trait::async_trait;
use serde_json::Value;

// Re-export contract types so existing imports continue to work.
pub use clarity_contract::{Message, MessageRole, StreamDelta, ToolCall};

/// Response from an LLM
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The text content of the response
    pub content: String,
    /// Tool calls to execute (if any)
    pub tool_calls: Vec<ToolCall>,
    /// Whether this is the final response
    pub is_complete: bool,
}

/// LLM Provider trait - implement this to integrate with different LLMs
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a response from the LLM
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError>;

    /// Stream the response as text chunks.
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
