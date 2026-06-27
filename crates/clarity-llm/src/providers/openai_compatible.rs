//! Generic OpenAI-compatible LLM provider.
//!
//! Works with any API that follows the OpenAI chat completions format,
//! including Kimi, DeepSeek, and OpenAI itself.

use crate::api::{
    LlmProvider, LlmResponse, Message, MessageRole, ProviderCapabilities, StreamDelta,
};
use crate::rate_limit;
use crate::request::{
    ApiFunctionCall, ApiMessage, ApiToolCall, ChatCompletionRequest, ChatCompletionResponse,
    MAX_MESSAGE_BODY_BYTES, cap_tools_json, guard_request_body_size, truncate_messages_by_bytes,
};
use crate::sse;
use async_trait::async_trait;
use clarity_contract::AgentError;
use futures::StreamExt;
use serde_json::{Value, json};
use std::env;
use std::sync::Arc;

/// Generic OpenAI-compatible LLM provider.
///
/// Works with any API that follows the OpenAI chat completions format.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleLlm {
    api_key: Arc<parking_lot::RwLock<String>>,
    base_url: String,
    model: String,
    client: reqwest::Client,
    prompt_cache_key: Arc<parking_lot::RwLock<Option<String>>>,
}

impl OpenAiCompatibleLlm {
    /// Create a new OpenAI-compatible LLM provider.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: Arc::new(parking_lot::RwLock::new(api_key.into())),
            base_url: base_url.into(),
            model: model.into(),
            client: crate::shared_http_client(),
            prompt_cache_key: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Update the API key at runtime (used by OAuth token refresh).
    pub fn update_api_key(&self, key: impl Into<String>) {
        *self.api_key.write() = key.into();
    }

    /// Create from environment variables.
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

pub(crate) fn convert_api_messages(messages: &[Message]) -> Vec<ApiMessage> {
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
        // Convert internal Message to API message format.
        let api_messages = convert_api_messages(&messages);

        let tools_opt = cap_tools_json(
            tools
                .as_array()
                .filter(|a| !a.is_empty())
                .map(|_| tools.clone()),
        );
        let thinking_opt = if self.base_url.contains("kimi.com") {
            Some(json!({"type": "disabled"}))
        } else if self.base_url.contains("deepseek.com") {
            Some(json!({"type": "enabled"}))
        } else {
            None
        };
        let mut request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            temperature: None,
            max_tokens: None,
            stream: false,
            prompt_cache_key: self.prompt_cache_key.read().clone(),
            thinking: thinking_opt,
        };
        guard_request_body_size(&mut request_body);

        // Build URL: base_url should end with /v1, e.g. https://api.kimi.com/coding/v1
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };

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

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| AgentError::Llm(format!("Failed to parse response: {}", e)))?;

        let choice = completion.choices.into_iter().next();
        let content = choice
            .as_ref()
            .map(|c| {
                // Kimi Code API may return reasoning_content instead of content.
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

        let tools_opt = cap_tools_json(
            tools
                .as_array()
                .filter(|a| !a.is_empty())
                .map(|_| tools.clone()),
        );
        let thinking_opt = if self.base_url.contains("kimi.com") {
            Some(json!({"type": "disabled"}))
        } else {
            None
        };
        let mut request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: tools_opt,
            temperature: None,
            max_tokens: None,
            stream: true,
            prompt_cache_key: self.prompt_cache_key.read().clone(),
            thinking: thinking_opt,
        };
        guard_request_body_size(&mut request_body);

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
                        rate_limit::wait_for_retry_after(&resp).await;
                        let err = resp.text().await.unwrap_or_default();
                        let _ = tx
                            .send(Err(AgentError::Llm(format!("API error: {}", err))))
                            .await;
                        return;
                    }

                    let mut stream = resp.bytes_stream();
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

                    // Flush any remaining completed tool call when stream ends without [DONE].
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
            prompt_guided_tool_calling: false,
            prompt_caching: true,
            vision: false,
            pricing: Some(crate::api::Pricing {
                input_per_1m: 0.5,
                output_per_1m: 1.5,
            }),
        }
    }
}
