//! Tool payload adapters for LLM providers without native tool calling support.

use clarity_contract::{Message, MessageRole};
use serde_json::Value;

/// Adapts tool payloads for LLM providers that don't support native tool calling.
///
/// Injects tool descriptions into the system prompt and clears the tools array so
/// the provider receives a prompt-guided representation instead of a structured
/// tool schema.
///
/// NOTE: Strips any existing `## Available Tools` markdown section (injected by
/// `SystemPromptBuilder`) before appending the XML dialect. This prevents the
/// tools from appearing twice in the same system message.
pub fn adapt_prompt_guided(messages: &[Message], tools: &Value) -> (Vec<Message>, Value) {
    let has_tools = tools.as_array().map(|a| !a.is_empty()).unwrap_or(false);

    if !has_tools {
        return (messages.to_vec(), tools.clone());
    }

    let tool_text = format_tools_for_prompt(tools);

    // ponytail: strip existing markdown `## Available Tools` section added by
    // SystemPromptBuilder so the XML version is the sole tool description.
    // The section ends at either `## Approval Mode` or `## Security Notice`.
    let adapted_messages: Vec<Message> = messages
        .iter()
        .map(|m| {
            if m.role == MessageRole::System {
                let cleaned = strip_markdown_tools_section(&m.content);
                Message {
                    role: MessageRole::System,
                    content: cleaned + &tool_text,
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

/// Remove any `## Available Tools\n...` markdown section from the system message.
///
/// The section spans from `## Available Tools` to the next `## ` heading
/// (typically `## Approval Mode` or `## Security Notice`).
fn strip_markdown_tools_section(content: &str) -> String {
    let marker = "## Available Tools\n";
    let Some(start) = content.find(marker) else {
        return content.to_string();
    };

    // Find the next `## ` heading after the tools section.
    let after_marker = start + marker.len();
    let end = content[after_marker..]
        .find("\n## ")
        .map(|p| after_marker + p)
        .unwrap_or(content.len());

    let mut cleaned = String::with_capacity(content.len());
    cleaned.push_str(&content[..start]);
    // Trim trailing newlines from the cut point to avoid blank lines.
    let tail = content[end..].trim_start_matches('\n');
    if !tail.is_empty() {
        cleaned.push('\n');
        cleaned.push_str(tail);
    }
    cleaned
}

// ponytail: minimal XML escaping for the tool-prompt dialect. Switch to a dedicated XML
/// library only if the project starts handling untrusted or complex XML documents.
/// Escape a string so it can be safely embedded inside XML attribute values or text nodes.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Format a list of tools as a text block for prompt-guided tool calling.
///
/// Uses an XML dialect that matches `clarity_core::agent::tool_parser::ToolFormat::Xml`,
/// which is the same fallback parser used by the agent loop when a provider returns
/// tool calls as plain text instead of native `tool_calls`.
fn format_tools_for_prompt(tools: &Value) -> String {
    let mut text = String::from(
        "\n\nYou have access to the following tools. \
         When you need to use a tool, output exactly one XML block on its own line \
         and then stop. Wait for the tool result before continuing.\n\n\
         Output format (you MUST use <arg key=...> tags for every parameter):\n\
         <tool name=\"tool_name\">\n\
           <arg key=\"arg_name\">arg_value</arg>\n\
         </tool>\n\n\
         Available tools:\n",
    );

    let mut first_example: Option<(String, String)> = None;

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
                text.push_str(&format!(
                    "<tool_description name=\"{}\" description=\"{}\">\n",
                    xml_escape(name),
                    xml_escape(desc)
                ));

                let mut first_required_arg: Option<String> = None;
                if let Some(params) = func.get("parameters") {
                    let required: Vec<&str> = params
                        .get("required")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                        .unwrap_or_default();

                    if let Some(props) = params.get("properties").and_then(|v| v.as_object()) {
                        for (param_name, param_schema) in props {
                            let ptype = param_schema
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("any");
                            let pdesc = param_schema
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let is_required = required.contains(&param_name.as_str());
                            text.push_str(&format!(
                                "  <parameter name=\"{}\" type=\"{}\" required=\"{}\">{}</parameter>\n",
                                xml_escape(param_name),
                                xml_escape(ptype),
                                is_required,
                                xml_escape(pdesc)
                            ));
                            if is_required && first_required_arg.is_none() {
                                first_required_arg = Some(param_name.clone());
                            }
                        }
                    }
                }

                text.push_str("</tool_description>\n");

                if first_example.is_none() {
                    if let Some(arg) = first_required_arg {
                        first_example = Some((name.to_string(), arg));
                    }
                }
            }
        }
    }

    if let Some((name, arg)) = first_example {
        text.push_str(&format!(
            "\n\
             Example call for {name}:\n\
             <tool name=\"{name}\">\n\
               <arg key=\"{arg}\">value</arg>\n\
             </tool>\n",
            name = xml_escape(&name),
            arg = xml_escape(&arg)
        ));
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tools() -> Value {
        json!([
            {
                "type": "function",
                "function": {
                    "name": "powershell",
                    "description": "Execute a PowerShell command.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "The command to execute"
                            },
                            "timeout": {
                                "type": "integer",
                                "description": "Timeout in seconds"
                            }
                        },
                        "required": ["command"]
                    }
                }
            }
        ])
    }

    #[test]
    fn test_adapt_prompt_guided_injects_into_system_only() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("List files."),
        ];
        let (adapted, adapted_tools) = adapt_prompt_guided(&messages, &sample_tools());

        assert_eq!(adapted.len(), 2);
        assert!(
            adapted[0]
                .content
                .contains("<tool_description name=\"powershell\"")
        );
        assert!(adapted[0].content.contains("<arg key=\"arg_name\">"));
        assert!(!adapted[1].content.contains("tool_description"));
        assert!(adapted_tools.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_format_tools_xml_escaping() {
        let tools = json!([
            {
                "type": "function",
                "function": {
                    "name": "bad&name",
                    "description": "Use <script> \"quotes\"",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path with <>& chars"
                            }
                        },
                        "required": ["path"]
                    }
                }
            }
        ]);
        let text = format_tools_for_prompt(&tools);
        assert!(text.contains("name=\"bad&amp;name\""));
        assert!(text.contains("description=\"Use &lt;script&gt; &quot;quotes&quot;\""));
        assert!(text.contains("&gt;"));
    }

    #[test]
    fn test_no_tools_returns_unchanged() {
        let messages = vec![Message::system("Hi")];
        let (adapted, tools) = adapt_prompt_guided(&messages, &json!([]));
        assert_eq!(adapted[0].content, "Hi");
        assert!(tools.as_array().unwrap().is_empty());
    }

    // ── strip_markdown_tools_section tests ──

    #[test]
    fn strip_removes_tools_section_before_approval_mode() {
        let input = "Base prompt.\n\n## Available Tools\n- plan: ...\n- read: ...\n\n## Approval Mode\nYOLO mode\n\n## Security Notice\nBe careful.";
        let result = strip_markdown_tools_section(input);
        assert!(!result.contains("## Available Tools"));
        assert!(!result.contains("- plan:"));
        assert!(result.contains("## Approval Mode"));
        assert!(result.contains("## Security Notice"));
        assert!(result.contains("Base prompt."));
    }

    #[test]
    fn strip_no_tools_section_returns_unchanged() {
        let input = "Base prompt.\n\n## Approval Mode\nYOLO";
        let result = strip_markdown_tools_section(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_tools_at_end_of_message() {
        let input = "Base prompt.\n\n## Available Tools\n- only tool";
        let result = strip_markdown_tools_section(input);
        assert!(result.contains("Base prompt."));
        assert!(!result.contains("## Available Tools"));
        assert!(!result.contains("- only tool"));
    }

    #[test]
    fn adapt_prompt_guided_no_duplicate_tools() {
        // Simulate a system message that already has markdown tools from SystemPromptBuilder.
        let messages = vec![
            Message::system(
                "You are helpful.\n\n## Available Tools\n- read: Read a file\n\n## Approval Mode\nYOLO",
            ),
            Message::user("Query"),
        ];
        let (adapted, _) = adapt_prompt_guided(&messages, &sample_tools());
        let system_content = &adapted[0].content;
        // Should NOT have the markdown tools section.
        assert!(
            !system_content.contains("## Available Tools\n- read"),
            "markdown tools section should be stripped"
        );
        // Should have the XML tools.
        assert!(
            system_content.contains("<tool_description"),
            "XML tools should be present"
        );
        // Should preserve other sections.
        assert!(system_content.contains("## Approval Mode"));
        assert!(system_content.contains("YOLO"));
        assert!(system_content.contains("You are helpful."));
    }
}
