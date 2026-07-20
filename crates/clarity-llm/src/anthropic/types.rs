//! Anthropic Messages API request/response types.
//!
//! These types mirror the Anthropic Messages API JSON shape consumed by clients
//! such as Claude Code. They are intentionally minimal and focused on the subset
//! needed to translate Anthropic requests into `clarity_contract::LlmProvider`
//! calls and back again.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Top-level Anthropic Messages API request.
#[derive(Debug, Deserialize)]
pub struct AnthropicRequest {
    /// Requested model identifier (e.g. `claude-sonnet-4-6`).
    ///
    /// The adapter ignores this when selecting the backend provider; it is only
    /// echoed back in the response.
    #[serde(default)]
    pub model: Option<String>,
    /// Maximum tokens to generate. Currently unused by the adapter.
    #[allow(dead_code)]
    pub max_tokens: u32,
    /// Conversation messages.
    pub messages: Vec<AnthropicMessage>,
    /// Tool definitions in Anthropic format.
    #[serde(default)]
    pub tools: Vec<AnthropicTool>,
    /// Whether the client requested streaming. The adapter currently returns a
    /// single non-streaming response even when this is `true`.
    #[serde(default)]
    pub stream: bool,
    /// System prompt content, either plain text or a list of text blocks.
    #[serde(default)]
    pub system: Option<SystemContent>,
}

/// System prompt content: plain text or structured blocks.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SystemContent {
    /// Plain string system prompt.
    Text(String),
    /// List of text blocks.
    Blocks(Vec<SystemTextBlock>),
}

/// A single text block inside a structured system prompt.
#[derive(Debug, Deserialize)]
pub struct SystemTextBlock {
    /// Text content of the block.
    pub text: String,
}

/// Extract a single system prompt string from structured system content.
pub fn extract_system_text(sys: &Option<SystemContent>) -> Option<String> {
    match sys {
        Some(SystemContent::Text(s)) => Some(s.clone()),
        Some(SystemContent::Blocks(blocks)) => {
            let text: String = blocks
                .iter()
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() { None } else { Some(text) }
        }
        None => None,
    }
}

/// A single message in the Anthropic conversation.
#[derive(Debug, Deserialize)]
pub struct AnthropicMessage {
    /// Message role: `user` or `assistant`.
    pub role: String,
    /// Message content: either a plain string or a list of content blocks.
    pub content: AnthropicContent,
}

/// Anthropic message content: plain text or structured blocks.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    /// Plain string content.
    Text(String),
    /// List of content blocks.
    Blocks(Vec<AnthropicBlock>),
}

/// A single content block inside an Anthropic message.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicBlock {
    /// Plain text block.
    #[serde(rename = "text")]
    Text {
        /// Text content.
        text: String,
    },
    /// Assistant-requested tool call.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Tool call identifier.
        id: String,
        /// Tool name.
        name: String,
        /// Tool arguments.
        input: Value,
    },
    /// Result returned from a tool execution.
    #[serde(rename = "tool_result")]
    ToolResult {
        /// Identifier of the tool call this result corresponds to.
        tool_use_id: String,
        /// Tool result content, if any.
        #[serde(default)]
        content: Option<Value>,
        /// Whether the tool result represents an error.
        #[serde(default)]
        is_error: Option<bool>,
    },
    /// Visible reasoning block.
    #[serde(rename = "thinking")]
    #[allow(dead_code)]
    Thinking {
        /// Reasoning text.
        thinking: String,
    },
    /// Redacted reasoning block.
    #[serde(rename = "redacted_thinking")]
    #[allow(dead_code)]
    RedactedThinking {
        /// Opaque redacted reasoning data.
        data: String,
    },
    /// Catch-all for unknown content block types.
    #[serde(other)]
    Unknown,
}

/// Anthropic tool definition.
#[derive(Debug, Deserialize)]
pub struct AnthropicTool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    #[serde(default)]
    pub description: String,
    /// JSON schema describing the tool's input.
    pub input_schema: Value,
}

/// Top-level Anthropic Messages API response.
#[derive(Debug, Serialize)]
pub struct AnthropicResponse {
    /// Response identifier.
    pub id: String,
    /// Response type (`message`).
    #[serde(rename = "type")]
    pub response_type: String,
    /// Message role (`assistant`).
    pub role: String,
    /// Model identifier echoed from the request.
    pub model: String,
    /// Response content blocks.
    pub content: Vec<ResponseBlock>,
    /// Reason the model stopped.
    pub stop_reason: String,
    /// Stop sequence, if any.
    pub stop_sequence: Option<String>,
    /// Token usage estimate.
    pub usage: Usage,
}

/// A single content block in an Anthropic response.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ResponseBlock {
    /// Plain text block.
    #[serde(rename = "text")]
    Text {
        /// Text content.
        text: String,
    },
    /// Tool use block.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Tool call identifier.
        id: String,
        /// Tool name.
        name: String,
        /// Tool arguments.
        input: Value,
    },
}

/// Token usage estimate.
#[derive(Debug, Serialize)]
pub struct Usage {
    /// Estimated input tokens.
    pub input_tokens: u32,
    /// Estimated output tokens.
    pub output_tokens: u32,
}
