//! Anthropic Messages API adapter over any `LlmProvider`.

use super::prompt::{build_prompt, strip_tool_xml};
use super::tools::convert_tools;
use super::types::{AnthropicRequest, AnthropicResponse, ResponseBlock, SystemContent, Usage};
use clarity_contract::{AgentError, LlmProvider, LlmResponse, Message, tool_parser};
use serde_json::Value;
use std::sync::Arc;

/// Adapter that exposes an Anthropic Messages API-compatible interface over an
/// arbitrary `LlmProvider` backend.
///
/// The adapter is stateless: each `complete` call constructs a fresh prompt
/// from the Anthropic request and delegates inference to the wrapped provider.
/// It is the caller's responsibility to reset any provider-side conversation
/// context if needed.
#[derive(Clone)]
pub struct AnthropicAdapter {
    provider: Arc<dyn LlmProvider>,
}

impl AnthropicAdapter {
    /// Create a new adapter wrapping the given provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Process an Anthropic request and return an Anthropic-formatted response.
    ///
    /// # Arguments
    ///
    /// * `req` - The deserialized Anthropic Messages API request.
    ///
    /// # Errors
    ///
    /// Returns `AgentError` if the underlying provider call fails.
    pub async fn complete(&self, req: AnthropicRequest) -> Result<AnthropicResponse, AgentError> {
        let model = req.model.as_deref().unwrap_or("deepseek-chat").to_string();
        let system = extract_system_text(&req.system);
        let tools_clarity = convert_tools(&req.tools);
        let prompt = build_prompt(&req.messages, &system);

        let clarity_messages = if let Some(sys) = &system {
            vec![Message::system(sys.clone()), Message::user(prompt)]
        } else {
            vec![Message::user(prompt)]
        };

        let input_tokens = clarity_messages
            .iter()
            .map(|m| m.content.len() as u32 / 4)
            .sum();

        let llm_response = self
            .provider
            .complete(&clarity_messages, &tools_clarity)
            .await?;

        let (content_blocks, stop_reason) =
            build_response_content(&llm_response, &req.messages, &req.system);

        Ok(AnthropicResponse {
            id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            model,
            stop_reason,
            stop_sequence: None,
            content: content_blocks,
            usage: Usage {
                input_tokens,
                output_tokens: (llm_response.content.len() / 4) as u32,
            },
        })
    }
}

/// Extract the system prompt text from the request.
fn extract_system_text(sys: &Option<SystemContent>) -> Option<String> {
    super::types::extract_system_text(sys)
}

/// Build Anthropic response content blocks and stop reason from an LLM response.
fn build_response_content(
    llm_response: &LlmResponse,
    _messages: &[super::types::AnthropicMessage],
    _system: &Option<SystemContent>,
) -> (Vec<ResponseBlock>, String) {
    // Parse XML tool calls from response.
    let tool_calls = if tool_parser::detect_tool_format(&llm_response.content)
        == Some(tool_parser::ToolFormat::Xml)
    {
        tool_parser::parse_tool_calls(&llm_response.content, tool_parser::ToolFormat::Xml)
    } else {
        vec![]
    };

    let clean_text = strip_tool_xml(&llm_response.content);

    let mut content_blocks: Vec<ResponseBlock> = Vec::new();
    if !clean_text.is_empty() {
        content_blocks.push(ResponseBlock::Text { text: clean_text });
    }
    for tc in &tool_calls {
        let input = serde_json::from_str(&tc.function.arguments).unwrap_or(Value::Null);
        content_blocks.push(ResponseBlock::ToolUse {
            id: tc.id.clone(),
            name: tc.function.name.clone(),
            input,
        });
    }

    let stop_reason = if tool_calls.is_empty() {
        "end_turn".to_string()
    } else {
        "tool_use".to_string()
    };

    (content_blocks, stop_reason)
}

/// Estimate input tokens for an Anthropic request body string.
pub fn estimate_request_tokens(body: &str) -> u32 {
    (body.len() / 4) as u32
}
