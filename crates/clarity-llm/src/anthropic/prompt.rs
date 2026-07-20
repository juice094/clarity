//! Prompt construction and text extraction for the Anthropic adapter.

use super::types::{AnthropicContent, AnthropicMessage};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

/// Serialize the full Anthropic conversation into a single prompt string.
///
/// The resulting prompt is suitable for providers that expect a flat
/// `system / user / assistant` text rather than structured message lists.
pub fn build_prompt(messages: &[AnthropicMessage], system: &Option<String>) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(sys) = system {
        parts.push(format!("System: {}", sys));
    }

    for msg in messages {
        let label = match msg.role.as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            _ => continue,
        };
        parts.push(format!("{}: {}", label, content_to_text(&msg.content)));
    }

    parts.join("\n\n")
}

/// Extract readable text from Anthropic content blocks.
///
/// Tool-use and tool-result blocks are serialized into a compact textual
/// representation so that downstream providers can reason about the
/// conversation history without native tool-calling support.
pub fn content_to_text(content: &AnthropicContent) -> String {
    match content {
        AnthropicContent::Text(s) => s.clone(),
        AnthropicContent::Blocks(blocks) => {
            let mut lines = Vec::new();
            for b in blocks {
                match b {
                    super::types::AnthropicBlock::Text { text } => lines.push(text.clone()),
                    super::types::AnthropicBlock::ToolUse { id, name, input } => {
                        lines.push(format!(
                            "[Tool Call: {name} id={id}] {}",
                            serde_json::to_string(input).unwrap_or_default()
                        ));
                    }
                    super::types::AnthropicBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let prefix = if is_error.unwrap_or(false) {
                            "[Tool Error"
                        } else {
                            "[Tool Result"
                        };
                        let text = content
                            .as_ref()
                            .and_then(|c| c.as_str().map(String::from))
                            .unwrap_or_default();
                        lines.push(format!("{prefix} id={tool_use_id}]: {text}"));
                    }
                    super::types::AnthropicBlock::Thinking { .. }
                    | super::types::AnthropicBlock::RedactedThinking { .. }
                    | super::types::AnthropicBlock::Unknown => {
                        // Internal reasoning or unknown blocks are not conversation content.
                    }
                }
            }
            lines.join("\n")
        }
    }
}

/// Strip XML `<tool>` and `<function_calls>` blocks from text.
///
/// After tool calls have been parsed and emitted as separate response blocks,
/// the remaining text should not contain the raw XML markup.
pub fn strip_tool_xml(content: &str) -> String {
    // SAFE: compile-time literal regex.
    #[allow(clippy::expect_used)]
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<tool\s[^>]*>.*?</tool>|<function_calls>.*?</function_calls>")
            .expect("strip regex")
    });
    let cleaned = RE.replace_all(content, "");
    // Collapse 3+ blank lines.
    // SAFE: compile-time literal regex.
    #[allow(clippy::expect_used)]
    let collapsed = Regex::new(r"\n{3,}")
        .expect("collapse regex")
        .replace_all(&cleaned, "\n\n");
    collapsed.trim().to_string()
}

/// Estimate token count from a JSON-serializable request body.
///
/// This is a rough heuristic (1 token per 4 characters) matching the previous
/// proxy behavior. It is not accurate for any specific tokenizer.
pub fn estimate_body_tokens(body: &str) -> u32 {
    (body.len() / 4) as u32
}

/// Convert an arbitrary tool input value into a JSON string.
///
/// Falls back to a JSON-encoded `{ "content": ... }` wrapper for non-JSON
/// values, preserving the original text.
pub fn tool_input_to_json(input: &Value) -> Value {
    match input {
        Value::Object(_) | Value::Array(_) => input.clone(),
        Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                parsed
            } else {
                serde_json::json!({ "content": s })
            }
        }
        other => serde_json::json!({ "content": other.to_string() }),
    }
}
