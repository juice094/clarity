//! LLM API types — shared contracts for LLM communication
//!
//! These types define the interface between the Agent and LLM providers.
//! They are kept separate from `agent/mod.rs` to avoid circular dependencies
//! (e.g., `llm/`, `compaction/`, and `subagents/` should not depend on `agent/`).

use crate::error::AgentError;
use crate::types::ToolCall;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// LLM message role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a tool response message
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Delta emitted by a streaming LLM response.
#[derive(Debug, Clone, Default)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

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
