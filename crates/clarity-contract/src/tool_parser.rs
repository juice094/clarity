//! Parse tool calls from LLM response content.
//!
//! Supports four formats provider-agnostically: JSON, XML, MiniMax, Perl.
//! Used by the agent loop and protocol adapters to extract tool invocations
//! from providers that do not emit native tool-calling structures.

use crate::{FunctionCall, ToolCall};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

// Module-level lazy regexes (avoid regex_creation_in_loops false positives).
// The patterns are compile-time literals; expect is safe because they are valid regexes.
// Tag names mirror clarity_contract::tool_format constants (XML_TOOL_TAG, XML_ARG_TAG, etc.).
#[allow(clippy::expect_used)]
static RE_TOOL: LazyLock<Regex> = LazyLock::new(|| {
    // Matches both self-closing <tool name="..." arg="..."/> and
    // container-style <tool name="...">...</tool>.
    Regex::new(r#"(?s)<tool\s+name=["']([^"']+)["']\s*([^>]*?)(?:>(.*?)</tool>|/>)"#)
        .expect("RE_TOOL regex is valid")
});
#[allow(clippy::expect_used)]
static RE_ARG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)<(?:arg|parameter)\s+(?:key|name)=["']([^"']+)["'][^>]*>(.*?)</(?:arg|parameter)>"#,
    )
    .expect("RE_ARG regex is valid")
});
#[allow(clippy::expect_used)]
static RE_INVOKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<invoke\s+name=["']([^"']+)["'][^>]*>(.*?)</invoke>"#)
        .expect("RE_INVOKE regex is valid")
});
#[allow(clippy::expect_used)]
static RE_PARAM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<parameter\s+name=["']([^"']+)["'][^>]*>(.*?)</parameter>"#)
        .expect("RE_PARAM regex is valid")
});
#[allow(clippy::expect_used)]
static RE_MINIMAX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)```(\w+)\s*\n(.*?)\n```").expect("RE_MINIMAX regex is valid")
});
#[allow(clippy::expect_used)]
static RE_GENERIC_ARG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<([a-zA-Z_][a-zA-Z0-9_\-]*)>(.*?)</([a-zA-Z_][a-zA-Z0-9_\-]*)>"#)
        .expect("RE_GENERIC_ARG regex is valid")
});
#[allow(clippy::expect_used)]
static RE_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([a-zA-Z_][a-zA-Z0-9_\-]*)\s*=\s*["']([^"']*)["']"#)
        .expect("RE_ATTR regex is valid")
});

/// Supported tool call formats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolFormat {
    /// Native JSON (OpenAI/Anthropic function calling)
    Json,
    /// XML-style: <tool name="...">{...}</tool>
    Xml,
    /// MiniMax-style: ```tool_name\n{...}\n```
    Minimax,
    /// Perl-style: $tool_name->({...})
    Perl,
}

/// Parse tool calls from LLM response content.
pub fn parse_tool_calls(content: &str, format: ToolFormat) -> Vec<ToolCall> {
    match format {
        ToolFormat::Json => parse_json_tool_calls(content),
        ToolFormat::Xml => parse_xml_tool_calls(content),
        ToolFormat::Minimax => parse_minimax_tool_calls(content),
        ToolFormat::Perl => parse_perl_tool_calls(content),
    }
}

fn parse_json_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut idx = 0;

    while let Some(start) = content[idx..].find('{') {
        let start = idx + start;
        let mut brace_count = 0;
        let mut end = start;
        for (i, ch) in content[start..].chars().enumerate() {
            if ch == '{' {
                brace_count += 1;
            } else if ch == '}' {
                brace_count -= 1;
                if brace_count == 0 {
                    end = start + i + 1;
                    break;
                }
            }
        }
        if end > start && brace_count == 0 {
            let json_str = &content[start..end];
            if let Ok(value) = serde_json::from_str::<Value>(json_str)
                && let (Some(name), Some(args)) = (
                    value.get("name").and_then(|v| v.as_str()),
                    value.get("arguments"),
                )
            {
                let args_str = if args.is_object() || args.is_array() {
                    args.to_string()
                } else {
                    args.as_str().unwrap_or("").to_string()
                };
                calls.push(ToolCall {
                    id: format!("call_{}", calls.len()),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: name.to_string(),
                        arguments: args_str,
                    },
                });
            }
        }
        idx = start + 1;
    }

    calls
}

fn parse_xml_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    // Pattern 1: <tool name="..." attr="..."/> or <tool name="...">...</tool>
    for caps in RE_TOOL.captures_iter(content) {
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let inner = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let mut args = serde_json::Map::new();

        // Self-closing style: arguments are XML attributes on the <tool> tag.
        for attr_caps in RE_ATTR.captures_iter(attrs) {
            let key = attr_caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let value = attr_caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if key == "name" {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(value) {
                args.insert(key.to_string(), v);
            } else {
                args.insert(key.to_string(), Value::String(value.to_string()));
            }
        }

        // Container style: arguments are inside the <tool> tag.
        for arg_caps in RE_ARG.captures_iter(inner) {
            let key = arg_caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let value = arg_caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if let Ok(v) = serde_json::from_str::<Value>(value) {
                args.insert(key.to_string(), v);
            } else {
                args.insert(key.to_string(), Value::String(value.to_string()));
            }
        }

        // Fallback 1: some models emit parameter values as direct child tags
        // (e.g. `<command>Get-ChildItem</command>`) instead of `<arg key="...">`.
        if args.is_empty() {
            for gen_caps in RE_GENERIC_ARG.captures_iter(inner) {
                let open_key = gen_caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let value = gen_caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                let close_key = gen_caps.get(3).map(|m| m.as_str()).unwrap_or("");
                if open_key != close_key || open_key == "arg" || open_key == "parameter" {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(value) {
                    args.insert(open_key.to_string(), v);
                } else {
                    args.insert(open_key.to_string(), Value::String(value.to_string()));
                }
            }
        }

        // Fallback 2: raw text directly inside <tool> with no wrapper tags
        // (e.g. `<tool name="powershell">\nGet-ChildItem\n</tool>`).
        if args.is_empty() {
            let raw = inner.trim();
            if !raw.is_empty() {
                args.insert("command".to_string(), Value::String(raw.to_string()));
            }
        }

        if !name.is_empty() {
            calls.push(ToolCall {
                id: format!("call_{}", calls.len()),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: name.to_string(),
                    arguments: Value::Object(args).to_string(),
                },
            });
        }
    }

    // Pattern 2: <function_calls><invoke name="...">...</invoke></function_calls>
    for caps in RE_INVOKE.captures_iter(content) {
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let inner = caps.get(2).map(|m| m.as_str()).unwrap_or("");

        // Skip if already captured by <tool> pattern (simple name+content overlap check)
        let already_captured = calls.iter().any(|c| {
            c.function.name == name && content.contains(&format!("<tool name=\"{}\"", name))
        });
        if already_captured {
            continue;
        }

        let mut args = serde_json::Map::new();

        for param_caps in RE_PARAM.captures_iter(inner) {
            let key = param_caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let value = param_caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if let Ok(v) = serde_json::from_str::<Value>(value) {
                args.insert(key.to_string(), v);
            } else {
                args.insert(key.to_string(), Value::String(value.to_string()));
            }
        }

        if !name.is_empty() {
            calls.push(ToolCall {
                id: format!("call_{}", calls.len()),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: name.to_string(),
                    arguments: Value::Object(args).to_string(),
                },
            });
        }
    }

    calls
}

fn parse_minimax_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    for caps in RE_MINIMAX.captures_iter(content) {
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let args_str = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");

        if name.is_empty() {
            continue;
        }

        // Validate that the content looks like JSON arguments
        let arguments = if args_str.starts_with('{') || args_str.starts_with('[') {
            if serde_json::from_str::<Value>(args_str).is_ok() {
                args_str.to_string()
            } else {
                serde_json::json!({ "content": args_str }).to_string()
            }
        } else {
            serde_json::json!({ "content": args_str }).to_string()
        };

        calls.push(ToolCall {
            id: format!("call_{}", calls.len()),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments,
            },
        });
    }

    calls
}

fn parse_perl_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut idx = 0;

    while let Some(dollar_pos) = content[idx..].find('$') {
        let dollar_pos = idx + dollar_pos;
        // Try to match $name->({...})
        let after_dollar = &content[dollar_pos + 1..];
        if let Some(name_end) = after_dollar.find("->(") {
            let name = &after_dollar[..name_end];
            if name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.is_empty() {
                let after_arrow = &after_dollar[name_end + 3..];
                // Skip whitespace
                let after_arrow = after_arrow.trim_start();
                if after_arrow.starts_with('{') {
                    // Find matching brace
                    let mut brace_count = 0;
                    let mut end = 0;
                    for (i, ch) in after_arrow.chars().enumerate() {
                        if ch == '{' {
                            brace_count += 1;
                        } else if ch == '}' {
                            brace_count -= 1;
                            if brace_count == 0 {
                                end = i + 1;
                                break;
                            }
                        }
                    }
                    if end > 0 && brace_count == 0 {
                        let args_str = &after_arrow[..end];
                        let arguments = if serde_json::from_str::<Value>(args_str).is_ok() {
                            args_str.to_string()
                        } else {
                            serde_json::json!({ "content": args_str }).to_string()
                        };
                        calls.push(ToolCall {
                            id: format!("call_{}", calls.len()),
                            call_type: "function".to_string(),
                            function: FunctionCall {
                                name: name.to_string(),
                                arguments,
                            },
                        });
                    }
                }
            }
        }
        idx = dollar_pos + 1;
    }

    calls
}

/// Auto-detect the tool format from content.
pub fn detect_tool_format(content: &str) -> Option<ToolFormat> {
    if content.contains("<tool ") || content.contains("<function_calls>") {
        Some(ToolFormat::Xml)
    } else if content.contains("```") && content.contains("\n{") {
        Some(ToolFormat::Minimax)
    } else if content.contains('$') && content.contains("->({") {
        Some(ToolFormat::Perl)
    } else if content.contains("\"name\"") && content.contains("\"arguments\"") {
        Some(ToolFormat::Json)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_tool_calls() {
        let content = r#"{"name": "file_read", "arguments": {"path": "src/main.rs"}}"#;
        let calls = parse_json_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "file_read");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["path"], "src/main.rs");
    }

    #[test]
    fn test_parse_xml_tool_calls() {
        let content = r#"<tool name="file_read"><arg key="path">src/main.rs</arg></tool>"#;
        let calls = parse_xml_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "file_read");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["path"], "src/main.rs");
    }

    #[test]
    fn test_parse_xml_tool_calls_generic_parameter_tags() {
        let content = r#"<tool name="powershell"><command>Get-ChildItem</command></tool>"#;
        let calls = parse_xml_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "powershell");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["command"], "Get-ChildItem");
    }

    #[test]
    fn test_parse_xml_tool_calls_raw_text_fallback() {
        // Model emits direct text without any arg wrapper tags.
        let content = "<tool name=\"powershell\">\nGet-ChildItem -File\n</tool>";
        let calls = parse_xml_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "powershell");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["command"], "Get-ChildItem -File");
    }

    #[test]
    fn test_parse_xml_tool_calls_self_closing_attributes() {
        let content = r#"<tool name="powershell" command="Get-ChildItem -File" timeout="30"/>"#;
        let calls = parse_xml_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "powershell");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["command"], "Get-ChildItem -File");
        // Numeric attribute values are parsed as JSON numbers when possible.
        assert_eq!(args["timeout"], 30);
    }

    #[test]
    fn test_parse_minimax_tool_calls() {
        let content = "```file_read\n{\"path\": \"src/main.rs\"}\n```";
        let calls = parse_minimax_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "file_read");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["path"], "src/main.rs");
    }

    #[test]
    fn test_parse_perl_tool_calls() {
        let content = r#"$file_read->({"path": "src/main.rs"})"#;
        let calls = parse_perl_tool_calls(content);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "file_read");
        let args: Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
        assert_eq!(args["path"], "src/main.rs");
    }

    #[test]
    fn test_detect_format() {
        assert_eq!(
            detect_tool_format(r#"<tool name="x"></tool>"#),
            Some(ToolFormat::Xml)
        );
        assert_eq!(
            detect_tool_format("```foo\n{}\n```"),
            Some(ToolFormat::Minimax)
        );
        assert_eq!(
            detect_tool_format(r#"$foo->({"a":1})"#),
            Some(ToolFormat::Perl)
        );
        assert_eq!(
            detect_tool_format(r#"{"name":"x","arguments":{}}"#),
            Some(ToolFormat::Json)
        );
        assert_eq!(detect_tool_format("just plain text"), None);
    }
}
