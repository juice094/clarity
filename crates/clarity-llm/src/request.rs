//! OpenAI-compatible chat-completion request types and size guards.
//!
//! This module holds the JSON request/response shapes used by the generic
//! OpenAI-compatible provider, plus byte-budget helpers that drop or truncate
//! content before it reaches providers with hard body-size limits.

use crate::api::{Message, MessageRole};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum content bytes to send in a single LLM request.
///
/// Providers such as DeepSeek and Kimi reject request bodies larger than ~2MB.
/// We budget 1.5MB for message content to leave room for JSON framing, tools,
/// and provider metadata.
pub(crate) const MAX_MESSAGE_BODY_BYTES: usize = 1_500_000;

/// Maximum serialized JSON bytes for the `tools` field. MCP servers can return
/// very large input schemas; if the tools payload exceeds this, we drop tools
/// for this request rather than risk a 413 from the provider.
pub(crate) const MAX_TOOLS_JSON_BYTES: usize = 300_000;

/// Maximum serialized JSON bytes for the entire request body.
///
/// Providers such as DeepSeek and Kimi reject bodies larger than ~2MB. We leave
/// a small margin for JSON framing and provider metadata.
pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 2_000_000;

/// Structured output / JSON mode configuration.
///
/// Mirrors OpenAI's `response_format` parameter. When set, the provider
/// constrains the model to produce output matching the given format.
/// Providers that do not support JSON mode ignore this field.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ResponseFormat {
    /// Standard JSON object mode (no schema enforcement).
    #[serde(rename = "json_object")]
    JsonObject,
    /// Strict JSON Schema enforcement (OpenAI structured-outputs, DeepSeek, etc.).
    #[serde(rename = "json_schema")]
    JsonSchema {
        /// The JSON Schema that the model output must conform to.
        json_schema: JsonSchemaSpec,
    },
}

/// Schema specification for structured output mode.
#[derive(Debug, Clone, Serialize)]
pub struct JsonSchemaSpec {
    /// Schema name (provider-visible, max 64 chars).
    pub name: String,
    /// JSON Schema definition describing the expected output shape.
    #[serde(rename = "schema")]
    pub schema: Value,
    /// Whether the provider should enforce strict schema adherence.
    /// When `true`, the model must produce output that exactly matches the
    /// schema; when `false` or `None`, the schema is advisory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    /// Human-readable description for the schema (optional, provider-dependent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ResponseFormat {
    /// Create a JSON Object mode request (no schema).
    pub fn json_object() -> Self {
        Self::JsonObject
    }

    /// Create a strict JSON Schema mode request.
    pub fn json_schema(name: impl Into<String>, schema: Value, strict: bool) -> Self {
        Self::JsonSchema {
            json_schema: JsonSchemaSpec {
                name: name.into(),
                schema,
                strict: if strict { Some(true) } else { None },
                description: None,
            },
        }
    }
}

/// Truncate message history to fit within a byte budget while preserving the
/// system prompt and the most recent user/assistant exchanges.
///
/// This is a last-line-of-defense guard for providers that enforce a maximum
/// request body size. It drops oldest non-system messages until the total
/// content size is below `max_bytes`, always keeping the final user message.
pub(crate) fn truncate_messages_by_bytes(messages: &[Message], max_bytes: usize) -> Vec<Message> {
    let total: usize = messages.iter().map(|m| m.content.len()).sum();
    if total <= max_bytes {
        return messages.to_vec();
    }

    let mut result = messages.to_vec();

    // Preserve the final user message index if present; otherwise the last message.
    let mut last_user = result
        .iter()
        .rposition(|m| m.role == MessageRole::User)
        .unwrap_or_else(|| result.len().saturating_sub(1));

    // First pass: drop oldest non-system messages until we fit or only system + last remain.
    while result.len() > 1 {
        let current: usize = result.iter().map(|m| m.content.len()).sum();
        if current <= max_bytes {
            break;
        }
        let Some(remove_idx) = result
            .iter()
            .position(|m| m.role != MessageRole::System)
            .filter(|&i| i != last_user)
        else {
            break;
        };
        result.remove(remove_idx);
        if remove_idx < last_user {
            last_user -= 1;
        }
    }

    // Last pass: if the budget is still blown, truncate the first system message.
    let current: usize = result.iter().map(|m| m.content.len()).sum();
    if current > max_bytes {
        if let Some(sys) = result.iter_mut().find(|m| m.role == MessageRole::System) {
            let non_system: usize = current - sys.content.len();
            let budget = max_bytes.saturating_sub(non_system);
            sys.content = truncate_to_bytes_llm(&sys.content, budget);
        }
    }

    result
}

// ponytail: manual UTF-8 boundary scan. Replace with str::floor_char_boundary once MSRV >= 1.91.
/// Find the largest valid UTF-8 boundary at or before `byte_idx`.
pub(crate) fn floor_char_boundary_llm(text: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(text.len());
    let mut idx = byte_idx;
    while idx > 0 && text.as_bytes()[idx] & 0b1100_0000 == 0b1000_0000 {
        idx -= 1;
    }
    idx
}

/// Truncate `text` to at most `max_bytes` UTF-8 bytes, appending a marker.
pub(crate) fn truncate_to_bytes_llm(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let marker = "\n\n...[truncated]";
    let budget = max_bytes.saturating_sub(marker.len());
    let split = floor_char_boundary_llm(text, budget);
    format!("{}{}", &text[..split], marker)
}

/// Drop the tools payload if its serialized JSON exceeds the byte budget.
pub(crate) fn cap_tools_json(tools_opt: Option<Value>) -> Option<Value> {
    match tools_opt {
        Some(ref tools) => {
            let size = serde_json::to_string(tools).unwrap_or_default().len();
            if size > MAX_TOOLS_JSON_BYTES {
                tracing::warn!(
                    "Tools JSON exceeds {} bytes ({} bytes); dropping tools for this request",
                    MAX_TOOLS_JSON_BYTES,
                    size
                );
                None
            } else {
                tools_opt
            }
        }
        None => None,
    }
}

/// Final guard: if the serialized request body is still over the provider limit,
/// drop tools and log. We do not silently truncate messages here to avoid
/// surprising the caller; message truncation already happened earlier.
pub(crate) fn guard_request_body_size(request_body: &mut ChatCompletionRequest) {
    let body_size = serde_json::to_string(request_body).map_or(0, |s| s.len());
    tracing::debug!(
        body_bytes = body_size,
        messages = request_body.messages.len(),
        tools_present = request_body.tools.is_some(),
        "LLM request prepared"
    );
    if body_size > MAX_REQUEST_BODY_BYTES {
        tracing::warn!(
            body_bytes = body_size,
            max_bytes = MAX_REQUEST_BODY_BYTES,
            "Request body exceeds budget; dropping tools"
        );
        request_body.tools = None;
        let body_size = serde_json::to_string(request_body).map_or(0, |s| s.len());
        if body_size > MAX_REQUEST_BODY_BYTES {
            tracing::error!(
                body_bytes = body_size,
                max_bytes = MAX_REQUEST_BODY_BYTES,
                "Request body still exceeds budget after dropping tools; provider will likely reject"
            );
        }
    }
}

/// OpenAI chat-completion request body.
#[derive(Debug, Serialize)]
pub(crate) struct ChatCompletionRequest {
    pub(crate) model: String,
    pub(crate) messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) thinking: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) response_format: Option<Value>,
}

/// Single message in an OpenAI chat-completion request.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiMessage {
    pub(crate) role: String,
    pub(crate) content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) reasoning_content: Option<String>,
}

/// Tool call as represented in an OpenAI chat-completion response.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ApiToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) call_type: String,
    pub(crate) function: ApiFunctionCall,
}

/// Function payload inside an [`ApiToolCall`].
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ApiFunctionCall {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

/// OpenAI chat-completion (non-streaming) response.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatCompletionResponse {
    pub(crate) choices: Vec<Choice>,
}

/// One choice from a chat-completion response.
#[derive(Debug, Deserialize)]
pub(crate) struct Choice {
    pub(crate) message: ApiMessage,
    // Intentionally retained: `finish_reason` is part of the OpenAI chat-completion
    // response schema and may be used for debugging / telemetry in the future.
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) finish_reason: Option<String>,
}
