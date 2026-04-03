//! Channels 模块 - 多平台接入点
//!
//! 提供统一的渠道接口，支持：
//! - Telegram Bot
//! - Discord Bot
//! - 通用 Webhook（适用于飞书/钉钉等）

pub mod telegram;
pub mod discord;
pub mod webhook;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use clarity_core::Agent;

/// 渠道统一接口 - 所有平台接入点需实现此 trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// 渠道名称
    fn name(&self) -> &str;

    /// 启动渠道，接收 Agent 用于处理消息
    async fn start(&self, agent: Arc<Agent>) -> Result<(), ChannelError>;

    /// 停止渠道
    #[allow(dead_code)]
    async fn stop(&self) -> Result<(), ChannelError>;
}

/// Channel 错误类型
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Channel not started")]
    #[allow(dead_code)]
    NotStarted,

    #[error("Configuration error: {0}")]
    #[allow(dead_code)]
    ConfigError(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for ChannelError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ChannelError::Unknown(e.to_string())
    }
}

/// 渠道状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum ChannelStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// 渠道消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ChannelMessage {
    pub user_id: String,
    pub username: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub metadata: Option<serde_json::Value>,
}

#[allow(dead_code)]
impl ChannelMessage {
    pub fn new(user_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            username: None,
            content: content.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: None,
        }
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// 渠道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ChannelConfig {
    pub enabled: bool,
    pub token: Option<String>,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub extra: Option<serde_json::Value>,
}

impl ChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn with_webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    pub fn with_webhook_secret(mut self, secret: impl Into<String>) -> Self {
        self.webhook_secret = Some(secret.into());
        self
    }
}

/// 流式响应回调
#[allow(dead_code)]
pub type StreamCallback = Box<dyn FnMut(&str) + Send + Sync>;

/// 消息响应 trait - 用于发送响应给用户
#[allow(dead_code)]
#[async_trait]
pub trait MessageResponder: Send + Sync {
    /// 发送文本响应
    async fn send_text(&self, text: &str) -> Result<(), ChannelError>;

    /// 发送流式响应（分批发送长消息）
    async fn send_stream<F>(&self, stream_fn: F) -> Result<(), ChannelError>
    where
        F: FnOnce(StreamCallback) -> Result<(), ChannelError> + Send;
}

/// 渠道管理器
pub struct ChannelManager {
    channels: Vec<Box<dyn Channel>>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    pub fn register(&mut self, channel: Box<dyn Channel>) {
        tracing::info!("Registered channel: {}", channel.name());
        self.channels.push(channel);
    }

    pub async fn start_all(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        for channel in &self.channels {
            tracing::info!("Starting channel: {}", channel.name());
            channel.start(agent.clone()).await?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn stop_all(&self) -> Result<(), ChannelError> {
        for channel in &self.channels {
            tracing::info!("Stopping channel: {}", channel.name());
            channel.stop().await?;
        }
        Ok(())
    }

    pub fn get_channel_names(&self) -> Vec<&str> {
        self.channels.iter().map(|c| c.name()).collect()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_message_creation() {
        let msg = ChannelMessage::new("user123", "Hello");
        assert_eq!(msg.user_id, "user123");
        assert_eq!(msg.content, "Hello");
        assert!(msg.username.is_none());
    }

    #[test]
    fn test_channel_message_with_username() {
        let msg = ChannelMessage::new("user123", "Hello")
            .with_username("Alice");
        assert_eq!(msg.username, Some("Alice".to_string()));
    }

    #[test]
    fn test_channel_config() {
        let config = ChannelConfig::new()
            .enabled()
            .with_token("test_token")
            .with_webhook_url("https://example.com/webhook");

        assert!(config.enabled);
        assert_eq!(config.token, Some("test_token".to_string()));
        assert_eq!(config.webhook_url, Some("https://example.com/webhook".to_string()));
    }
}
