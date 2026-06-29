//! Anthropic Messages API → DeepSeek App reverse proxy.
//!
//! Listens on `127.0.0.1:PORT` (default 18791), accepts Anthropic-formatted
//! `POST /v1/messages` requests, and translates them to DeepSeek device API
//! calls through `clarity_llm::anthropic::AnthropicAdapter`.
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

use axum::{
    Json, Router,
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use clarity_contract::{LlmProvider, Message};
use clarity_llm::anthropic::{AnthropicAdapter, AnthropicRequest};
use clarity_llm::deepseek_device::{
    DeepSeekDeviceConfig, DeepSeekDeviceCredentials, DeepSeekDeviceOptions, DeepSeekDeviceProvider,
};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Shared application state.
#[derive(Clone)]
struct AppState {
    /// Anthropic facade over the DeepSeek device provider.
    adapter: AnthropicAdapter,
}

async fn log_requests(req: Request<axum::body::Body>, next: Next) -> impl IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    info!(
        "<< {} {} | {:?}",
        method,
        uri,
        headers.get("x-api-key").map(|_| "***")
    );
    let response = next.run(req).await;
    info!(">> {} {} -> {}", method, uri, response.status());
    response
}

async fn messages_handler(State(state): State<Arc<AppState>>, body: String) -> impl IntoResponse {
    let req: AnthropicRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            warn!("Deserialize error: {e}");
            warn!("Body preview: {}", &body[..body.len().min(1000)]);
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": { "type": "invalid_request_error", "message": format!("deserialization: {e}") }
                })),
            )
                .into_response();
        }
    };

    debug!("Anthropic request: {} tools", req.tools.len());

    match state.adapter.complete(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            warn!("Adapter error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": { "type": "api_error", "message": "Internal server error" }
                })),
            )
                .into_response()
        }
    }
}

async fn count_tokens_handler(body: String) -> impl IntoResponse {
    // Rough estimate: 1 token per 4 chars of the JSON body.
    let chars = body.len();
    Json(serde_json::json!({
        "input_tokens": (chars / 4) as u32
    }))
}

async fn models_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [
            {"id": "claude-sonnet-4-6", "object": "model", "created": 1, "owned_by": "cc-proxy"},
            {"id": "claude-opus-4-8", "object": "model", "created": 1, "owned_by": "cc-proxy"},
            {"id": "claude-haiku-4-5-20251001", "object": "model", "created": 1, "owned_by": "cc-proxy"},
            {"id": "claude-fable-5", "object": "model", "created": 1, "owned_by": "cc-proxy"}
        ]
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("clarity_anthropic_proxy=debug,info")
        .init();

    let config = DeepSeekDeviceConfig {
        base_url: "https://chat.deepseek.com".into(),
        client_version: "2.1.8".into(),
        device_id: "cc-proxy".into(),
        credentials: if let (Ok(mobile), Ok(password)) = (
            std::env::var("DEEPSEEK_DEVICE_MOBILE"),
            std::env::var("DEEPSEEK_DEVICE_PASSWORD"),
        ) {
            info!("Using DEEPSEEK_DEVICE_MOBILE + DEEPSEEK_DEVICE_PASSWORD");
            DeepSeekDeviceCredentials::Password { mobile, password }
        } else if let Ok(token) = std::env::var("DEEPSEEK_DEVICE_TOKEN") {
            info!("Using DEEPSEEK_DEVICE_TOKEN (fallback)");
            DeepSeekDeviceCredentials::Token(token)
        } else {
            anyhow::bail!(
                "Set DEEPSEEK_DEVICE_MOBILE+DEEPSEEK_DEVICE_PASSWORD or DEEPSEEK_DEVICE_TOKEN"
            );
        },
        options: DeepSeekDeviceOptions::from_model_id("deepseek-chat"),
    };

    let provider: Arc<dyn LlmProvider> = Arc::new(DeepSeekDeviceProvider::new(config));

    // Warm up: login / validate token immediately.
    info!("Initializing provider (login/PoW)...");
    if let Err(e) = provider
        .complete(&[Message::user("ping")], &Value::Array(vec![]))
        .await
    {
        warn!("Initial auth check failed (non-fatal): {e}");
    } else {
        info!("Provider ready");
    }

    let adapter = AnthropicAdapter::new(provider);
    let port: u16 = std::env::var("CC_PROXY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(18791);

    let state = Arc::new(AppState { adapter });
    let app = Router::new()
        .route("/v1/messages", post(messages_handler))
        .route("/v1/messages/count_tokens", post(count_tokens_handler))
        .route("/v1/models", get(models_handler))
        .layer(middleware::from_fn(log_requests))
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
    use clarity_llm::anthropic::types::{AnthropicBlock, AnthropicContent, AnthropicMessage};
    use clarity_llm::anthropic::{build_prompt, content_to_text, convert_tools, strip_tool_xml};

    #[test]
    fn convert_tools_maps_input_schema_to_parameters() {
        let tools = vec![clarity_llm::anthropic::types::AnthropicTool {
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
        let prompt = build_prompt(&msgs, &Some("You are helpful.".to_string()));
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
