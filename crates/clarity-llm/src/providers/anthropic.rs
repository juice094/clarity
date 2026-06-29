//! Anthropic LLM provider.
//!
//! Supports Anthropic Messages API format.
//! Used by Kimi Code and other Claude-compatible endpoints.

use crate::api::{
    LlmProvider, LlmResponse, Message, MessageRole, ProviderCapabilities, StreamDelta,
};
use crate::rate_limit;
use crate::tool_payload;
use async_trait::async_trait;
use clarity_contract::AgentError;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

/// Anthropic LLM provider.
///
/// Supports Anthropic Messages API format.
/// Used by Kimi Code and other Claude-compatible endpoints.
#[derive(Debug, Clone)]
pub struct AnthropicLlm {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

/// Anthropic API request body.
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

/// Single message in an Anthropic request.
#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Anthropic API (non-streaming) response.
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

/// One content block in an Anthropic response.
#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

impl AnthropicLlm {
    /// Create from environment variables.
    ///
    /// Required: `ANTHROPIC_AUTH_TOKEN`
    /// Optional: `ANTHROPIC_BASE_URL` (default: `"https://api.anthropic.com"`)
    /// Optional: `ANTHROPIC_MODEL` (default: `"claude-3-sonnet-20240229"`)
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("ANTHROPIC_AUTH_TOKEN")
            .map_err(|_| AgentError::Llm("ANTHROPIC_AUTH_TOKEN not set".into()))?;

        let base_url =
            env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".into());

        let model =
            env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-sonnet-20240229".into());

        Ok(Self::new(api_key, base_url, model))
    }

    /// Create with explicit parameters.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to build Anthropic reqwest Client with timeout, falling back: {}",
                    e
                );
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(300))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new())
            });
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            client,
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
        let (adapted_messages, _adapted_tools) = tool_payload::adapt_prompt_guided(messages, tools);

        // Extract system message if present.
        let system_msg = adapted_messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Convert messages (excluding system).
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
            rate_limit::wait_for_retry_after(&response).await;
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
        let (adapted_messages, _adapted_tools) = tool_payload::adapt_prompt_guided(messages, tools);

        // Extract system message if present.
        let system_msg = adapted_messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Convert messages (excluding system).
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
                        rate_limit::wait_for_retry_after(&resp).await;
                        let err = resp.text().await.unwrap_or_default();
                        let _ = tx
                            .send(Err(AgentError::Llm(format!("API error: {}", err))))
                            .await;
                        return;
                    }

                    // Process SSE stream.
                    let mut stream = resp.bytes_stream();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let text = String::from_utf8_lossy(&bytes);
                                for line in text.lines() {
                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            return;
                                        }
                                        // Parse SSE event and extract content.
                                        if let Ok(event) = serde_json::from_str::<Value>(data) {
                                            // Anthropic streaming format: content_block_delta events.
                                            if let Some(delta) = event.get("delta") {
                                                if let Some(text) =
                                                    delta.get("text").and_then(|t| t.as_str())
                                                {
                                                    if tx
                                                        .send(Ok(StreamDelta {
                                                            content: Some(text.to_string()),
                                                            reasoning_content: None,
                                                            tool_calls: vec![],
                                                            partial_tool_calls: vec![],
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
