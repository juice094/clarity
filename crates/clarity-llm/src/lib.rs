#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        missing_docs,
        unsafe_code
    )
)]
//! LLM Provider System for Project Clarity
//!
//! This module provides integrations with various LLM providers:
//! - DeepSeek (OpenAI-compatible API)
//! - Kimi (Moonshot)
//! - OpenAI
//! - More providers can be added by implementing the LlmProvider trait

pub mod anthropic;
pub mod api;
pub mod auth;
pub mod catalog;
pub mod deepseek;
pub mod deepseek_device;
pub mod deepseek_pow;
pub mod factory;
pub mod kalosm;
pub mod llama_server;
#[cfg(feature = "local-llm")]
pub mod local_gguf;
pub mod mesh;
pub mod model_listing;
pub mod model_registry;
pub mod ollama;
pub mod policy;
pub mod provider_race;
pub mod providers;
pub mod registry_table;
pub mod runtime;
pub mod runtime_router;
pub mod sse;
pub mod tool_payload;

pub(crate) mod rate_limit;
pub(crate) mod request;

/// Version of the clarity-llm crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Re-export provider types
pub use clarity_contract::ReliableProvider;
pub use deepseek::DeepSeekProvider;
pub use deepseek_device::{
    DeepSeekDeviceConfig, DeepSeekDeviceCredentials, DeepSeekDeviceMode, DeepSeekDeviceOptions,
    DeepSeekDeviceProvider,
};
pub use factory::LlmFactory;
pub use kalosm::{KalosmConfig, KalosmProvider};
pub use llama_server::LlamaServerProvider;
#[cfg(feature = "local-llm")]
pub use local_gguf::{ChatTemplate, LocalGgufConfig, LocalGgufProvider};
pub use model_registry::{
    AuthType, ModelConfigFile, ModelEntry, ModelRegistry, OAuthProviderConfig, ProtocolType,
    ProviderConfig, build_provider_from_registry, build_provider_from_registry_entry,
    build_provider_from_registry_with_key, default_secret_store,
};
pub use ollama::OllamaProvider;
pub use policy::{ProviderSelection, select_provider};
pub use providers::{AnthropicLlm, KimiCodeLlm, KimiLlm, OAuthLlm, OpenAiCompatibleLlm};

pub use model_listing::{get_available_models, scan_local_models};

pub use api::{LlmProvider, LlmResponse, Message, MessageRole, ProviderCapabilities, StreamDelta};

#[cfg(test)]
pub(crate) use request::*;

pub use request::{JsonSchemaSpec, ResponseFormat};

use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

/// Resolve a local model path from environment or default search directory.
///
/// Priority:
/// 1. `CLARITY_LOCAL_MODEL_PATH` environment variable
/// 2. First `.gguf` file found in `~/models/`
///
/// Returns `None` if no model is found, allowing callers to provide
/// a helpful error message instead of a hard-coded personal path.
pub fn resolve_local_model_path() -> Option<PathBuf> {
    // 1. Explicit env var
    if let Ok(path) = env::var("CLARITY_LOCAL_MODEL_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            if let Some(ext) = p.extension() {
                if ext.to_string_lossy().eq_ignore_ascii_case("gguf") {
                    return Some(p);
                }
            }
            tracing::warn!(
                "CLARITY_LOCAL_MODEL_PATH points to a non-.gguf file: {}. Ignoring.",
                p.display()
            );
        }
    }

    // 2. Auto-discover in ~/models/
    if let Some(home) = dirs::home_dir() {
        let models_dir = home.join("models");
        if models_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&models_dir) {
                // Pick the first .gguf file (sorted for stability)
                let mut ggufs: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                            .unwrap_or(false)
                    })
                    .map(|e| e.path())
                    .collect();
                ggufs.sort();
                if let Some(first) = ggufs.into_iter().next() {
                    return Some(first);
                }
            }
        }
    }

    None
}

/// Help text shown when no local model is found.
pub(crate) const LOCAL_MODEL_HELP: &str = "No local model found. To use local inference:\n\
    1. Download a GGUF model (e.g. from https://huggingface.co)\n\
    2. Place it in ~/models/ or set CLARITY_LOCAL_MODEL_PATH to its full path\n\
    3. Optionally set CLARITY_LOCAL_TOKENIZER_REPO to a HuggingFace repo ID for the tokenizer";

static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub(crate) fn shared_http_client() -> reqwest::Client {
    SHARED_HTTP_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .connect_timeout(Duration::from_secs(10))
                .pool_max_idle_per_host(10)
                .build()
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "failed to build custom reqwest client ({}), using default",
                        e
                    );
                    reqwest::Client::new()
                })
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn no_proxy_client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("test client build should not fail")
    }

    #[tokio::test]
    async fn test_openai_stream_assembles_tool_calls() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf).await;
            let response = "HTTP/1.1 200 OK\r\n\
                Content-Type: text/event-stream\r\n\
                Cache-Control: no-cache\r\n\
                Connection: keep-alive\r\n\
                \r\n\
                data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n\
                data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_123\",\"type\":\"function\",\"function\":{\"name\":\"read_file\"}}]}}]}\n\n\
                data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\": \\\"/test.txt\\\"}\"}}]}}]}\n\n\
                data: [DONE]\n\n";
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        let llm = OpenAiCompatibleLlm::with_client(
            "test-key",
            format!("http://127.0.0.1:{}", port),
            "gpt-4o",
            no_proxy_client(),
        );
        let mut rx = llm.stream(&[], &serde_json::json!({})).unwrap();

        let mut deltas = Vec::new();
        while let Some(result) = rx.recv().await {
            deltas.push(result.unwrap());
        }

        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].content, Some("Hello ".to_string()));
        assert!(deltas[0].tool_calls.is_empty());
        assert_eq!(deltas[1].content, None);
        assert_eq!(deltas[1].tool_calls.len(), 1);
        assert_eq!(deltas[1].tool_calls[0].id, "call_123");
        assert_eq!(deltas[1].tool_calls[0].function.name, "read_file");
        assert_eq!(
            deltas[1].tool_calls[0].function.arguments,
            "{\"path\": \"/test.txt\"}"
        );
    }

    #[test]
    fn test_chat_completion_request_serialization_with_cache_key() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: Some("cache-key-123".into()),
            thinking: None,
            response_format: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json.get("model").unwrap(), "test-model");
        assert_eq!(json.get("prompt_cache_key").unwrap(), "cache-key-123");
    }

    #[test]
    fn test_openai_prompt_caching_capability() {
        let provider = OpenAiCompatibleLlm::new("key", "https://api.example.com/v1", "model");
        assert!(provider.capabilities().prompt_caching);
    }

    #[test]
    fn test_chat_completion_request_serialization_without_cache_key() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: None,
            thinking: None,
            response_format: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("prompt_cache_key").is_none());
        assert!(json.get("response_format").is_none());
    }

    #[test]
    fn test_chat_completion_request_serialization_with_response_format() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: None,
            thinking: None,
            response_format: Some(json!({"type": "json_object"})),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(
            json.get("response_format").unwrap(),
            &json!({"type": "json_object"})
        );
    }

    /// Read one full HTTP/1.1 request (headers + Content-Length body).
    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let header_end = loop {
            let n = stream.read(&mut tmp).await.unwrap();
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let content_length = headers
            .lines()
            .find_map(|l| {
                l.strip_prefix("Content-Length:")
                    .or_else(|| l.strip_prefix("content-length:"))
            })
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while buf.len() < header_end + content_length {
            let n = stream.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
        String::from_utf8_lossy(&buf).to_string()
    }

    #[tokio::test]
    async fn test_openai_complete_sends_response_format_only_when_set() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(2);
        tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_http_request(&mut stream).await;
                let _ = tx.send(request).await;
                let response_body =
                    r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let llm = OpenAiCompatibleLlm::with_client(
            "test-key",
            format!("http://127.0.0.1:{}", port),
            "gpt-4o",
            no_proxy_client(),
        );
        let messages = vec![Message::user("hello")];
        let tools = serde_json::json!({});

        // With response_format set, the request body carries the field.
        LlmProvider::set_response_format(&llm, Some(json!({"type": "json_object"})));
        llm.complete(&messages, &tools).await.unwrap();
        let request = rx.recv().await.unwrap();
        let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
        let body_json: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body_json.get("response_format").unwrap(),
            &json!({"type": "json_object"})
        );

        // After clearing, the field disappears from the request body.
        LlmProvider::set_response_format(&llm, None);
        llm.complete(&messages, &tools).await.unwrap();
        let request = rx.recv().await.unwrap();
        let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
        let body_json: serde_json::Value = serde_json::from_str(body).unwrap();
        assert!(body_json.get("response_format").is_none());
    }

    #[test]
    fn test_truncate_messages_caps_system_message() {
        let system = Message::system("x".repeat(2_000_000));
        let messages = vec![system];
        let result = truncate_messages_by_bytes(&messages, MAX_MESSAGE_BODY_BYTES);
        assert_eq!(result.len(), 1);
        assert!(result[0].content.len() <= MAX_MESSAGE_BODY_BYTES);
        assert!(result[0].content.contains("...[truncated]"));
    }

    #[test]
    fn test_truncate_messages_keeps_final_user() {
        let mut messages = vec![Message::system("system".to_string())];
        for i in 0..4 {
            messages.push(Message::user(format!("user message {}", i).repeat(400_000)));
            messages.push(Message::assistant(
                format!("assistant reply {}", i).repeat(400_000),
            ));
        }
        messages.push(Message::user("user message 4".repeat(100_000)));
        let result = truncate_messages_by_bytes(&messages, MAX_MESSAGE_BODY_BYTES);
        assert!(result.iter().any(|m| m.role == MessageRole::User));
        let last_user_content = result
            .iter()
            .rfind(|m| m.role == MessageRole::User)
            .map(|m| m.content.as_str());
        assert_eq!(
            last_user_content,
            Some("user message 4".repeat(100_000).as_str())
        );
        let total: usize = result.iter().map(|m| m.content.len()).sum();
        assert!(total <= MAX_MESSAGE_BODY_BYTES);
    }

    #[test]
    fn test_cap_tools_json_drops_oversized_tools() {
        let huge_description = "x".repeat(600_000);
        let tools = json!([{
            "type": "function",
            "function": {
                "name": "big_tool",
                "description": huge_description,
                "parameters": { "type": "object", "properties": {} }
            }
        }]);
        let tools_opt = cap_tools_json(Some(tools));
        assert!(tools_opt.is_none());
    }

    #[test]
    fn test_guard_request_body_drops_tools_when_oversized() {
        let huge_content = "x".repeat(2_500_000);
        let mut request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: huge_content,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            tools: Some(json!([{
                "type": "function",
                "function": {
                    "name": "small_tool",
                    "description": "a tool",
                    "parameters": { "type": "object", "properties": {} }
                }
            }])),
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: None,
            thinking: None,
            response_format: None,
        };
        guard_request_body_size(&mut request);
        assert!(request.tools.is_none());
    }
}
