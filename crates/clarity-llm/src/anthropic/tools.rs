//! Anthropic tool definition conversion.

use super::types::AnthropicTool;
use serde_json::Value;

/// Convert Anthropic tool definitions to OpenAI-style function JSON.
///
/// Providers in the Clarity ecosystem consume tools as a JSON array of
/// `{"type": "function", "function": { ... }}` objects. This function maps
/// Anthropic's `input_schema` to the `parameters` field of that representation.
pub fn convert_tools(anthropic_tools: &[AnthropicTool]) -> Value {
    Value::Array(
        anthropic_tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema
                    }
                })
            })
            .collect(),
    )
}
