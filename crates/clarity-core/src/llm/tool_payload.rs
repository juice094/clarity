//! Tool payload adapters for LLM providers without native tool calling support.

use clarity_contract::{Message, MessageRole};
use serde_json::Value;

/// Adapts tool payloads for LLM providers that don't support native tool calling.
pub trait ToolPayloadAdapter: Send + Sync {
    /// Modify messages and tools for provider consumption.
    /// Returns (adapted_messages, adapted_tools).
    fn adapt(&self, messages: &[Message], tools: &Value) -> (Vec<Message>, Value);
}

/// Pass-through adapter for providers with native tool calling support.
pub struct NativeToolAdapter;

impl ToolPayloadAdapter for NativeToolAdapter {
    fn adapt(&self, messages: &[Message], tools: &Value) -> (Vec<Message>, Value) {
        (messages.to_vec(), tools.clone())
    }
}

/// Injects tool descriptions into the system prompt for prompt-guided providers.
pub struct PromptGuidedAdapter;

impl ToolPayloadAdapter for PromptGuidedAdapter {
    fn adapt(&self, messages: &[Message], tools: &Value) -> (Vec<Message>, Value) {
        let has_tools = tools.as_array().map(|a| !a.is_empty()).unwrap_or(false);

        if !has_tools {
            return (messages.to_vec(), tools.clone());
        }

        let tool_text = format_tools_for_prompt(tools);

        let adapted_messages: Vec<Message> = messages
            .iter()
            .map(|m| {
                if m.role == MessageRole::System {
                    Message {
                        role: MessageRole::System,
                        content: m.content.clone() + &tool_text,
                        tool_calls: m.tool_calls.clone(),
                        tool_call_id: m.tool_call_id.clone(),
                    }
                } else {
                    m.clone()
                }
            })
            .collect();

        (adapted_messages, Value::Array(vec![]))
    }
}

/// Format a list of tools as a text block for prompt-guided tool calling.
fn format_tools_for_prompt(tools: &Value) -> String {
    let mut text = String::from(
        "\n\nYou have access to the following tools. \
         When you need to use a tool, output a JSON object in this exact format on its own line:\n\
         {\"tool_calls\": [{\"id\": \"call_1\", \"type\": \"function\", \"function\": {\"name\": \"tool_name\", \"arguments\": {\"arg1\": \"value1\"}}}]\n\n\
         Available tools:\n"
    );
    if let Some(arr) = tools.as_array() {
        for tool in arr {
            if let Some(func) = tool.get("function") {
                let name = func
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let desc = func
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                text.push_str(&format!("- {}: {}\n", name, desc));
            }
        }
    }
    text
}
