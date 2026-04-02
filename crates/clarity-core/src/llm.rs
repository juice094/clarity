//! LLM Providers - 大语言模型接入层
//!
//! 支持多种 LLM 服务：
//! - Kimi Code (api.kimi.com/coding) - Anthropic 协议，每周 1024 次免费
//! - Kimi API (api.moonshot.cn) - OpenAI 协议，按量付费
//! - OpenAI 兼容 API - 通用接口
//! - 本地模型 (Ollama) - 离线运行
//!
//! ## Claude Code 配置兼容
//!
//! ```bash
//! # Kimi Code (与 Claude Code 相同配置)
//! export ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
//! export ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use tracing::{debug, error, info};

use crate::agent::{LlmProvider, LlmResponse, Message, ToolCall};
use crate::error::AgentError;

/// 消息格式转换
fn convert_messages(messages: &[Message]) -> Vec<Value> {
    messages.iter().map(|m| {
        json!({
            "role": match m.role {
                crate::agent::MessageRole::System => "system",
                crate::agent::MessageRole::User => "user",
                crate::agent::MessageRole::Assistant => "assistant",
                crate::agent::MessageRole::Tool => "tool",
            },
            "content": m.content.clone(),
        })
    }).collect()
}

/// Kimi LLM Provider
#[derive(Debug, Clone)]
pub struct KimiLlm {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    protocol: Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Protocol {
    /// Anthropic 协议（Kimi Code、Claude 兼容）
    Anthropic,
    /// OpenAI 协议（Kimi API、标准 API）
    OpenAI,
}

impl KimiLlm {
    /// 从环境变量创建（兼容 Claude Code 配置）
    pub fn from_env() -> Result<Self, AgentError> {
        // 优先级：ANTHROPIC_* (Claude Code 风格) > KIMI_* (Clarity 风格)
        let (api_key, key_source) = if let Ok(key) = env::var("ANTHROPIC_AUTH_TOKEN") {
            (key, "ANTHROPIC_AUTH_TOKEN")
        } else if let Ok(key) = env::var("KIMI_API_KEY") {
            (key, "KIMI_API_KEY")
        } else {
            return Err(AgentError::Llm(
                "未设置 API Key。请设置 ANTHROPIC_AUTH_TOKEN 或 KIMI_API_KEY".into()
            ));
        };
        
        let base_url = env::var("ANTHROPIC_BASE_URL")
            .or_else(|_| env::var("KIMI_BASE_URL"))
            .unwrap_or_else(|_| "https://api.moonshot.cn/v1".into());
        
        // 检测协议
        let protocol = if base_url.contains("kimi.com/coding") 
            || base_url.contains("anthropic")
            || env::var("USE_ANTHROPIC_PROTOCOL").is_ok() {
            Protocol::Anthropic
        } else {
            Protocol::OpenAI
        };
        
        // 规范化 base_url
        let base_url = if protocol == Protocol::Anthropic && !base_url.ends_with('/') {
            format!("{}/", base_url)
        } else {
            base_url
        };
        
        let model = env::var("ANTHROPIC_MODEL")
            .or_else(|_| env::var("KIMI_MODEL"))
            .unwrap_or_else(|_| {
                if protocol == Protocol::Anthropic {
                    "kimi-for-coding".into()
                } else {
                    "moonshot-v1-8k".into()
                }
            });
        
        info!("使用 {} 协议，base_url: {}", 
            if protocol == Protocol::Anthropic { "Anthropic" } else { "OpenAI" },
            base_url
        );
        
        Ok(Self {
            client: Client::new(),
            base_url,
            api_key,
            model,
            protocol,
        })
    }
    
    /// 直接创建
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let base_url = base_url.into();
        let protocol = if base_url.contains("kimi.com/coding") || base_url.contains("anthropic") {
            Protocol::Anthropic
        } else {
            Protocol::OpenAI
        };
        
        Self {
            client: Client::new(),
            base_url,
            api_key: api_key.into(),
            model: model.into(),
            protocol,
        }
    }
    
    /// 构建请求 URL
    fn build_url(&self) -> String {
        match self.protocol {
            Protocol::Anthropic => format!("{}v1/messages", self.base_url),
            Protocol::OpenAI => format!("{}chat/completions", self.base_url),
        }
    }
    
    /// 构建请求体
    fn build_request_body(&self, messages: &[Message], _tools: &Value) -> Value {
        let msgs = convert_messages(messages);
        
        match self.protocol {
            Protocol::Anthropic => {
                // Anthropic 协议（Kimi Code）
                json!({
                    "model": self.model,
                    "messages": msgs,
                    "max_tokens": 4096,
                    "temperature": 0.7,
                })
            }
            Protocol::OpenAI => {
                // OpenAI 协议（Kimi API）
                json!({
                    "model": self.model,
                    "messages": msgs,
                    "temperature": 0.7,
                })
            }
        }
    }
    
    /// 解析响应
    fn parse_response(&self, json: Value) -> Result<LlmResponse, AgentError> {
        match self.protocol {
            Protocol::Anthropic => self.parse_anthropic_response(json),
            Protocol::OpenAI => self.parse_openai_response(json),
        }
    }
    
    /// 解析 Anthropic 格式响应
    fn parse_anthropic_response(&self, json: Value) -> Result<LlmResponse, AgentError> {
        // Anthropic 格式: { "content": [{"type": "text", "text": "..."}] }
        let content = if let Some(arr) = json["content"].as_array() {
            arr.iter()
                .filter(|c| c["type"] == "text")
                .map(|c| c["text"].as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("")
        } else {
            json["content"].as_str().unwrap_or("").to_string()
        };
        
        // 提取 tool_calls（如果在 content 中）
        let mut tool_calls = Vec::new();
        if let Some(arr) = json["content"].as_array() {
            for item in arr {
                if item["type"] == "tool_use" {
                    if let Ok(tc) = serde_json::from_value::<ToolCall>(item.clone()) {
                        tool_calls.push(tc);
                    }
                }
            }
        }
        
        let is_complete = tool_calls.is_empty();
        Ok(LlmResponse {
            content,
            tool_calls,
            is_complete,
        })
    }
    
    /// 解析 OpenAI 格式响应
    fn parse_openai_response(&self, json: Value) -> Result<LlmResponse, AgentError> {
        let choice = json["choices"]
            .get(0)
            .ok_or_else(|| AgentError::InvalidResponse("No choices".into()))?;
        
        let message = &choice["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();
        
        let mut tool_calls = Vec::new();
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                if let Ok(tc) = serde_json::from_value::<ToolCall>(call.clone()) {
                    tool_calls.push(tc);
                }
            }
        }
        
        let is_complete = tool_calls.is_empty();
        Ok(LlmResponse {
            content,
            tool_calls,
            is_complete,
        })
    }
}

#[async_trait]
impl LlmProvider for KimiLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let url = self.build_url();
        let body = self.build_request_body(messages, tools);
        
        debug!("Request to {}: {}", url, serde_json::to_string(&body).unwrap_or_default());
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("User-Agent", "Clarity/0.1.0")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("Request failed: {}", e)))?;
        
        let status = response.status();
        let text = response.text().await.map_err(|e| {
            AgentError::Llm(format!("Failed to read response: {}", e))
        })?;
        
        if !status.is_success() {
            error!("API error ({}): {}", status, text);
            return Err(AgentError::Llm(format!("API error {}: {}", status, text)));
        }
        
        let json: Value = serde_json::from_str(&text).map_err(|e| {
            AgentError::Llm(format!("Failed to parse JSON: {}", e))
        })?;
        
        self.parse_response(json)
    }
}

/// OpenAI 兼容 Provider
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleLlm {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiCompatibleLlm {
    /// 从环境变量创建
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| AgentError::Llm("OPENAI_API_KEY not set".into()))?;
        
        let base_url = env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        
        let model = env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-3.5-turbo".into());
        
        Ok(Self::new(api_key, base_url, model))
    }
    
    /// 直接创建
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
    
    /// Ollama 快捷配置
    pub fn ollama(model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:11434/v1".into(),
            api_key: "ollama".into(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleLlm {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let body = json!({
            "model": self.model,
            "messages": convert_messages(messages),
            "tools": tools,
            "temperature": 0.7,
        });
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("Request failed: {}", e)))?;
        
        let status = response.status();
        let text = response.text().await.map_err(|e| {
            AgentError::Llm(format!("Failed to read response: {}", e))
        })?;
        
        if !status.is_success() {
            return Err(AgentError::Llm(format!("API error {}: {}", status, text)));
        }
        
        let json: Value = serde_json::from_str(&text).map_err(|e| {
            AgentError::Llm(format!("Failed to parse JSON: {}", e))
        })?;
        
        // OpenAI 格式解析
        let choice = json["choices"]
            .get(0)
            .ok_or_else(|| AgentError::InvalidResponse("No choices".into()))?;
        
        let message = &choice["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();
        
        let mut tool_calls = Vec::new();
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                if let Ok(tc) = serde_json::from_value::<ToolCall>(call.clone()) {
                    tool_calls.push(tc);
                }
            }
        }
        
        let is_complete = tool_calls.is_empty();
        Ok(LlmResponse {
            content,
            tool_calls,
            is_complete,
        })
    }
}

/// LLM Provider 工厂
pub struct LlmFactory;

impl LlmFactory {
    /// 自动检测并创建 Provider
    pub fn auto() -> Result<Box<dyn LlmProvider>, AgentError> {
        // 优先检测 Kimi Code（Claude Code 配置风格）
        if env::var("ANTHROPIC_BASE_URL").is_ok() 
            || env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
            info!("使用 KimiLlm (ANTHROPIC_* 配置)");
            return Ok(Box::new(KimiLlm::from_env()?));
        }
        
        if env::var("KIMI_API_KEY").is_ok() {
            info!("使用 KimiLlm (KIMI_* 配置)");
            return Ok(Box::new(KimiLlm::from_env()?));
        }
        
        if env::var("OPENAI_API_KEY").is_ok() {
            info!("使用 OpenAiCompatibleLlm");
            return Ok(Box::new(OpenAiCompatibleLlm::from_env()?));
        }
        
        Err(AgentError::Llm(
            "未配置 LLM。请设置以下环境变量之一：\n".to_string() +
            "  - ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN (Kimi Code)\n" +
            "  - KIMI_API_KEY (Kimi API)\n" +
            "  - OPENAI_API_KEY (OpenAI 兼容)"
        ))
    }
}
