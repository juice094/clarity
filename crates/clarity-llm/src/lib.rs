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

pub mod api;
pub mod auth;
pub mod deepseek;
pub mod kalosm;
pub mod llama_server;
#[cfg(feature = "local-llm")]
pub mod local_gguf;
pub mod mcp_llm_provider;
pub mod mesh;
pub mod model_registry;
pub mod ollama;
pub mod policy;
pub mod registry_table;
pub mod reliable;
pub mod runtime;
pub mod runtime_router;
pub mod sse;
pub mod tool_payload;

/// Version of the clarity-llm crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Re-export provider types
pub use deepseek::DeepSeekProvider;
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
pub use reliable::ReliableProvider;

pub use api::{LlmProvider, LlmResponse, Message, MessageRole, ProviderCapabilities, StreamDelta};
pub use policy::{DefaultProviderSelectionPolicy, ProviderSelection, ProviderSelectionPolicy};
pub use tool_payload::{NativeToolAdapter, PromptGuidedAdapter, ToolPayloadAdapter};

use async_trait::async_trait;
use clarity_contract::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

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
const LOCAL_MODEL_HELP: &str = "No local model found. To use local inference:\n\
    1. Download a GGUF model (e.g. from https://huggingface.co)\n\
    2. Place it in ~/models/ or set CLARITY_LOCAL_MODEL_PATH to its full path\n\
    3. Optionally set CLARITY_LOCAL_TOKENIZER_REPO to a HuggingFace repo ID for the tokenizer";
use std::time::Duration;

static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_http_client() -> reqwest::Client {
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

/// Maximum content bytes to send in a single LLM request.
///
/// Providers such as DeepSeek and Kimi reject request bodies larger than ~2MB.
/// We budget 1.5MB for message content to leave room for JSON framing, tools,
/// and provider metadata.
const MAX_MESSAGE_BODY_BYTES: usize = 1_500_000;

/// Truncate message history to fit within a byte budget while preserving the
/// system prompt and the most recent user/assistant exchanges.
///
/// This is a last-line-of-defense guard for providers that enforce a maximum
/// request body size. It drops oldest non-system messages until the total
/// content size is below `max_bytes`, always keeping the final user message.
pub fn truncate_messages_by_bytes(messages: &[Message], max_bytes: usize) -> Vec<Message> {
    let total: usize = messages.iter().map(|m| m.content.len()).sum();
    if total <= max_bytes {
        return messages.to_vec();
    }

    let mut result = Vec::new();
    let mut start = 0;
    if !messages.is_empty() && messages[0].role == MessageRole::System {
        result.push(messages[0].clone());
        start = 1;
    }

    let mut kept = messages[start..].to_vec();
    let budget = max_bytes.saturating_sub(result.iter().map(|m| m.content.len()).sum::<usize>());

    while kept.len() > 1 {
        let current: usize = kept.iter().map(|m| m.content.len()).sum();
        if current <= budget {
            break;
        }
        kept.remove(0);
    }

    result.extend(kept);
    result
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
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    reasoning_content: Option<String>,
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
    // Intentionally retained: `finish_reason` is part of the OpenAI chat-completion
    // response schema and may be used for debugging / telemetry in the future.
    #[allow(dead_code)]
    #[serde(default)]
    finish_reason: Option<String>,
}

/// Generic OpenAI-compatible LLM provider
///
/// Works with any API that follows the OpenAI chat completions format
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleLlm {
    api_key: Arc<parking_lot::RwLock<String>>,
    base_url: String,
    model: String,
    client: reqwest::Client,
    prompt_cache_key: Arc<parking_lot::RwLock<Option<String>>>,
}

impl OpenAiCompatibleLlm {
    /// Create a new OpenAI-compatible LLM provider
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: Arc::new(parking_lot::RwLock::new(api_key.into())),
            base_url: base_url.into(),
            model: model.into(),
            client: shared_http_client(),
            prompt_cache_key: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Update the API key at runtime (used by OAuth token refresh).
    pub fn update_api_key(&self, key: impl Into<String>) {
        *self.api_key.write() = key.into();
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| AgentError::Llm("OPENAI_API_KEY not set".into()))?;

        let base_url =
            env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into());

        let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Set a key used to enable prompt caching for subsequent requests.
    pub fn set_prompt_cache_key(&self, key: impl Into<String>) {
        *self.prompt_cache_key.write() = Some(key.into());
    }
}

fn convert_api_messages(messages: &[Message]) -> Vec<ApiMessage> {
    messages
        .iter()
        .map(|m| {
            let reasoning_content = if m.role == MessageRole::Assistant && m.tool_calls.is_some() {
                Some("".to_string())
            } else {
                None
            };
            ApiMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
                tool_calls: m.tool_calls.clone().map(|tcs| {
                    tcs.into_iter()
                        .map(|tc| ApiToolCall {
                            id: tc.id,
                            call_type: tc.call_type,
                            function: ApiFunctionCall {
                                name: tc.function.name,
                                arguments: tc.function.arguments,
                            },
                        })
                        .collect()
                }),
                tool_call_id: m.tool_call_id.clone(),
                reasoning_content,
            }
        })
        .collect()
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        // Guard against providers that reject oversized request bodies.
        let messages = truncate_messages_by_bytes(messages, MAX_MESSAGE_BODY_BYTES);
        // Convert internal Message to API message format
        let api_messages = convert_api_messages(&messages);

        let tools_opt = tools
            .as_array()
            .filter(|a| !a.is_empty())
            .map(|_| tools.clone());
        let thinking_opt = if self.base_url.contains("kimi.com") {
            Some(json!({"type": "disabled"}))
        } else if self.base_url.contains("deepseek.com") {
            Some(json!({"type": "enabled"}))
        } else {
            None
        };
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: self.prompt_cache_key.read().clone(),
            thinking: thinking_opt,
        };

        // Build URL: base_url should end with /v1, e.g. https://api.kimi.com/coding/v1
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };

        tracing::debug!(
            "LLM complete request: {} messages, tools={}",
            request_body.messages.len(),
            serde_json::to_string(&request_body.tools).unwrap_or_default()
        );

        let response = self
            .client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.api_key.read().clone()),
            )
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
        let content = choice
            .as_ref()
            .map(|c| {
                // Kimi Code API may return reasoning_content instead of content
                if c.message.content.is_empty() {
                    c.message.reasoning_content.clone().unwrap_or_default()
                } else {
                    c.message.content.clone()
                }
            })
            .unwrap_or_default();
        let tool_calls: Vec<clarity_contract::ToolCall> = choice
            .and_then(|c| c.message.tool_calls)
            .map(|tcs| {
                tcs.into_iter()
                    .map(|tc| clarity_contract::ToolCall {
                        id: tc.id,
                        call_type: if tc.call_type.is_empty() {
                            "function".to_string()
                        } else {
                            tc.call_type
                        },
                        function: clarity_contract::FunctionCall {
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
        // Guard against providers that reject oversized request bodies.
        let messages = truncate_messages_by_bytes(messages, MAX_MESSAGE_BODY_BYTES);
        let api_messages = convert_api_messages(&messages);

        let tools_opt = tools
            .as_array()
            .filter(|a| !a.is_empty())
            .map(|_| tools.clone());
        let thinking_opt = if self.base_url.contains("kimi.com") {
            Some(json!({"type": "disabled"}))
        } else {
            None
        };
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            temperature: None,
            max_tokens: None,
            stream: true,
            prompt_cache_key: self.prompt_cache_key.read().clone(),
            thinking: thinking_opt,
        };

        tracing::debug!(
            "LLM stream request: {} messages, tools={}",
            request_body.messages.len(),
            serde_json::to_string(&request_body.tools).unwrap_or_default()
        );

        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };
        let api_key = self.api_key.read().clone();
        let client = self.client.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
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
                        let _ = tx
                            .send(Err(AgentError::Llm(format!("API error: {}", err))))
                            .await;
                        return;
                    }

                    let mut stream = resp.bytes_stream();
                    use futures::StreamExt;
                    let mut parser = sse::SseParser::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let text = String::from_utf8_lossy(&bytes);
                                for line in text.lines() {
                                    if let Some(data) = line.strip_prefix("data:") {
                                        let data = data.trim_start();
                                        let deltas = parser.process_line(data);
                                        for delta in deltas {
                                            if tx.send(Ok(delta)).await.is_err() {
                                                return;
                                            }
                                        }
                                        if data == "[DONE]" {
                                            return;
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

                    // Flush any remaining completed tool call when stream ends without [DONE]
                    if let Some(delta) = parser.flush() {
                        let _ = tx.send(Ok(delta)).await;
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

    fn set_prompt_cache_key(&self, key: &str) {
        self.set_prompt_cache_key(key);
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: true,
            prompt_caching: true,
            vision: false,
            pricing: Some(crate::api::Pricing {
                input_per_1m: 0.5,
                output_per_1m: 1.5,
            }),
        }
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
        let api_key =
            env::var("KIMI_API_KEY").map_err(|_| AgentError::Llm("KIMI_API_KEY not set".into()))?;

        let base_url =
            env::var("KIMI_BASE_URL").unwrap_or_else(|_| "https://api.moonshot.ai/v1".into());

        let model = env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2.6".into());

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

    fn set_prompt_cache_key(&self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: true,
            prompt_caching: true,
            vision: false,
            pricing: None,
        }
    }
}

/// OAuth-backed LLM Provider
///
/// Wraps an OpenAI-compatible LLM with automatic OAuth token refresh.
/// Supports any provider that uses OAuth 2.0 Device Authorization Grant.
#[derive(Debug, Clone)]
pub struct OAuthLlm {
    inner: OpenAiCompatibleLlm,
    token_manager: crate::auth::OAuthTokenManager,
}

impl OAuthLlm {
    /// Create with an explicit token manager.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        token_manager: crate::auth::OAuthTokenManager,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new(api_key, base_url, model),
            token_manager,
        }
    }

    /// Create with the default Kimi Code configuration (convenience alias).
    pub fn kimi_code() -> Self {
        Self {
            inner: OpenAiCompatibleLlm::new("", "https://api.kimi.com/coding/v1", "kimi-k2.6"),
            token_manager: crate::auth::OAuthTokenManager::new(),
        }
    }

    /// Create from environment variables (backward-compatible with KimiCodeLlm::from_env).
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("KIMI_CODE_API_KEY")
            .map_err(|_| AgentError::Llm("KIMI_CODE_API_KEY not set".into()))?;
        let base_url = env::var("KIMI_CODE_BASE_URL")
            .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into());
        let model = env::var("KIMI_CODE_MODEL").unwrap_or_else(|_| "kimi-k2.6".into());
        Ok(Self::new(
            api_key,
            base_url,
            model,
            crate::auth::OAuthTokenManager::new(),
        ))
    }

    /// Set a key used to enable prompt caching for subsequent requests.
    pub fn set_prompt_cache_key(&self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }
}

#[async_trait]
impl LlmProvider for OAuthLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        if let Ok(Some(token)) = self.token_manager.try_fresh().await {
            self.inner.update_api_key(token);
        }
        // If try_fresh returns None, fall back to the static api_key set at construction time.
        self.inner.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let inner = self.inner.clone();
        let token_manager = self.token_manager.clone();
        let messages = messages.to_vec();
        let tools = tools.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            if let Ok(Some(token)) = token_manager.try_fresh().await {
                inner.update_api_key(token);
            }
            match inner.stream(&messages, &tools) {
                Ok(mut inner_rx) => {
                    while let Some(item) = inner_rx.recv().await {
                        if tx.send(item).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                }
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&self, key: &str) {
        self.inner.set_prompt_cache_key(key);
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: true,
            prompt_caching: true,
            vision: false,
            pricing: None,
        }
    }
}

/// Backward-compatible alias for the Kimi Code LLM provider.
pub type KimiCodeLlm = OAuthLlm;

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
    /// Optional: ANTHROPIC_BASE_URL (default: `"https://api.anthropic.com"`)
    /// Optional: ANTHROPIC_MODEL (default: "claude-3-sonnet-20240229")
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| AgentError::Llm("ANTHROPIC_AUTH_TOKEN not set".into()))?;

        let base_url =
            env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".into());

        let model =
            env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-sonnet-20240229".into());

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
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        use crate::tool_payload::PromptGuidedAdapter;
        use crate::tool_payload::ToolPayloadAdapter;

        let (adapted_messages, _adapted_tools) = PromptGuidedAdapter.adapt(messages, tools);

        // Extract system message if present
        let system_msg = adapted_messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Convert messages (excluding system)
        let anthropic_messages: Vec<AnthropicMessage> = adapted_messages
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
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        use crate::tool_payload::PromptGuidedAdapter;
        use crate::tool_payload::ToolPayloadAdapter;

        let (adapted_messages, _adapted_tools) = PromptGuidedAdapter.adapt(messages, tools);

        // Extract system message if present
        let system_msg = adapted_messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Convert messages (excluding system)
        let anthropic_messages: Vec<AnthropicMessage> = adapted_messages
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
                        let _ = tx
                            .send(Err(AgentError::Llm(format!("API error: {}", err))))
                            .await;
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
                                        if let Ok(event) =
                                            serde_json::from_str::<serde_json::Value>(data)
                                        {
                                            // Anthropic streaming format: content_block_delta events
                                            if let Some(delta) = event.get("delta") {
                                                if let Some(text) =
                                                    delta.get("text").and_then(|t| t.as_str())
                                                {
                                                    if tx
                                                        .send(Ok(StreamDelta {
                                                            content: Some(text.to_string()),
                                                            tool_calls: vec![],
                                                        }))
                                                        .await
                                                        .is_err()
                                                    {
                                                        return;
                                                    }
                                                }
                                            }
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

    fn set_prompt_cache_key(&self, _key: &str) {}

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: false,
            pricing: Some(crate::api::Pricing {
                input_per_1m: 3.0,
                output_per_1m: 15.0,
            }),
            ..Default::default()
        }
    }
}

/// Factory for creating LLM providers.
///
/// **Frozen for new providers** — use `ModelRegistry::load()` +
/// `build_provider_from_registry()` for configuration-driven routing.
///
/// `auto()` / `create()` / `create_with_key()` remain active for backward
/// compatibility with existing callers (gateway, tui, egui). They first
/// consult the registry, then fall back to legacy env-var detection.
///
/// Provider-specific helpers (`anthropic`, `deepseek`, `kimi`, `openai`)
/// are deprecated; prefer registry aliases or `create_with_key()`.
pub struct LlmFactory;

impl LlmFactory {
    /// Auto-detect provider — uses ModelRegistry if available, otherwise legacy env-var scan.
    pub async fn auto() -> Result<Box<dyn LlmProvider>, AgentError> {
        // Try registry first
        match ModelRegistry::load_async().await {
            Ok(registry) => {
                if let Some(first) = registry.list_models().into_iter().next() {
                    return Self::create(&first.alias).await;
                }
            }
            Err(e) => {
                tracing::debug!(
                    "ModelRegistry not available ({}), falling back to legacy auto-detect",
                    e
                );
            }
        }

        // Legacy fallback: hard-coded env-var priority
        if env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
            return Ok(Box::new(AnthropicLlm::from_env()?));
        }

        if env::var("KIMI_CODE_API_KEY").is_ok() {
            return Ok(Box::new(KimiCodeLlm::from_env()?));
        }

        if let Ok(kimi_key) = env::var("KIMI_API_KEY") {
            if kimi_key.starts_with("sk-kimi-") {
                let base_url = env::var("KIMI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into());
                let model = env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2.6".into());
                return Ok(Box::new(OAuthLlm::new(
                    kimi_key,
                    base_url,
                    model,
                    crate::auth::OAuthTokenManager::new(),
                )));
            }
            return Ok(Box::new(KimiLlm::from_env()?));
        }

        if env::var("DEEPSEEK_API_KEY").is_ok() {
            return Ok(Box::new(DeepSeekProvider::from_env()?));
        }

        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok(Box::new(OpenAiCompatibleLlm::from_env()?));
        }

        #[cfg(feature = "local-llm")]
        if let Some(model_path) = resolve_local_model_path() {
            tracing::info!(
                "No cloud LLM configured; falling back to local GGUF model at {}",
                model_path.display()
            );
            let repo = std::env::var("CLARITY_LOCAL_TOKENIZER_REPO")
                .unwrap_or_else(|_| "Qwen/Qwen2.5-7B-Instruct".into());
            let config = LocalGgufConfig::new(model_path)?.with_tokenizer_repo(repo);
            return Ok(Box::new(LocalGgufProvider::new(config).await?));
        }

        Err(AgentError::Llm(
            "No LLM provider configured. Please set one of:\n\
             - ANTHROPIC_AUTH_TOKEN (for Claude)\n\
             - KIMI_CODE_API_KEY (for Kimi Code)\n\
             - KIMI_API_KEY (for Moonshot)\n\
             - DEEPSEEK_API_KEY\n\
             - OPENAI_API_KEY\n\
             Or create ~/.config/clarity/models.toml\n\
             Or use local inference:\n"
                .to_string()
                + LOCAL_MODEL_HELP,
        ))
    }

    /// Auto-detect provider, returning an `Arc` for direct use with `Agent::set_llm`.
    pub async fn auto_arc() -> Result<Arc<dyn LlmProvider>, AgentError> {
        Self::auto().await.map(Arc::from)
    }

    /// Create a provider by alias or legacy name.
    /// First checks ModelRegistry (with encrypted keys from the default
    /// secret store), then falls back to hard-coded legacy names.
    #[allow(deprecated)]
    pub async fn create(name: &str) -> Result<Box<dyn LlmProvider>, AgentError> {
        // Try registry first
        if let Ok(registry) = ModelRegistry::load_async().await {
            if let Some(entry) = registry.get(name) {
                if let Some(provider_cfg) = registry.get_provider(&entry.provider) {
                    let secrets = default_secret_store().ok();
                    return model_registry::build_provider_from_registry_entry(
                        provider_cfg,
                        entry,
                        None,
                        secrets.as_ref(),
                    )
                    .await;
                }
            }
        }

        // Legacy fallback
        let lower = name.to_lowercase();
        match lower.as_str() {
            "anthropic" | "claude" => Ok(Box::new(Self::anthropic()?)),
            "deepseek" => Ok(Box::new(Self::deepseek()?)),
            "openai" => Ok(Box::new(Self::openai()?)),
            "kimi" | "kimi-code" | "moonshot" | "kimi_code" => {
                if lower == "kimi_code" || env::var("KIMI_CODE_API_KEY").is_ok() {
                    Ok(Box::new(KimiCodeLlm::from_env()?))
                } else {
                    Ok(Box::new(Self::kimi()?))
                }
            }
            "kalosm" | "local" => {
                #[cfg(feature = "local-llm")]
                if let Some(model_path) = resolve_local_model_path() {
                    let repo = std::env::var("CLARITY_LOCAL_TOKENIZER_REPO")
                        .unwrap_or_else(|_| "Qwen/Qwen2.5-7B-Instruct".into());
                    let config = LocalGgufConfig::new(model_path)?.with_tokenizer_repo(repo);
                    return Ok(Box::new(LocalGgufProvider::new(config).await?));
                }
                Err(AgentError::Llm(
                    "Local LLM not available. Ensure the local-llm feature is enabled.\n"
                        .to_string()
                        + LOCAL_MODEL_HELP,
                ))
            }
            _ => Err(AgentError::Llm(format!(
                "Unknown model alias '{}'. Create ~/.config/clarity/models.toml or use a legacy name: anthropic, kimi, deepseek, openai, kalosm",
                name
            ))),
        }
    }

    /// Create a provider by alias, returning an `Arc` for direct use with `Agent::set_llm`.
    pub async fn create_arc(name: &str) -> Result<Arc<dyn LlmProvider>, AgentError> {
        Self::create(name).await.map(Arc::from)
    }

    /// Create a provider with an explicit API key, bypassing environment variables.
    /// Used by the Tauri GUI so users can configure provider + key through Settings.
    pub fn create_with_key(
        name: &str,
        api_key: &str,
        model: &str,
    ) -> Result<Box<dyn LlmProvider>, AgentError> {
        let lower = name.to_lowercase();
        // kimi_code supports OAuth: empty key is okay (token loaded from file)
        if api_key.is_empty() && lower != "kimi_code" {
            return Err(AgentError::Llm(format!(
                "Provider '{}' requires an API key. Please enter it in Settings.",
                name
            )));
        }
        match lower.as_str() {
            "anthropic" | "claude" => Ok(Box::new(AnthropicLlm::new(
                api_key,
                "https://api.anthropic.com",
                model,
            ))),
            "deepseek" => Ok(Box::new(DeepSeekProvider::new(
                api_key,
                "https://api.deepseek.com/v1",
                if model.is_empty() {
                    "deepseek-chat"
                } else {
                    model
                },
            ))),
            "openai" => Ok(Box::new(OpenAiCompatibleLlm::new(
                api_key,
                "https://api.openai.com/v1",
                if model.is_empty() { "gpt-4o" } else { model },
            ))),
            "kimi" | "kimi-code" | "moonshot" | "kimi_code" => {
                let is_kimi_code = lower == "kimi_code" || api_key.starts_with("sk-kimi-");
                if is_kimi_code {
                    Ok(Box::new(OAuthLlm::new(
                        api_key,
                        "https://api.kimi.com/coding/v1",
                        if model.is_empty() { "kimi-k2.6" } else { model },
                        crate::auth::OAuthTokenManager::new(),
                    )))
                } else {
                    Ok(Box::new(KimiLlm::new(
                        api_key,
                        "https://api.moonshot.ai/v1",
                        if model.is_empty() { "kimi-k2.6" } else { model },
                    )))
                }
            }
            "ollama" => Ok(Box::new(OpenAiCompatibleLlm::new(
                api_key, // Ollama usually doesn't need a key, but we pass it anyway
                "http://localhost:11434/v1",
                if model.is_empty() { "llama3.2" } else { model },
            ))),
            _ => Err(AgentError::Llm(format!(
                "Unknown provider '{}'. Supported: openai, anthropic, kimi, deepseek, ollama, local",
                name
            ))),
        }
    }

    /// `Arc` wrapper for `create_with_key`.
    pub fn create_with_key_arc(
        name: &str,
        api_key: &str,
        model: &str,
    ) -> Result<Arc<dyn LlmProvider>, AgentError> {
        Self::create_with_key(name, api_key, model).map(Arc::from)
    }

    /// Create an Anthropic provider from environment
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn anthropic() -> Result<AnthropicLlm, AgentError> {
        AnthropicLlm::from_env()
    }

    /// Create a DeepSeek provider from environment
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn deepseek() -> Result<DeepSeekProvider, AgentError> {
        DeepSeekProvider::from_env()
    }

    /// Create a Kimi provider from environment
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn kimi() -> Result<KimiLlm, AgentError> {
        KimiLlm::from_env()
    }

    /// Create an OpenAI-compatible provider from environment
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
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

        let llm =
            OpenAiCompatibleLlm::new("test-key", format!("http://127.0.0.1:{}", port), "gpt-4o");
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
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("prompt_cache_key").is_none());
    }
}
