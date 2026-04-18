//! Local LLM inference via Kalosm (Candle-based)
//!
//! Supports GGUF models (Qwen2.5, Llama, etc.) with CUDA acceleration.
//! Enabled via the `local-llm` Cargo feature.

use super::{LlmProvider, LlmResponse, Message, StreamDelta};
#[cfg(feature = "local-llm")]
use crate::agent::{FunctionCall, ToolCall};
use crate::error::AgentError;
use serde_json::Value;
use std::path::PathBuf;
#[cfg(feature = "local-llm")]
use std::sync::Arc;

#[cfg(feature = "local-llm")]
use kalosm::language::*;

/// Configuration for Kalosm local inference
#[derive(Debug, Clone)]
pub struct KalosmConfig {
    /// Path to the GGUF model file
    pub model_path: PathBuf,
    /// Maximum context length (tokens)
    pub max_context_length: usize,
    /// Sampling temperature
    pub temperature: f32,
}

impl Default for KalosmConfig {
    fn default() -> Self {
        Self {
            model_path: default_model_path(),
            max_context_length: 4096,
            temperature: 0.7,
        }
    }
}

impl KalosmConfig {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            ..Default::default()
        }
    }

    pub fn with_max_context_length(mut self, length: usize) -> Self {
        self.max_context_length = length;
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }
}

/// Discover default model path from known locations
fn default_model_path() -> PathBuf {
    let candidates = [
        PathBuf::from(r"C:\Users\22414\Desktop\model\Qwen2.5-7B-Instruct.Q4_K_M.gguf"),
        PathBuf::from(r"C:\Users\22414\Desktop\model\Qwen2.5-14B-Instruct.Q4_K_M.gguf"),
    ];
    for path in &candidates {
        if path.exists() {
            return path.clone();
        }
    }
    candidates[0].clone()
}

/// Kalosm-based local LLM provider
#[cfg(feature = "local-llm")]
pub struct KalosmProvider {
    model: Arc<tokio::sync::Mutex<Llama>>,
    #[allow(dead_code)]
    config: KalosmConfig,
    cache_key: Option<String>,
}

#[cfg(feature = "local-llm")]
impl KalosmProvider {
    pub async fn new(config: KalosmConfig) -> Result<Self, AgentError> {
        if !config.model_path.exists() {
            return Err(AgentError::Llm(format!(
                "Model not found at {}. Please download a GGUF model (e.g., Qwen2.5-7B-Instruct.Q4_K_M.gguf) and place it in ~/Desktop/model/",
                config.model_path.display()
            )));
        }

        let source = LlamaSource::new(FileSource::local(config.model_path.clone()));
        let model = Llama::builder()
            .with_source(source)
            .build()
            .await
            .map_err(|e| AgentError::Llm(format!("Failed to load Kalosm model: {}", e)))?;

        Ok(Self {
            model: Arc::new(tokio::sync::Mutex::new(model)),
            config,
            cache_key: None,
        })
    }

    /// Format clarity messages into Qwen2.5 chat template prompt
    fn format_messages(&self, messages: &[Message], tools: &Value) -> String {
        let mut prompt = String::new();
        let mut system_prompt = String::new();
        let mut conversation = Vec::new();

        for msg in messages {
            match msg.role {
                crate::agent::MessageRole::System => {
                    system_prompt.push_str(&msg.content);
                }
                crate::agent::MessageRole::User => {
                    conversation.push(("user", msg.content.clone()));
                }
                crate::agent::MessageRole::Assistant => {
                    conversation.push(("assistant", msg.content.clone()));
                }
                crate::agent::MessageRole::Tool => {
                    conversation.push(("tool", msg.content.clone()));
                }
            }
        }

        // Append tool descriptions to system prompt if tools are provided
        if !tools.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            system_prompt.push_str("\n\nYou have access to the following tools. When you need to use a tool, output a JSON object in this exact format on its own line:\n");
            system_prompt.push_str(r#"{"tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "tool_name", "arguments": {"arg1": "value1"}}}]"#);
            system_prompt.push_str("}\n\nAvailable tools:\n");
            if let Some(arr) = tools.as_array() {
                for tool in arr {
                    if let Some(func) = tool.get("function") {
                        let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let desc = func.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        system_prompt.push_str(&format!("- {}: {}\n", name, desc));
                    }
                }
            }
        }

        // Build Qwen2.5 chat template
        if !system_prompt.is_empty() {
            prompt.push_str("<|im_start|>system\n");
            prompt.push_str(&system_prompt);
            prompt.push_str("<|im_end|>\n");
        }

        for (role, content) in &conversation {
            match *role {
                "user" => {
                    prompt.push_str("<|im_start|>user\n");
                    prompt.push_str(content);
                    prompt.push_str("<|im_end|>\n");
                }
                "assistant" => {
                    prompt.push_str("<|im_start|>assistant\n");
                    prompt.push_str(content);
                    prompt.push_str("<|im_end|>\n");
                }
                "tool" => {
                    prompt.push_str("<|im_start|>tool\n");
                    prompt.push_str(content);
                    prompt.push_str("<|im_end|>\n");
                }
                _ => {}
            }
        }

        // Final assistant prefix to trigger generation
        prompt.push_str("<|im_start|>assistant\n");

        prompt
    }

    /// Parse tool calls from generated text
    fn parse_tool_calls(&self, text: &str) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();

        // Try to find JSON tool_calls array
        if let Some(start) = text.find(r#"{"tool_calls":"#) {
            if let Some(end) = text[start..].find("]}") {
                let json_str = &text[start..start + end + 2];
                if let Ok(value) = serde_json::from_str::<Value>(json_str) {
                    if let Some(calls) = value.get("tool_calls").and_then(|v| v.as_array()) {
                        for (idx, call) in calls.iter().enumerate() {
                            if let (Some(name), Some(args)) = (
                                call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()),
                                call.get("function").and_then(|f| f.get("arguments")),
                            ) {
                                let id = call
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&format!("call_{}", idx))
                                    .to_string();
                                tool_calls.push(ToolCall {
                                    id,
                                    call_type: "function".to_string(),
                                    function: FunctionCall {
                                        name: name.to_string(),
                                        arguments: args.to_string(),
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }

        tool_calls
    }

    /// Extract just the assistant response (remove trailing template tokens)
    fn extract_response(&self, text: &str) -> String {
        text.trim()
            .trim_end_matches("<|im_end|>")
            .trim_end_matches("<|im_start|>")
            .trim()
            .to_string()
    }
}

#[cfg(feature = "local-llm")]
#[async_trait::async_trait]
impl LlmProvider for KalosmProvider {
    async fn complete(&self, messages: &[Message], tools: &Value) -> Result<LlmResponse, AgentError> {
        let prompt = self.format_messages(messages, tools);
        let model = self.model.lock().await;
        let mut chat = model.chat();

        let mut stream = chat(&prompt);
        let mut answer = String::new();

        while let Some(token) = stream.next().await {
            answer.push_str(token.as_ref());
        }

        let response_text = self.extract_response(&answer);
        let tool_calls = self.parse_tool_calls(&response_text);

        // If tool calls were found, strip the JSON from the content
        let content = if !tool_calls.is_empty() {
            response_text
                .split(r#"{"tool_calls":"#)
                .next()
                .unwrap_or(&response_text)
                .trim()
                .to_string()
        } else {
            response_text
        };

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
        let prompt = self.format_messages(messages, tools);
        let model = self.model.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            let model = model.lock().await;
            let mut chat = model.chat();
            let mut stream = chat(&prompt);

            while let Some(token) = stream.next().await {
                let text: &str = token.as_ref();
                if tx
                    .send(Ok(StreamDelta {
                        content: Some(text.to_string()),
                        tool_calls: vec![],
                    }))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.cache_key = Some(key.to_string());
    }
}

/// Stub implementation when `local-llm` feature is disabled
#[cfg(not(feature = "local-llm"))]
pub struct KalosmProvider {
    cache_key: Option<String>,
}

#[cfg(not(feature = "local-llm"))]
impl KalosmProvider {
    pub async fn new(_config: KalosmConfig) -> Result<Self, AgentError> {
        Err(AgentError::Llm(
            "Kalosm provider requires the `local-llm` feature to be enabled".to_string(),
        ))
    }
}

#[cfg(not(feature = "local-llm"))]
#[async_trait::async_trait]
impl LlmProvider for KalosmProvider {
    async fn complete(&self, _messages: &[Message], _tools: &Value) -> Result<LlmResponse, AgentError> {
        Err(AgentError::Llm(
            "Kalosm provider requires the `local-llm` feature".to_string(),
        ))
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        Err(AgentError::Llm(
            "Kalosm provider requires the `local-llm` feature".to_string(),
        ))
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.cache_key = Some(key.to_string());
    }
}
