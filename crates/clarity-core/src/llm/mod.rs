//! LLM Provider System for Project Clarity
//!
//! This module provides integrations with various LLM providers:
//! - DeepSeek (OpenAI-compatible API)
//! - Kimi (Moonshot)
//! - OpenAI
//! - More providers can be added by implementing the LlmProvider trait

pub mod deepseek;

// Re-export provider types
pub use deepseek::DeepSeekProvider;

use crate::agent::{LlmProvider, LlmResponse, Message, MessageRole};
use crate::error::AgentError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::sync::OnceLock;
use std::time::Duration;

/// Delta emitted by a streaming LLM response.
#[derive(Debug, Clone, Default)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub tool_calls: Vec<crate::agent::ToolCall>,
}

static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_http_client() -> reqwest::Client {
    SHARED_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .build()
            .expect("failed to build reqwest client")
    }).clone()
}

// ==================== OpenAI Compatible API Types ====================

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ApiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ApiMessage,
    #[allow(dead_code)]
    #[serde(default)]
    finish_reason: Option<String>,
}

/// Generic OpenAI-compatible LLM provider
///
/// Works with any API that follows the OpenAI chat completions format
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
    prompt_cache_key: Option<String>,
}

impl OpenAiCompatibleLlm {
    /// Create a new OpenAI-compatible LLM provider
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            client: shared_http_client(),
            prompt_cache_key: None,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| AgentError::Llm("OPENAI_API_KEY not set".into()))?;

        let base_url = env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());

        let model = env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4o".into());

        Ok(Self::new(api_key, base_url, model))
    }

    pub fn set_prompt_cache_key(&mut self, key: impl Into<String>) {
        self.prompt_cache_key = Some(key.into());
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        // Convert internal Message to API message format
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| ApiMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
                tool_calls: None,
            })
            .collect();

        let tools_opt = tools.as_array().filter(|a| !a.is_empty()).map(|_| tools.clone());
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: self.prompt_cache_key.clone(),
        };

        // Build URL: base_url should end with /v1, e.g. https://api.kimi.com/coding/v1
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };
        
        tracing::debug!("Sending request to {}", url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("User-Agent", "claude-code/0.1.0 (Claude Code)")
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
                "API error ({}): {}",
                status, error_text
            )));
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| AgentError::Llm(format!("Failed to parse response: {}", e)))?;

        let choice = completion.choices.into_iter().next();
        let content = choice.as_ref().map(|c| c.message.content.clone()).unwrap_or_default();
        let tool_calls: Vec<crate::agent::ToolCall> = choice
            .and_then(|c| c.message.tool_calls)
            .map(|tcs| {
                tcs.into_iter()
                    .map(|tc| crate::agent::ToolCall {
                        id: tc.id,
                        call_type: if tc.call_type.is_empty() {
                            "function".to_string()
                        } else {
                            tc.call_type
                        },
                        function: crate::agent::FunctionCall {
                            name: tc.function.name,
                            arguments: tc.function.arguments,
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();

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
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| ApiMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
                tool_calls: None,
            })
            .collect();

        let tools_opt = tools.as_array().filter(|a| !a.is_empty()).map(|_| tools.clone());
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            temperature: None,
            max_tokens: None,
            stream: true,
            prompt_cache_key: self.prompt_cache_key.clone(),
        };

        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };
        let api_key = self.api_key.clone();
        let client = self.client.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            #[derive(Default)]
            struct PartialToolCall {
                id: String,
                call_type: String,
                name: String,
                arguments: String,
            }

            let assemble = |ptc: &PartialToolCall| -> crate::agent::ToolCall {
                crate::agent::ToolCall {
                    id: ptc.id.clone(),
                    call_type: if ptc.call_type.is_empty() {
                        "function".to_string()
                    } else {
                        ptc.call_type.clone()
                    },
                    function: crate::agent::FunctionCall {
                        name: ptc.name.clone(),
                        arguments: ptc.arguments.clone(),
                    },
                }
            };

            let flush_last = |pc: &[PartialToolCall], lsi: Option<usize>| -> Option<crate::agent::ToolCall> {
                let idx = lsi?;
                let ptc = pc.get(idx)?;
                let call = assemble(ptc);
                if call.id.is_empty() || call.function.name.is_empty() {
                    None
                } else {
                    Some(call)
                }
            };

            let mut partial_calls: Vec<PartialToolCall> = Vec::new();
            let mut last_seen_index: Option<usize> = None;

            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .header("User-Agent", "claude-code/0.1.0 (Claude Code)")
                .json(&request_body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let err = resp.text().await.unwrap_or_default();
                        let _ = tx.send(Err(AgentError::Llm(format!("API error: {}", err)))).await;
                        return;
                    }

                    let mut stream = resp.bytes_stream();
                    use futures::StreamExt;

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let text = String::from_utf8_lossy(&bytes);
                                for line in text.lines() {
                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            if let Some(call) = flush_last(&partial_calls, last_seen_index) {
                                                let _ = tx.send(Ok(StreamDelta {
                                                    content: None,
                                                    tool_calls: vec![call],
                                                })).await;
                                            }
                                            return;
                                        }
                                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                                            if let Some(choices) = event.get("choices").and_then(|c| c.as_array()) {
                                                for choice in choices {
                                                    if let Some(delta) = choice.get("delta") {
                                                        // Content delta
                                                        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                            if !content.is_empty() && tx.send(Ok(StreamDelta {
                                                                content: Some(content.to_string()),
                                                                tool_calls: vec![],
                                                            })).await.is_err() {
                                                                return;
                                                            }
                                                        }

                                                        // Tool call deltas
                                                        if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                                            for tc_delta in tool_calls {
                                                                if let Some(index) = tc_delta.get("index").and_then(|i| i.as_u64()).map(|i| i as usize) {
                                                                    // Flush previous index when a new one appears
                                                                    if let Some(last) = last_seen_index {
                                                                        if index > last {
                                                                            if let Some(call) = flush_last(&partial_calls, last_seen_index) {
                                                                                if tx.send(Ok(StreamDelta {
                                                                                    content: None,
                                                                                    tool_calls: vec![call],
                                                                                })).await.is_err() {
                                                                                    return;
                                                                                }
                                                                            }
                                                                        }
                                                                    }

                                                                    last_seen_index = Some(index);

                                                                    if index >= partial_calls.len() {
                                                                        partial_calls.resize_with(index + 1, PartialToolCall::default);
                                                                    }

                                                                    if let Some(id) = tc_delta.get("id").and_then(|i| i.as_str()) {
                                                                        partial_calls[index].id.push_str(id);
                                                                    }
                                                                    if let Some(call_type) = tc_delta.get("type").and_then(|t| t.as_str()) {
                                                                        partial_calls[index].call_type.push_str(call_type);
                                                                    }
                                                                    if let Some(func) = tc_delta.get("function") {
                                                                        if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                                                            partial_calls[index].name.push_str(name);
                                                                        }
                                                                        if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                                                            partial_calls[index].arguments.push_str(args);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(AgentError::Llm(format!("Stream error: {}", e)))).await;
                                return;
                            }
                        }
                    }

                    // Flush any remaining completed tool call when stream ends without [DONE]
                    if let Some(call) = flush_last(&partial_calls, last_seen_index) {
                        let _ = tx.send(Ok(StreamDelta {
                            content: None,
                            tool_calls: vec![call],
                        })).await;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(AgentError::Llm(format!("HTTP error: {}", e)))).await;
                }
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.set_prompt_cache_key(key);
    }
}

/// Kimi (Moonshot) LLM Provider
#[derive(Debug, Clone)]
pub struct KimiLlm {
    inner: OpenAiCompatibleLlm,
}

impl KimiLlm {
    /// Create from environment variables
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("KIMI_API_KEY")
            .map_err(|_| AgentError::Llm("KIMI_API_KEY not set".into()))?;

        let base_url = env::var("KIMI_BASE_URL")
            .unwrap_or_else(|_| "https://api.moonshot.ai/v1".into());

        let model = env::var("KIMI_MODEL")
            .unwrap_or_else(|_| "kimi-k2-07132k".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create with explicit parameters
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new(api_key, base_url, model),
        }
    }
}

#[async_trait]
impl LlmProvider for KimiLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        self.inner.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

/// Kimi Code LLM Provider
///
/// Uses OpenAI-compatible API for Kimi Code programming plan.
/// Keys start with `sk-kimi-` and use a separate endpoint from Moonshot.
#[derive(Debug, Clone)]
pub struct KimiCodeLlm {
    inner: OpenAiCompatibleLlm,
}

impl KimiCodeLlm {
    /// Create from environment variables
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("KIMI_CODE_API_KEY")
            .map_err(|_| AgentError::Llm("KIMI_CODE_API_KEY not set".into()))?;

        let base_url = env::var("KIMI_CODE_BASE_URL")
            .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into());

        let model = env::var("KIMI_CODE_MODEL")
            .unwrap_or_else(|_| "kimi-k2-07132k".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create with explicit parameters
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new(api_key, base_url, model),
        }
    }

    pub fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

#[async_trait]
impl LlmProvider for KimiCodeLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        self.inner.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        self.inner.stream(messages, tools)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

/// Anthropic LLM Provider
/// 
/// Supports Anthropic Messages API format
/// Used by Kimi Code and other Claude-compatible endpoints
#[derive(Debug, Clone)]
pub struct AnthropicLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

/// Anthropic API request/response types
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

impl AnthropicLlm {
    /// Create from environment variables
    /// 
    /// Required: ANTHROPIC_AUTH_TOKEN
    /// Optional: ANTHROPIC_BASE_URL (default: "https://api.anthropic.com")
    /// Optional: ANTHROPIC_MODEL (default: "claude-3-sonnet-20240229")
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| AgentError::Llm("ANTHROPIC_AUTH_TOKEN not set".into()))?;

        let base_url = env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".into());

        let model = env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-3-sonnet-20240229".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create with explicit parameters
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicLlm {
    async fn complete(
        &self,
        messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        // Extract system message if present
        let system_msg = messages.iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Convert messages (excluding system)
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| AnthropicMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
            })
            .collect();

        let request_body = AnthropicRequest {
            model: self.model.clone(),
            messages: anthropic_messages,
            system: system_msg,
            max_tokens: Some(4096),
        };

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        
        tracing::debug!("Sending Anthropic request to {}", url);
        
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("User-Agent", "claude-code/0.1.0 (Claude Code)")
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
                "API error ({}): {}",
                status, error_text
            )));
        }

        let anthropic_resp: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| AgentError::Llm(format!("Failed to parse response: {}", e)))?;

        let content = anthropic_resp
            .content
            .into_iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(LlmResponse {
            content,
            tool_calls: vec![],
            is_complete: true,
        })
    }

    fn stream(
        &self,
        messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        // Extract system message if present
        let system_msg = messages.iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Convert messages (excluding system)
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| AnthropicMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
            })
            .collect();

        let request_body = AnthropicRequest {
            model: self.model.clone(),
            messages: anthropic_messages,
            system: system_msg,
            max_tokens: Some(4096),
        };

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let api_key = self.api_key.clone();
        let client = self.client.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            let response = client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .header("User-Agent", "claude-code/0.1.0 (Claude Code)")
                .json(&request_body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let err = resp.text().await.unwrap_or_default();
                        let _ = tx.send(Err(AgentError::Llm(format!("API error: {}", err)))).await;
                        return;
                    }

                    // Process SSE stream
                    let mut stream = resp.bytes_stream();
                    use futures::StreamExt;

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let text = String::from_utf8_lossy(&bytes);
                                for line in text.lines() {
                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            return;
                                        }
                                        // Parse SSE event and extract content
                                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                                            // Anthropic streaming format: content_block_delta events
                                            if let Some(delta) = event.get("delta") {
                                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                    if tx.send(Ok(StreamDelta {
                                                        content: Some(text.to_string()),
                                                        tool_calls: vec![],
                                                    })).await.is_err() {
                                                        return;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(AgentError::Llm(format!("Stream error: {}", e)))).await;
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(AgentError::Llm(format!("HTTP error: {}", e)))).await;
                }
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, _key: &str) {}
}

/// Factory for creating LLM providers
pub struct LlmFactory;

impl LlmFactory {
    /// Auto-detect provider from environment variables
    /// Priority: ANTHROPIC > KIMI_CODE > KIMI > DEEPSEEK > OPENAI
    pub fn auto() -> Result<Box<dyn LlmProvider>, AgentError> {
        if env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
            return Ok(Box::new(AnthropicLlm::from_env()?));
        }

        if env::var("KIMI_CODE_API_KEY").is_ok() {
            return Ok(Box::new(KimiCodeLlm::from_env()?));
        }

        if let Ok(kimi_key) = env::var("KIMI_API_KEY") {
            // Kimi Code keys start with "sk-kimi-" and need the coding endpoint.
            if kimi_key.starts_with("sk-kimi-") {
                let base_url = env::var("KIMI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into());
                let model = env::var("KIMI_MODEL")
                    .unwrap_or_else(|_| "kimi-k2-07132k".into());
                return Ok(Box::new(KimiCodeLlm::new(kimi_key, base_url, model)));
            }
            return Ok(Box::new(KimiLlm::from_env()?));
        }

        if env::var("DEEPSEEK_API_KEY").is_ok() {
            return Ok(Box::new(DeepSeekProvider::from_env()?));
        }

        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok(Box::new(OpenAiCompatibleLlm::from_env()?));
        }

        Err(AgentError::Llm(
            "No LLM provider configured. Please set one of:\n\
             - ANTHROPIC_AUTH_TOKEN (for Claude)\n\
             - KIMI_CODE_API_KEY (for Kimi Code)\n\
             - KIMI_API_KEY (for Moonshot)\n\
             - DEEPSEEK_API_KEY\n\
             - OPENAI_API_KEY".into()
        ))
    }

    /// Create an Anthropic provider from environment
    pub fn anthropic() -> Result<AnthropicLlm, AgentError> {
        AnthropicLlm::from_env()
    }

    /// Create a DeepSeek provider from environment
    pub fn deepseek() -> Result<DeepSeekProvider, AgentError> {
        DeepSeekProvider::from_env()
    }

    /// Create a Kimi provider from environment
    pub fn kimi() -> Result<KimiLlm, AgentError> {
        KimiLlm::from_env()
    }

    /// Create an OpenAI-compatible provider from environment
    pub fn openai() -> Result<OpenAiCompatibleLlm, AgentError> {
        OpenAiCompatibleLlm::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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

        let llm = OpenAiCompatibleLlm::new(
            "test-key",
            format!("http://127.0.0.1:{}", port),
            "gpt-4o",
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
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: Some("cache-key-123".into()),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json.get("model").unwrap(), "test-model");
        assert_eq!(json.get("prompt_cache_key").unwrap(), "cache-key-123");
    }

    #[test]
    fn test_chat_completion_request_serialization_without_cache_key() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("prompt_cache_key").is_none());
    }
}
