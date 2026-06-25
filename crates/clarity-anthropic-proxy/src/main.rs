//! Anthropic Messages API → DeepSeek App reverse proxy.
//!
//! Listens on `127.0.0.1:PORT` (default 18791), accepts Anthropic-formatted
//! `POST /v1/messages` requests, translates them to DeepSeek device API calls
//! through `DeepSeekDeviceProvider`, parses XML tool calls, and returns
//! Anthropic-formatted responses.
//!
//! ## Usage
//!
//! ```bash
//! # Set credentials (one of):
//! export DEEPSEEK_DEVICE_TOKEN="your-mmkv-token"
//! # OR:
//! export DEEPSEEK_DEVICE_MOBILE="13800138000"
//! export DEEPSEEK_DEVICE_PASSWORD="your_password"
//!
//! # Optional:
//! export CC_PROXY_PORT=18791  # default
//!
//! cargo run -p clarity-anthropic-proxy --release
//! ```
//!
//! Then point Claude Code at it:
//! ```bash
//! export ANTHROPIC_BASE_URL="http://127.0.0.1:18791/v1/messages"
//! ```

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use clarity_contract::{LlmProvider, Message};
use clarity_core::agent::tool_parser::{self, ToolFormat};
use clarity_llm::deepseek_device::{
    DeepSeekDeviceConfig, DeepSeekDeviceCredentials, DeepSeekDeviceOptions, DeepSeekDeviceProvider,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tracing::{debug, info, warn};

// ── Anthropic Request Types ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnthropicRequest {
    #[serde(default)]
    model: Option<String>,
    #[allow(dead_code)]
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(default)]
    tools: Vec<AnthropicTool>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    system: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicBlock>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Option<Value>,
        #[serde(default)]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicTool {
    name: String,
    #[serde(default)]
    description: String,
    input_schema: Value,
}

// ── Anthropic Response Types ─────────────────────────────────────────

#[derive(Debug, Serialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    model: String,
    content: Vec<ResponseBlock>,
    stop_reason: String,
    stop_sequence: Option<String>,
    usage: Usage,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ResponseBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Serialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

// ── Translation Layer ────────────────────────────────────────────────

/// Serialize the full Anthropic conversation into a single prompt string.
fn build_prompt(messages: &[AnthropicMessage], system: Option<&str>) -> String {
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
fn content_to_text(content: &AnthropicContent) -> String {
    match content {
        AnthropicContent::Text(s) => s.clone(),
        AnthropicContent::Blocks(blocks) => {
            let mut lines = Vec::new();
            for b in blocks {
                match b {
                    AnthropicBlock::Text { text } => lines.push(text.clone()),
                    AnthropicBlock::ToolUse { id, name, input } => {
                        lines.push(format!(
                            "[Tool Call: {name} id={id}] {}",
                            serde_json::to_string(input).unwrap_or_default()
                        ));
                    }
                    AnthropicBlock::ToolResult {
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
                }
            }
            lines.join("\n")
        }
    }
}

/// Convert Anthropic tool defs to OpenAI-style functions JSON.
fn convert_tools(anthropic_tools: &[AnthropicTool]) -> Value {
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

/// Strip XML `<tool>` and `<function_calls>` blocks from text.
fn strip_tool_xml(content: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<tool\s[^>]*>.*?</tool>|<function_calls>.*?</function_calls>")
            .expect("strip regex")
    });
    let cleaned = RE.replace_all(content, "");
    // Collapse 3+ blank lines
    Regex::new(r"\n{3,}")
        .unwrap()
        .replace_all(&cleaned, "\n\n")
        .trim()
        .to_string()
}

// ── Shared State ─────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    provider: Arc<DeepSeekDeviceProvider>,
}

// ── Handler ──────────────────────────────────────────────────────────

async fn messages_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnthropicRequest>,
) -> impl IntoResponse {
    if req.stream {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": { "type": "invalid_request_error", "message": "Streaming not yet supported" }
            })),
        )
            .into_response();
    }

    let model = req.model.as_deref().unwrap_or("deepseek-chat");
    let system = req.system.as_deref();
    let tools_clarity = convert_tools(&req.tools);
    let prompt = build_prompt(&req.messages, system);
    debug!("Prompt: {} chars, {} tools", prompt.len(), req.tools.len());

    // Stateless — each request is self-contained
    state.provider.reset_conversation_context();

    let clarity_messages = vec![
        Message::system(""), // adapt_prompt_guided injects XML tool descriptions here
        Message::user(prompt),
    ];

    let input_tokens = clarity_messages
        .iter()
        .map(|m| m.content.len() as u32 / 4)
        .sum();

    let llm_response = match state
        .provider
        .complete(&clarity_messages, &tools_clarity)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("Provider error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": { "type": "api_error", "message": format!("{e}") }
                })),
            )
                .into_response();
        }
    };

    // Parse XML tool calls from response
    let tool_calls =
        if tool_parser::detect_tool_format(&llm_response.content) == Some(ToolFormat::Xml) {
            tool_parser::parse_tool_calls(&llm_response.content, ToolFormat::Xml)
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

    let response = AnthropicResponse {
        id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        model: model.to_string(),
        stop_reason: if tool_calls.is_empty() {
            "end_turn"
        } else {
            "tool_use"
        }
        .to_string(),
        stop_sequence: None,
        content: content_blocks,
        usage: Usage {
            input_tokens,
            output_tokens: (llm_response.content.len() / 4) as u32,
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

// ── Server Entry Point ───────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("clarity_anthropic_proxy=debug,info")
        .init();

    let config = DeepSeekDeviceConfig {
        base_url: "https://chat.deepseek.com".into(),
        client_version: "2.1.8".into(),
        device_id: "cc-proxy".into(),
        credentials: if let Ok(token) = std::env::var("DEEPSEEK_DEVICE_TOKEN") {
            info!("Using DEEPSEEK_DEVICE_TOKEN");
            DeepSeekDeviceCredentials::Token(token)
        } else if let (Ok(mobile), Ok(password)) = (
            std::env::var("DEEPSEEK_DEVICE_MOBILE"),
            std::env::var("DEEPSEEK_DEVICE_PASSWORD"),
        ) {
            info!("Using DEEPSEEK_DEVICE_MOBILE + DEEPSEEK_DEVICE_PASSWORD");
            DeepSeekDeviceCredentials::Password { mobile, password }
        } else {
            anyhow::bail!(
                "Set DEEPSEEK_DEVICE_TOKEN or DEEPSEEK_DEVICE_MOBILE+DEEPSEEK_DEVICE_PASSWORD"
            );
        },
        options: DeepSeekDeviceOptions::from_model_id("deepseek-chat"),
    };

    let provider = Arc::new(DeepSeekDeviceProvider::new(config));
    // Warm up: login / validate token immediately
    info!("Initializing provider (login/PoW)...");
    if let Err(e) = provider
        .complete(&[Message::user("ping")], &Value::Array(vec![]))
        .await
    {
        warn!("Initial auth check failed (non-fatal): {e}");
    } else {
        info!("Provider ready");
    }

    let port: u16 = std::env::var("CC_PROXY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(18791);

    let state = Arc::new(AppState { provider });
    let app = Router::new()
        .route("/v1/messages", post(messages_handler))
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    info!("Listening on http://{addr}");
    println!("cc-proxy listening on http://{addr}");
    println!("Set ANTHROPIC_BASE_URL=http://{addr}/v1/messages");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_tools_maps_input_schema_to_parameters() {
        let tools = vec![AnthropicTool {
            name: "test".into(),
            description: "desc".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {"x": {"type": "string"}}}),
        }];
        let result = convert_tools(&tools);
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "test");
    }

    #[test]
    fn strip_tool_xml_removes_tool_blocks() {
        let input = "Before\n<tool name=\"x\">\n<arg key=\"y\">z</arg>\n</tool>\nAfter";
        let result = strip_tool_xml(input);
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
        assert!(!result.contains("<tool"));
    }

    #[test]
    fn build_prompt_includes_system_and_messages() {
        let msgs = vec![AnthropicMessage {
            role: "user".into(),
            content: AnthropicContent::Text("Hello".into()),
        }];
        let prompt = build_prompt(&msgs, Some("You are helpful."));
        assert!(prompt.contains("System: You are helpful."));
        assert!(prompt.contains("User: Hello"));
    }

    #[test]
    fn content_to_text_handles_tool_blocks() {
        let content = AnthropicContent::Blocks(vec![
            AnthropicBlock::Text {
                text: "Let me check".into(),
            },
            AnthropicBlock::ToolUse {
                id: "toolu_1".into(),
                name: "sh".into(),
                input: serde_json::json!({"command": "ls"}),
            },
        ]);
        let text = content_to_text(&content);
        assert!(text.contains("Let me check"));
        assert!(text.contains("[Tool Call: sh"));
        assert!(text.contains("toolu_1"));
    }
}
