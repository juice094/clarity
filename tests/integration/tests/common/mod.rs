#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
#![allow(dead_code)]

use async_trait::async_trait;
use clarity_core::agent::StreamDelta;
use clarity_core::agent::{LlmProvider, LlmResponse, Message, ToolCall};
use clarity_core::error::AgentError;
use parking_lot::Mutex;
use serde_json::Value;

/// A mock LLM that returns a predetermined sequence of responses.
/// Once the sequence is exhausted it returns a simple text response.
pub struct SequentialMockLlm {
    responses: Mutex<Vec<LlmResponse>>,
}

impl SequentialMockLlm {
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
        }
    }
}

#[async_trait]
impl LlmProvider for SequentialMockLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let mut responses = self.responses.lock();
        if responses.is_empty() {
            Ok(LlmResponse {
                content: "Done".to_string(),
                tool_calls: vec![],
                is_complete: true,
            })
        } else {
            Ok(responses.remove(0))
        }
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamDelta {
                    content: Some("Done".to_string()),
                    tool_calls: vec![],
                    partial_tool_calls: vec![],
                    reasoning_content: None,
                }))
                .await;
        });
        Ok(rx)
    }

    fn set_prompt_cache_key(&self, _key: &str) {}
}

/// Convenience builder for a simple text-only mock response.
pub fn text_response(content: impl Into<String>) -> LlmResponse {
    LlmResponse {
        content: content.into(),
        tool_calls: vec![],
        is_complete: true,
    }
}

/// Convenience builder for a response that triggers a tool call.
pub fn tool_call_response(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> LlmResponse {
    LlmResponse {
        content: content.into(),
        tool_calls,
        is_complete: false,
    }
}
