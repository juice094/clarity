//! Local LLM inference via Candle (zero-dependency, GGUF quantized)
//!
//! Loads and runs GGUF-quantized models directly in Rust without requiring
//! external binaries (Ollama, llama.cpp server, etc.).
//!
//! Supported architectures:
//! - Qwen2 / Qwen2.5 (including DeepSeek-R1-Distill-Qwen variants)
//!
//! ## Quick start
//!
//! ```no_run
//! use clarity_core::llm::local_gguf::{LocalGgufConfig, LocalGgufProvider};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = LocalGgufConfig::new("model.gguf")
//!     .with_tokenizer_repo("deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B");
//! let provider = LocalGgufProvider::new(config).await?;
//! # Ok(())
//! # }
//! ```

use super::{LlmProvider, LlmResponse, Message, StreamDelta};
use crate::agent::{FunctionCall, ToolCall};
use crate::error::AgentError;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights as Qwen2Model;

/// Chat template used for formatting messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatTemplate {
    /// Standard Qwen2 / Qwen2.5 instruct template.
    /// Uses `<|im_start|>` / `<|im_end|>` tokens.
    #[default]
    Qwen2,
    /// DeepSeek-R1-Distill-Qwen template.
    /// Uses `<｜User｜>` / `<｜Assistant｜>` tokens.
    DeepSeekR1,
}

impl ChatTemplate {
    /// Detect template from model name / path.
    pub fn detect(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("deepseek") && lower.contains("r1") {
            Self::DeepSeekR1
        } else {
            Self::Qwen2
        }
    }

    /// Format messages into a prompt string.
    fn format(&self, messages: &[Message], tools: &Value) -> String {
        match self {
            ChatTemplate::Qwen2 => Self::format_qwen2(messages, tools),
            ChatTemplate::DeepSeekR1 => Self::format_deepseek_r1(messages, tools),
        }
    }

    fn format_qwen2(messages: &[Message], tools: &Value) -> String {
        let mut prompt = String::new();
        let mut system = String::new();
        let mut conversation = Vec::new();

        for msg in messages {
            match msg.role {
                crate::agent::MessageRole::System => {
                    system.push_str(&msg.content);
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

        // Append tool descriptions to system prompt
        if !tools.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            system.push_str("\n\nYou have access to the following tools. When you need to use a tool, output a JSON object in this exact format on its own line:\n");
            system.push_str(r#"{"tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "tool_name", "arguments": {"arg1": "value1"}}}]"#);
            system.push_str("}\n\nAvailable tools:\n");
            if let Some(arr) = tools.as_array() {
                for tool in arr {
                    if let Some(func) = tool.get("function") {
                        let name = func
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let desc = func
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        system.push_str(&format!("- {}: {}\n", name, desc));
                    }
                }
            }
        }

        if !system.is_empty() {
            prompt.push_str("<|im_start|>system\n");
            prompt.push_str(&system);
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

        prompt.push_str("<|im_start|>assistant\n");
        prompt
    }

    fn format_deepseek_r1(messages: &[Message], tools: &Value) -> String {
        let mut result = String::new();
        let mut system = String::new();

        for msg in messages {
            if msg.role == crate::agent::MessageRole::System {
                system.push_str(&msg.content);
            }
        }

        if !tools.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            system.push_str("\n\nYou have access to the following tools. When you need to use a tool, output a JSON object in this exact format on its own line:\n");
            system.push_str(r#"{"tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "tool_name", "arguments": {"arg1": "value1"}}}]"#);
            system.push_str("}\n\nAvailable tools:\n");
            if let Some(arr) = tools.as_array() {
                for tool in arr {
                    if let Some(func) = tool.get("function") {
                        let name = func
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let desc = func
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        system.push_str(&format!("- {}: {}\n", name, desc));
                    }
                }
            }
        }

        if !system.is_empty() {
            result.push_str(&system);
            result.push('\n');
        }

        for msg in messages {
            match msg.role {
                crate::agent::MessageRole::User => {
                    result.push_str("<｜User｜>");
                    result.push_str(&msg.content);
                }
                crate::agent::MessageRole::Assistant => {
                    result.push_str("<｜Assistant｜>");
                    result.push_str(&msg.content);
                }
                _ => {}
            }
        }

        result.push_str("<｜Assistant｜>");
        result
    }

    fn eos_token(&self) -> &'static str {
        match self {
            ChatTemplate::Qwen2 => "<|im_end|>",
            ChatTemplate::DeepSeekR1 => "<｜end▁of▁sentence｜>",
        }
    }
}

/// Configuration for local GGUF inference.
#[derive(Debug, Clone)]
pub struct LocalGgufConfig {
    /// Path to the GGUF model file.
    pub model_path: PathBuf,
    /// HuggingFace repo ID to download tokenizer from (e.g. "deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B").
    pub tokenizer_repo: Option<String>,
    /// Local path to tokenizer.json (overrides `tokenizer_repo` if set).
    pub tokenizer_path: Option<PathBuf>,
    /// Maximum tokens to generate.
    pub max_tokens: usize,
    /// Sampling temperature (0.0 = greedy).
    pub temperature: f64,
    /// Top-p nucleus sampling cutoff.
    pub top_p: Option<f64>,
    /// Top-k sampling cutoff.
    pub top_k: Option<usize>,
    /// Random seed for sampling.
    pub seed: u64,
    /// Repeat penalty (1.0 = no penalty).
    pub repeat_penalty: f32,
    /// Number of recent tokens to consider for repeat penalty.
    pub repeat_last_n: usize,
    /// Chat template to use.
    pub chat_template: ChatTemplate,
}

impl Default for LocalGgufConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            tokenizer_repo: None,
            tokenizer_path: None,
            max_tokens: 2048,
            temperature: 0.7,
            top_p: None,
            top_k: None,
            seed: 299792458,
            repeat_penalty: 1.1,
            repeat_last_n: 64,
            chat_template: ChatTemplate::Qwen2,
        }
    }
}

impl LocalGgufConfig {
    /// Create config with a model path.
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        let path = model_path.into();
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if ext != "gguf" {
                panic!("LocalGgufConfig: model file must have .gguf extension, got: {}", path.display());
            }
        } else {
            panic!("LocalGgufConfig: model file must have .gguf extension, got: {}", path.display());
        }
        let template =
            ChatTemplate::detect(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""));
        Self {
            model_path: path,
            chat_template: template,
            ..Default::default()
        }
    }

    pub fn with_tokenizer_repo(mut self, repo: impl Into<String>) -> Self {
        self.tokenizer_repo = Some(repo.into());
        self
    }

    pub fn with_tokenizer_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.tokenizer_path = Some(path.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_chat_template(mut self, template: ChatTemplate) -> Self {
        self.chat_template = template;
        self
    }
}

/// Helper to stream-decode tokens into UTF-8 text chunks.
struct TokenOutputStream {
    tokenizer: tokenizers::Tokenizer,
    tokens: Vec<u32>,
    prev_tokens: Vec<u32>,
}

impl TokenOutputStream {
    fn new(tokenizer: tokenizers::Tokenizer) -> Self {
        Self {
            tokenizer,
            tokens: Vec::new(),
            prev_tokens: Vec::new(),
        }
    }

    fn next_token(&mut self, token: u32) -> Result<Option<String>, AgentError> {
        self.tokens.push(token);
        let text = self
            .tokenizer
            .decode(&self.tokens, false)
            .map_err(|e| AgentError::Llm(format!("Tokenizer decode error: {}", e)))?;
        let prev_text = self
            .tokenizer
            .decode(&self.prev_tokens, false)
            .map_err(|e| AgentError::Llm(format!("Tokenizer decode error: {}", e)))?;
        let new_text = text.strip_prefix(&prev_text).unwrap_or(&text).to_string();
        self.prev_tokens.push(token);
        Ok(if new_text.is_empty() {
            None
        } else {
            Some(new_text)
        })
    }

    fn decode_rest(&self) -> Result<Option<String>, AgentError> {
        let text = self
            .tokenizer
            .decode(&self.tokens, false)
            .map_err(|e| AgentError::Llm(format!("Tokenizer decode error: {}", e)))?;
        let prev_text = self
            .tokenizer
            .decode(&self.prev_tokens, false)
            .map_err(|e| AgentError::Llm(format!("Tokenizer decode error: {}", e)))?;
        let rest = text.strip_prefix(&prev_text).unwrap_or("").to_string();
        Ok(if rest.is_empty() { None } else { Some(rest) })
    }
}

/// Candle-based local GGUF provider.
pub struct LocalGgufProvider {
    model: Arc<tokio::sync::Mutex<Qwen2Model>>,
    tokenizer: tokenizers::Tokenizer,
    device: Device,
    config: LocalGgufConfig,
    cache_key: Option<String>,
}

impl LocalGgufProvider {
    /// Load a model from the given configuration.
    pub async fn new(config: LocalGgufConfig) -> Result<Self, AgentError> {
        if !config.model_path.exists() {
            return Err(AgentError::Llm(format!(
                "Model file not found: {}. Please download a GGUF model and place it at this path.",
                config.model_path.display()
            )));
        }

        let device = pick_device()
            .map_err(|e| AgentError::Llm(format!("Failed to initialize compute device: {}", e)))?;
        tracing::info!("LocalGgufProvider using device: {:?}", device);

        // Load model (blocking I/O + heavy computation)
        let model_path = config.model_path.clone();
        let device_for_load = device.clone();
        let model = tokio::task::spawn_blocking(move || load_model(&model_path, &device_for_load))
            .await
            .map_err(|e| AgentError::Llm(format!("Model load task panicked: {}", e)))??;

        // Load tokenizer
        let tokenizer = load_tokenizer(&config).await?;

        Ok(Self {
            model: Arc::new(tokio::sync::Mutex::new(model)),
            tokenizer,
            device,
            config,
            cache_key: None,
        })
    }

    /// Run text generation for the given prompt.
    async fn generate(
        &self,
        prompt: &str,
        max_tokens: usize,
        tx: Option<tokio::sync::mpsc::Sender<Result<StreamDelta, AgentError>>>,
    ) -> Result<String, AgentError> {
        let mut model = self.model.lock().await;
        let tokenizer = self.tokenizer.clone();
        let device = self.device.clone();
        let config = self.config.clone();
        // Note: KV cache is automatically reset on the first forward call
        // with index_pos=0, so no explicit clearing is needed.

        // Encode prompt
        let tokens = tokenizer
            .encode(prompt.to_string(), true)
            .map_err(|e| AgentError::Llm(format!("Tokenizer encode error: {}", e)))?;
        let prompt_tokens = tokens.get_ids().to_vec();
        let prompt_len = prompt_tokens.len();

        if prompt_tokens.is_empty() {
            return Err(AgentError::Llm("Empty prompt after tokenization".into()));
        }

        // Build logits processor
        let sampling = if config.temperature <= 0.0 {
            Sampling::ArgMax
        } else {
            match (config.top_k, config.top_p) {
                (None, None) => Sampling::All {
                    temperature: config.temperature,
                },
                (Some(k), None) => Sampling::TopK {
                    k,
                    temperature: config.temperature,
                },
                (None, Some(p)) => Sampling::TopP {
                    p,
                    temperature: config.temperature,
                },
                (Some(k), Some(p)) => Sampling::TopKThenTopP {
                    k,
                    p,
                    temperature: config.temperature,
                },
            }
        };
        let mut logits_processor = LogitsProcessor::from_sampling(config.seed, sampling);

        // Forward pass for prompt tokens
        let input = Tensor::new(prompt_tokens.as_slice(), &device)
            .map_err(|e| AgentError::Llm(format!("Tensor creation error: {}", e)))?
            .unsqueeze(0)
            .map_err(|e| AgentError::Llm(format!("Tensor unsqueeze error: {}", e)))?;
        let logits = model
            .forward(&input, 0)
            .map_err(|e| AgentError::Llm(format!("Model forward error: {}", e)))?;
        let logits = logits
            .squeeze(0)
            .map_err(|e| AgentError::Llm(format!("Tensor squeeze error: {}", e)))?;
        let mut next_token = logits_processor
            .sample(&logits)
            .map_err(|e| AgentError::Llm(format!("Sampling error: {}", e)))?;

        let eos_token_id = tokenizer
            .get_vocab(true)
            .get(config.chat_template.eos_token())
            .copied();

        let mut all_tokens = vec![next_token];
        let mut token_stream = TokenOutputStream::new(tokenizer);
        let mut generated_text = String::new();

        // Decode first token
        if let Some(text) = token_stream.next_token(next_token)? {
            if let Some(ref sender) = tx {
                let _ = sender
                    .send(Ok(StreamDelta {
                        content: Some(text.clone()),
                        tool_calls: vec![],
                    }))
                    .await;
            }
            generated_text.push_str(&text);
        }

        // Auto-regressive generation
        for index in 0..max_tokens {
            let input = Tensor::new(&[next_token], &device)
                .map_err(|e| AgentError::Llm(format!("Tensor creation error: {}", e)))?
                .unsqueeze(0)
                .map_err(|e| AgentError::Llm(format!("Tensor unsqueeze error: {}", e)))?;
            let logits = model
                .forward(&input, prompt_len + index)
                .map_err(|e| AgentError::Llm(format!("Model forward error: {}", e)))?;
            let logits = logits
                .squeeze(0)
                .map_err(|e| AgentError::Llm(format!("Tensor squeeze error: {}", e)))?;

            // Apply repeat penalty
            let logits = if config.repeat_penalty == 1.0 {
                logits
            } else {
                let start_at = all_tokens.len().saturating_sub(config.repeat_last_n);
                candle_transformers::utils::apply_repeat_penalty(
                    &logits,
                    config.repeat_penalty,
                    &all_tokens[start_at..],
                )
                .map_err(|e| AgentError::Llm(format!("Repeat penalty error: {}", e)))?
            };

            next_token = logits_processor
                .sample(&logits)
                .map_err(|e| AgentError::Llm(format!("Sampling error: {}", e)))?;
            all_tokens.push(next_token);

            if let Some(text) = token_stream.next_token(next_token)? {
                if let Some(ref sender) = tx {
                    let _ = sender
                        .send(Ok(StreamDelta {
                            content: Some(text.clone()),
                            tool_calls: vec![],
                        }))
                        .await;
                }
                generated_text.push_str(&text);
            }

            if Some(next_token) == eos_token_id {
                break;
            }
        }

        if let Some(rest) = token_stream.decode_rest()? {
            if let Some(ref sender) = tx {
                let _ = sender
                    .send(Ok(StreamDelta {
                        content: Some(rest.clone()),
                        tool_calls: vec![],
                    }))
                    .await;
            }
            generated_text.push_str(&rest);
        }

        Ok(generated_text)
    }

    /// Parse tool calls from generated text.
    fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();

        if let Some(start) = text.find(r#"{"tool_calls":"#) {
            if let Some(end) = text[start..].find("]}") {
                let json_str = &text[start..start + end + 2];
                if let Ok(value) = serde_json::from_str::<Value>(json_str) {
                    if let Some(calls) = value.get("tool_calls").and_then(|v| v.as_array()) {
                        for (idx, call) in calls.iter().enumerate() {
                            if let (Some(name), Some(args)) = (
                                call.get("function")
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str()),
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
}

#[async_trait::async_trait]
impl LlmProvider for LocalGgufProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let prompt = self.config.chat_template.format(messages, tools);
        tracing::debug!("LocalGguf prompt:\n{}", prompt);

        let generated = self.generate(&prompt, self.config.max_tokens, None).await?;

        // Strip EOS token from output
        let eos = self.config.chat_template.eos_token();
        let response_text = generated.trim_end_matches(eos).trim().to_string();

        let tool_calls = LocalGgufProvider::parse_tool_calls(&response_text);
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
        let prompt = self.config.chat_template.format(messages, tools);
        let max_tokens = self.config.max_tokens;
        let model = self.model.clone();
        let tokenizer = self.tokenizer.clone();
        let device = self.device.clone();
        let config = self.config.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            // Run generation with cloned state
            let result = generate_with_state(
                model,
                tokenizer,
                device,
                config,
                &prompt,
                max_tokens,
                Some(tx.clone()),
            )
            .await;

            if let Err(e) = result {
                let _ = tx.send(Err(e)).await;
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, key: &str) {
        self.cache_key = Some(key.to_string());
    }

    fn capabilities(&self) -> crate::llm::api::ProviderCapabilities {
        crate::llm::api::ProviderCapabilities {
            native_tool_calling: true,
            ..Default::default()
        }
    }
}

/// Generate text using provided (cloned) state. Used by both `complete` and `stream`.
async fn generate_with_state(
    model: Arc<tokio::sync::Mutex<Qwen2Model>>,
    tokenizer: tokenizers::Tokenizer,
    device: Device,
    config: LocalGgufConfig,
    prompt: &str,
    max_tokens: usize,
    tx: Option<tokio::sync::mpsc::Sender<Result<StreamDelta, AgentError>>>,
) -> Result<String, AgentError> {
    let mut model = model.lock().await;

    let tokens = tokenizer
        .encode(prompt.to_string(), true)
        .map_err(|e| AgentError::Llm(format!("Tokenizer encode error: {}", e)))?;
    let prompt_tokens = tokens.get_ids().to_vec();
    let prompt_len = prompt_tokens.len();

    if prompt_tokens.is_empty() {
        return Err(AgentError::Llm("Empty prompt after tokenization".into()));
    }

    let sampling = if config.temperature <= 0.0 {
        Sampling::ArgMax
    } else {
        match (config.top_k, config.top_p) {
            (None, None) => Sampling::All {
                temperature: config.temperature,
            },
            (Some(k), None) => Sampling::TopK {
                k,
                temperature: config.temperature,
            },
            (None, Some(p)) => Sampling::TopP {
                p,
                temperature: config.temperature,
            },
            (Some(k), Some(p)) => Sampling::TopKThenTopP {
                k,
                p,
                temperature: config.temperature,
            },
        }
    };
    let mut logits_processor = LogitsProcessor::from_sampling(config.seed, sampling);

    let input = Tensor::new(prompt_tokens.as_slice(), &device)
        .map_err(|e| AgentError::Llm(format!("Tensor creation error: {}", e)))?
        .unsqueeze(0)
        .map_err(|e| AgentError::Llm(format!("Tensor unsqueeze error: {}", e)))?;
    let logits = model
        .forward(&input, 0)
        .map_err(|e| AgentError::Llm(format!("Model forward error: {}", e)))?;
    let logits = logits
        .squeeze(0)
        .map_err(|e| AgentError::Llm(format!("Tensor squeeze error: {}", e)))?;
    let mut next_token = logits_processor
        .sample(&logits)
        .map_err(|e| AgentError::Llm(format!("Sampling error: {}", e)))?;

    let eos_token_id = tokenizer
        .get_vocab(true)
        .get(config.chat_template.eos_token())
        .copied();

    let mut all_tokens = vec![next_token];
    let mut token_stream = TokenOutputStream::new(tokenizer);
    let mut generated_text = String::new();

    if let Some(text) = token_stream.next_token(next_token)? {
        if let Some(ref sender) = tx {
            let _ = sender
                .send(Ok(StreamDelta {
                    content: Some(text.clone()),
                    tool_calls: vec![],
                }))
                .await;
        }
        generated_text.push_str(&text);
    }

    for index in 0..max_tokens {
        let input = Tensor::new(&[next_token], &device)
            .map_err(|e| AgentError::Llm(format!("Tensor creation error: {}", e)))?
            .unsqueeze(0)
            .map_err(|e| AgentError::Llm(format!("Tensor unsqueeze error: {}", e)))?;
        let logits = model
            .forward(&input, prompt_len + index)
            .map_err(|e| AgentError::Llm(format!("Model forward error: {}", e)))?;
        let logits = logits
            .squeeze(0)
            .map_err(|e| AgentError::Llm(format!("Tensor squeeze error: {}", e)))?;

        let logits = if config.repeat_penalty == 1.0 {
            logits
        } else {
            let start_at = all_tokens.len().saturating_sub(config.repeat_last_n);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                config.repeat_penalty,
                &all_tokens[start_at..],
            )
            .map_err(|e| AgentError::Llm(format!("Repeat penalty error: {}", e)))?
        };

        next_token = logits_processor
            .sample(&logits)
            .map_err(|e| AgentError::Llm(format!("Sampling error: {}", e)))?;
        all_tokens.push(next_token);

        if let Some(text) = token_stream.next_token(next_token)? {
            if let Some(ref sender) = tx {
                let _ = sender
                    .send(Ok(StreamDelta {
                        content: Some(text.clone()),
                        tool_calls: vec![],
                    }))
                    .await;
            }
            generated_text.push_str(&text);
        }

        if Some(next_token) == eos_token_id {
            break;
        }
    }

    if let Some(rest) = token_stream.decode_rest()? {
        if let Some(ref sender) = tx {
            let _ = sender
                .send(Ok(StreamDelta {
                    content: Some(rest.clone()),
                    tool_calls: vec![],
                }))
                .await;
        }
        generated_text.push_str(&rest);
    }

    Ok(generated_text)
}

/// Pick the best available device (CUDA > CPU).
fn pick_device() -> candle_core::Result<Device> {
    #[cfg(feature = "local-llm-cuda")]
    {
        if candle_core::utils::cuda_is_available() {
            tracing::info!("CUDA is available, using GPU");
            return Device::new_cuda(0);
        }
    }
    tracing::info!("CUDA not available, using CPU");
    Ok(Device::Cpu)
}

/// Load a GGUF model from disk.
fn load_model(path: &Path, device: &Device) -> Result<Qwen2Model, AgentError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| AgentError::Llm(format!("Failed to open model file: {}", e)))?;

    let start = std::time::Instant::now();
    let model_content = gguf_file::Content::read(&mut file)
        .map_err(|e| AgentError::Llm(format!("Failed to read GGUF file: {}", e)))?;

    let mut total_size = 0usize;
    for (_, tensor) in model_content.tensor_infos.iter() {
        let elem_count = tensor.shape.elem_count();
        total_size += elem_count * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
    }

    tracing::info!(
        "Loaded {} tensors ({:.2} MB) in {:.2}s",
        model_content.tensor_infos.len(),
        total_size as f64 / 1e6,
        start.elapsed().as_secs_f32()
    );

    let model = Qwen2Model::from_gguf(model_content, &mut file, device)
        .map_err(|e| AgentError::Llm(format!("Failed to load model weights: {}", e)))?;

    tracing::info!("Model built successfully");
    Ok(model)
}

/// Load tokenizer from local path or download from HuggingFace.
async fn load_tokenizer(config: &LocalGgufConfig) -> Result<tokenizers::Tokenizer, AgentError> {
    if let Some(ref path) = config.tokenizer_path {
        if path.exists() {
            return tokenizers::Tokenizer::from_file(path)
                .map_err(|e| AgentError::Llm(format!("Failed to load tokenizer: {}", e)));
        }
    }

    if let Some(ref repo) = config.tokenizer_repo {
        let repo = repo.clone();
        let tokenizer_path = tokio::task::spawn_blocking(move || {
            let api = hf_hub::api::sync::Api::new()?;
            let api = api.model(repo);
            api.get("tokenizer.json")
        })
        .await
        .map_err(|e| AgentError::Llm(format!("Tokenizer download task panicked: {}", e)))?
        .map_err(|e| AgentError::Llm(format!("Failed to download tokenizer: {}", e)))?;

        return tokenizers::Tokenizer::from_file(tokenizer_path)
            .map_err(|e| AgentError::Llm(format!("Failed to load downloaded tokenizer: {}", e)));
    }

    // Fallback: look for tokenizer.json next to the model file
    let sibling = config.model_path.with_file_name("tokenizer.json");
    if sibling.exists() {
        return tokenizers::Tokenizer::from_file(&sibling)
            .map_err(|e| AgentError::Llm(format!("Failed to load tokenizer: {}", e)));
    }

    Err(AgentError::Llm(
        "No tokenizer found. Please set `tokenizer_path` or `tokenizer_repo` in the config, \
         or place a tokenizer.json next to the model file."
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_template_detect() {
        assert_eq!(
            ChatTemplate::detect("DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf"),
            ChatTemplate::DeepSeekR1
        );
        assert_eq!(
            ChatTemplate::detect("Qwen2.5-7B-Instruct.Q4_K_M.gguf"),
            ChatTemplate::Qwen2
        );
        assert_eq!(
            ChatTemplate::detect("something-else.gguf"),
            ChatTemplate::Qwen2
        );
    }

    #[test]
    fn test_qwen2_format() {
        let messages = vec![Message::system("You are helpful."), Message::user("Hello")];
        let prompt = ChatTemplate::Qwen2.format(&messages, &serde_json::json!([]));
        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("You are helpful."));
        assert!(prompt.contains("<|im_start|>user"));
        assert!(prompt.contains("Hello"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_deepseek_r1_format() {
        let messages = vec![Message::system("You are helpful."), Message::user("Hello")];
        let prompt = ChatTemplate::DeepSeekR1.format(&messages, &serde_json::json!([]));
        assert!(prompt.contains("You are helpful."));
        assert!(prompt.contains("<｜User｜>Hello"));
        assert!(prompt.ends_with("<｜Assistant｜>"));
    }

    #[test]
    fn test_parse_tool_calls() {
        let text = r#"I'll read the file for you.
{"tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": {"path": "/test.txt"}}}]}
"#;
        let calls = LocalGgufProvider::parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "read_file");
    }

    /// End-to-end benchmark: load a real GGUF model, generate tokens, and measure latency.
    /// Requires a GGUF model at the path resolved by `resolve_local_model_path()`.
    #[tokio::test]
    #[ignore = "Requires local GGUF model"]
    async fn test_local_gguf_loads_real_model() {
        let model_path = crate::llm::resolve_local_model_path().unwrap_or_else(|| {
            PathBuf::from(r"C:\Users\22414\Desktop\model\Qwen2.5-7B-Instruct.Q4_K_M.gguf")
        });
        if !model_path.exists() {
            eprintln!(
                "Skipping e2e test: model not found at {}",
                model_path.display()
            );
            return;
        }

        // Try tokenizer.json next to model, else hf-hub fallback
        let tokenizer_path = model_path.with_file_name("tokenizer.json");
        let mut config = LocalGgufConfig::new(&model_path)
            .with_max_tokens(30)
            .with_temperature(0.7);
        if tokenizer_path.exists() {
            config = config.with_tokenizer_path(&tokenizer_path);
        }

        // Measure load time
        let load_start = std::time::Instant::now();
        let provider = LocalGgufProvider::new(config)
            .await
            .expect("failed to load model");
        let load_ms = load_start.elapsed().as_millis();
        println!("[BENCH] Model loaded in {} ms", load_ms);

        // Measure generation time
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("What is 2+2? Answer with a single number."),
        ];
        let gen_start = std::time::Instant::now();
        let response = provider
            .complete(&messages, &serde_json::json!([]))
            .await
            .expect("generation failed");
        let gen_ms = gen_start.elapsed().as_millis() as f64;

        // Estimate token count from response length (rough heuristic: 1 token ≈ 4 chars for English)
        let output_len = response.content.len();
        let estimated_tokens = (output_len / 4).max(1);
        let ms_per_token = gen_ms / estimated_tokens as f64;

        println!(
            "[BENCH] Generated {} chars (~{} tokens) in {} ms",
            output_len, estimated_tokens, gen_ms as u64
        );
        println!("[BENCH] Latency: {:.1} ms/token", ms_per_token);
        println!("[BENCH] Output: {:?}", response.content);

        // Assert generation produced something reasonable
        assert!(!response.content.is_empty(), "Generated text was empty");
        // NOTE: CPU mode typically yields 3000-6000 ms/token for 7B models.
        // The real acceptance criteria is CUDA mode: target < 200 ms/token.
        println!("[BENCH] Device: {:?}", provider.device);
        println!(
            "[BENCH] {} mode verdict: {} ms/token — {}",
            if format!("{:?}", provider.device).contains("Cuda") {
                "CUDA"
            } else {
                "CPU"
            },
            ms_per_token,
            if ms_per_token < 300.0 {
                "EXCELLENT"
            } else if ms_per_token < 800.0 {
                "ACCEPTABLE"
            } else {
                "TOO SLOW — consider CUDA or smaller model"
            }
        );
    }
}
