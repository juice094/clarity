//! Ollama Local LLM Provider
//!
//! Connects to a locally-running Ollama instance via its native `/api/chat`
//! endpoint. Ollama uses an OpenAI-compatible message format for requests,
//! but returns responses in its own shape (`message` at the root).
//!
//! ## Quick start
//!
//! 1. Install Ollama: [ollama.com/download](https://ollama.com/download)
//! 2. Pull a model: `ollama pull llama3`
//! 3. Ensure the server is running (default: http://localhost:11434)
//!
//! ## Configuration
//!
//! Environment variables:
//! - `OLLAMA_HOST` - Base URL (default: `http://localhost:11434`)
//! - `OLLAMA_MODEL` - Model name (default: `llama3`)

use crate::api::{LlmProvider, LlmResponse, Message, StreamDelta};
use crate::shared_http_client;
use async_trait::async_trait;
use clarity_contract::AgentError;
use clarity_contract::FunctionCall;
use clarity_contract::ToolCall;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::env;

/// Ollama LLM Provider
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Value>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaApiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_calls: Option<Vec<OllamaApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OllamaApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OllamaApiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OllamaApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    // Intentionally retained: `role` is part of the Ollama chat response schema
    // and is useful for validating the response shape during deserialization.
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Deserialize, Clone)]
struct OllamaToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    call_type: Option<String>,
    function: OllamaToolFunctionCall,
}

#[derive(Debug, Deserialize, Clone)]
struct OllamaToolFunctionCall {
    name: String,
    #[serde(deserialize_with = "deserialize_arguments")]
    arguments: String,
}

fn deserialize_arguments<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(s),
        _ => serde_json::to_string(&value).map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Deserialize)]
struct OllamaStreamChunk {
    message: OllamaMessage,
    done: bool,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// `base_url` should be the root URL, e.g. `http://localhost:11434`.
    /// `model` is the Ollama model tag, e.g. `llama3` or `qwen`.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            client: shared_http_client(),
        }
    }

    /// Create from environment variables.
    ///
    /// Optional: `OLLAMA_HOST` (default: "http://localhost:11434")
    /// Optional: `OLLAMA_MODEL` (default: "llama3")
    pub fn from_env() -> Result<Self, AgentError> {
        let base_url = env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3".into());
        Ok(Self::new(base_url, model))
    }
}

fn convert_messages(messages: &[Message]) -> Vec<OllamaApiMessage> {
    messages
        .iter()
        .map(|m| OllamaApiMessage {
            role: format!("{:?}", m.role).to_lowercase(),
            content: m.content.clone(),
            tool_calls: m.tool_calls.clone().map(|tcs| {
                tcs.into_iter()
                    .map(|tc| OllamaApiToolCall {
                        id: tc.id,
                        call_type: tc.call_type,
                        function: OllamaApiFunctionCall {
                            name: tc.function.name,
                            arguments: tc.function.arguments,
                        },
                    })
                    .collect()
            }),
            tool_call_id: m.tool_call_id.clone(),
        })
        .collect()
}

fn next_tool_call_id() -> String {
    format!("ollama_call_{}", rand::random::<u16>())
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let api_messages = convert_messages(messages);
        let tools_opt = tools
            .as_array()
            .filter(|a| !a.is_empty())
            .map(|_| tools.clone());

        let request_body = OllamaChatRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            stream: false,
        };

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));

        tracing::debug!(
            "Ollama complete request: {} messages, model={}",
            request_body.messages.len(),
            self.model
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AgentError::Llm(format!(
                "Ollama API error ({}): {}",
                status, error_text
            )));
        }

        let completion: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| AgentError::Llm(format!("Failed to parse Ollama response: {}", e)))?;

        let content = completion.message.content;
        let tool_calls: Vec<ToolCall> = completion
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id.unwrap_or_else(next_tool_call_id),
                call_type: tc.call_type.unwrap_or_else(|| "function".to_string()),
                function: FunctionCall {
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                },
            })
            .collect();

        Ok(LlmResponse {
            content,
            tool_calls,
            is_complete: true,
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let api_messages = convert_messages(messages);
        let tools_opt = tools
            .as_array()
            .filter(|a| !a.is_empty())
            .map(|_| tools.clone());

        let request_body = OllamaChatRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            stream: true,
        };

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let client = self.client.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let err = resp.text().await.unwrap_or_default();
                        let _ = tx
                            .send(Err(AgentError::Llm(format!("Ollama API error: {}", err))))
                            .await;
                        return;
                    }

                    let mut stream = resp.bytes_stream();
                    use futures::StreamExt;

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let text = String::from_utf8_lossy(&bytes);
                                for line in text.lines() {
                                    let line = line.trim();
                                    if line.is_empty() {
                                        continue;
                                    }
                                    match serde_json::from_str::<OllamaStreamChunk>(line) {
                                        Ok(event) => {
                                            if event.done {
                                                return;
                                            }
                                            if !event.message.content.is_empty()
                                                && tx
                                                    .send(Ok(StreamDelta {
                                                        content: Some(event.message.content),
                                                        reasoning_content: None,
                                                        tool_calls: vec![],
                                                    }))
                                                    .await
                                                    .is_err()
                                            {
                                                return;
                                            }
                                            if let Some(tcs) = event.message.tool_calls {
                                                let tool_calls: Vec<ToolCall> = tcs
                                                    .into_iter()
                                                    .map(|tc| ToolCall {
                                                        id: tc.id.unwrap_or_else(next_tool_call_id),
                                                        call_type: tc.call_type.unwrap_or_else(
                                                            || "function".to_string(),
                                                        ),
                                                        function: FunctionCall {
                                                            name: tc.function.name,
                                                            arguments: tc.function.arguments,
                                                        },
                                                    })
                                                    .collect();
                                                if !tool_calls.is_empty()
                                                    && tx
                                                        .send(Ok(StreamDelta {
                                                            content: None,
                                                            reasoning_content: None,
                                                            tool_calls,
                                                        }))
                                                        .await
                                                        .is_err()
                                                {
                                                    return;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                "Failed to parse Ollama stream line: {} ({})",
                                                line,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(Err(AgentError::Llm(format!("Stream error: {}", e))))
                                    .await;
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(AgentError::Llm(format!("HTTP error: {}", e))))
                        .await;
                }
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&self, _key: &str) {
        // Ollama does not support prompt cache key
    }

    fn capabilities(&self) -> crate::api::ProviderCapabilities {
        crate::api::ProviderCapabilities {
            native_tool_calling: true,
            prompt_guided_tool_calling: false,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serializes tests that mutate process environment variables.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, value: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn test_ollama_provider_creation() {
        let provider = OllamaProvider::new("http://localhost:11434", "llama3");
        assert_eq!(provider.base_url, "http://localhost:11434");
        assert_eq!(provider.model, "llama3");
    }

    #[test]
    fn test_ollama_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_host = env::var("OLLAMA_HOST").ok();
        let original_model = env::var("OLLAMA_MODEL").ok();

        set_env("OLLAMA_HOST", "http://ollama.example.com");
        set_env("OLLAMA_MODEL", "qwen");

        let provider = OllamaProvider::from_env().unwrap();
        assert_eq!(provider.base_url, "http://ollama.example.com");
        assert_eq!(provider.model, "qwen");

        match original_host {
            Some(v) => set_env("OLLAMA_HOST", &v),
            None => remove_env("OLLAMA_HOST"),
        }
        match original_model {
            Some(v) => set_env("OLLAMA_MODEL", &v),
            None => remove_env("OLLAMA_MODEL"),
        }
    }

    #[test]
    fn test_ollama_from_env_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_host = env::var("OLLAMA_HOST").ok();
        let original_model = env::var("OLLAMA_MODEL").ok();

        remove_env("OLLAMA_HOST");
        remove_env("OLLAMA_MODEL");

        let provider = OllamaProvider::from_env().unwrap();
        assert_eq!(provider.base_url, "http://localhost:11434");
        assert_eq!(provider.model, "llama3");

        match original_host {
            Some(v) => set_env("OLLAMA_HOST", &v),
            None => remove_env("OLLAMA_HOST"),
        }
        match original_model {
            Some(v) => set_env("OLLAMA_MODEL", &v),
            None => remove_env("OLLAMA_MODEL"),
        }
    }

    #[test]
    fn test_ollama_request_serialization() {
        let request = OllamaChatRequest {
            model: "llama3".into(),
            messages: vec![OllamaApiMessage {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            stream: false,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json.get("model").unwrap(), "llama3");
        assert_eq!(json.get("stream").unwrap(), false);
        assert!(json.get("tools").is_none());
    }

    #[test]
    fn test_ollama_response_deserialization() {
        let json_str = r#"
        {
            "model": "llama3",
            "message": {
                "role": "assistant",
                "content": "Hello there!"
            },
            "done": true
        }
        "#;
        let resp: OllamaChatResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.message.content, "Hello there!");
        assert!(resp.message.tool_calls.is_none());
    }

    #[test]
    fn test_ollama_response_with_tool_calls_object_args() {
        let json_str = r#"
        {
            "model": "llama3",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "get_weather",
                            "arguments": {"location": "Paris"}
                        }
                    }
                ]
            },
            "done": true
        }
        "#;
        let resp: OllamaChatResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.message.content, "");
        let tcs = resp.message.tool_calls.unwrap();
        assert_eq!(tcs[0].function.name, "get_weather");
        assert_eq!(tcs[0].function.arguments, r#"{"location":"Paris"}"#);
    }

    #[test]
    fn test_ollama_response_with_tool_calls_string_args() {
        let json_str = r#"
        {
            "model": "llama3",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"Paris\"}"
                        }
                    }
                ]
            },
            "done": true
        }
        "#;
        let resp: OllamaChatResponse = serde_json::from_str(json_str).unwrap();
        let tcs = resp.message.tool_calls.unwrap();
        assert_eq!(tcs[0].function.arguments, r#"{"location": "Paris"}"#);
    }

    #[test]
    fn test_ollama_stream_chunk_deserialization() {
        let json_str =
            r#"{"model":"llama3","message":{"role":"assistant","content":"Hello"},"done":false}"#;
        let chunk: OllamaStreamChunk = serde_json::from_str(json_str).unwrap();
        assert!(!chunk.done);
        assert_eq!(chunk.message.content, "Hello");
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![Message::system("You are helpful"), Message::user("Hello")];
        let api_msgs = convert_messages(&messages);
        assert_eq!(api_msgs.len(), 2);
        assert_eq!(api_msgs[0].role, "system");
        assert_eq!(api_msgs[0].content, "You are helpful");
        assert_eq!(api_msgs[1].role, "user");
        assert_eq!(api_msgs[1].content, "Hello");
    }

    #[tokio::test]
    async fn test_ollama_stream_mock() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf).await;
            let response = "HTTP/1.1 200 OK\r\n\
                Content-Type: application/x-ndjson\r\n\
                \r\n\
                {\"model\":\"llama3\",\"message\":{\"role\":\"assistant\",\"content\":\"Hello \" },\"done\":false}\n\
                {\"model\":\"llama3\",\"message\":{\"role\":\"assistant\",\"content\":\"world\"},\"done\":false}\n\
                {\"model\":\"llama3\",\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true}\n";
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        let llm = OllamaProvider::new(format!("http://127.0.0.1:{}", port), "llama3");
        let mut rx = llm.stream(&[], &json!({})).unwrap();

        let mut contents = Vec::new();
        while let Some(result) = rx.recv().await {
            let delta = result.unwrap();
            if let Some(content) = delta.content {
                contents.push(content);
            }
        }

        assert_eq!(contents, vec!["Hello ", "world"]);
    }

    #[tokio::test]
    async fn test_ollama_complete_mock() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("/api/chat"));
            assert!(req.contains("\"stream\":false"));

            let response = r#"HTTP/1.1 200 OK
Content-Type: application/json

{"model":"llama3","message":{"role":"assistant","content":"Hi there!"},"done":true}"#;
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        let llm = OllamaProvider::new(format!("http://127.0.0.1:{}", port), "llama3");
        let result = llm
            .complete(&[Message::user("Hello")], &json!({}))
            .await
            .unwrap();

        assert_eq!(result.content, "Hi there!");
        assert!(result.tool_calls.is_empty());
    }
}
