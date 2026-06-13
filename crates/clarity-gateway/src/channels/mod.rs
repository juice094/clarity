//! Channels 模块 - 多平台接入点
//!
//! 提供统一的渠道接口，支持：
//! - Telegram Bot
//! - Discord Bot
//! - 通用 Webhook（适用于飞书/钉钉等）

pub mod discord;
pub mod slack;
pub mod telegram;
pub mod webhook;
pub mod wechat;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use clarity_channels::retry::RetryableError;
use clarity_core::Agent;

/// 渠道统一接口 - 所有平台接入点需实现此 trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// 渠道名称
    fn name(&self) -> &str;

    /// 启动渠道，接收 Agent 用于处理消息
    async fn start(&self, agent: Arc<Agent>) -> Result<(), ChannelError>;

    /// 停止渠道
    // Intentionally retained: lifecycle method required by the Channel contract.
    #[allow(dead_code)]
    async fn stop(&self) -> Result<(), ChannelError>;
}

/// Channel 错误类型
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    /// Connection to the channel platform failed.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Authentication with the channel platform failed.
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Sending a message through the channel failed.
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// The channel has not been started.
    #[error("Channel not started")]
    // Intentionally retained: part of the public ChannelError API.
    #[allow(dead_code)]
    NotStarted,

    /// Channel configuration is invalid.
    #[error("Configuration error: {0}")]
    // Intentionally retained: part of the public ChannelError API.
    #[allow(dead_code)]
    ConfigError(String),

    /// An HTTP request to the channel platform failed.
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// An unknown channel error occurred.
    #[error("Unknown error: {0}")]
    Unknown(String),

    /// Multiple channel errors aggregated during a batched operation.
    #[error("Multiple channel failures: {0:?}")]
    Multiple(Vec<ChannelError>),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for ChannelError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ChannelError::Unknown(e.to_string())
    }
}

impl RetryableError for ChannelError {
    fn is_retryable(&self) -> bool {
        match self {
            ChannelError::HttpError(e) => {
                e.is_timeout()
                    || e.is_connect()
                    || matches!(e.status(), Some(s) if s.is_server_error() || s.as_u16() == 429)
            }
            ChannelError::ConnectionFailed(_) | ChannelError::SendFailed(_) => true,
            _ => false,
        }
    }
}

/// 渠道状态
// Intentionally retained: public status enum for channel implementations.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelStatus {
    /// Channel is stopped.
    Stopped,
    /// Channel is starting.
    Starting,
    /// Channel is running.
    Running,
    /// Channel encountered an error.
    Error(String),
}

/// 渠道消息结构
// Intentionally retained: public message type for channel implementations.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    /// User identifier on the channel platform.
    pub user_id: String,
    /// Optional human-readable username.
    pub username: Option<String>,
    /// Message text.
    pub content: String,
    /// Message timestamp in RFC 3339 format.
    pub timestamp: String,
    /// Arbitrary channel-specific metadata.
    pub metadata: Option<serde_json::Value>,
}

// Intentionally retained: builder API for public ChannelMessage type.
#[allow(dead_code)]
impl ChannelMessage {
    /// Create a new channel message.
    pub fn new(user_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            username: None,
            content: content.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: None,
        }
    }

    /// Set the username.
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Attach metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// 渠道配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelConfig {
    /// Whether the channel is enabled at startup.
    pub enabled: bool,
    /// Bot or API token used by the channel.
    pub token: Option<String>,
    /// Webhook URL for outbound push channels.
    pub webhook_url: Option<String>,
    /// Shared secret for signing or verifying webhook payloads.
    pub webhook_secret: Option<String>,
    /// Arbitrary extra configuration supplied by the user.
    pub extra: Option<serde_json::Value>,
}

impl ChannelConfig {
    /// Create a new disabled channel configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable the channel.
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Set the API or bot token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set the outbound webhook URL.
    pub fn with_webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    /// Set the webhook signing secret.
    pub fn with_webhook_secret(mut self, secret: impl Into<String>) -> Self {
        self.webhook_secret = Some(secret.into());
        self
    }

    /// Attach arbitrary extra configuration.
    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.extra = Some(extra);
        self
    }
}

/// 流式响应回调
// Intentionally retained: public type alias for streaming responder implementations.
#[allow(dead_code)]
pub type StreamCallback = Box<dyn FnMut(&str) + Send + Sync>;

/// 消息响应 trait - 用于发送响应给用户
// Intentionally retained: public responder trait for future channel integrations.
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
    /// Create an empty channel manager.
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    /// Register a channel implementation.
    pub fn register(&mut self, channel: Box<dyn Channel>) {
        tracing::info!("Registered channel: {}", channel.name());
        self.channels.push(channel);
    }

    /// Start all registered channels.
    ///
    /// Errors from individual channels are collected and returned as a single
    /// [`ChannelError::Multiple`] rather than failing fast on the first error.
    pub async fn start_all(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        let mut errors = Vec::new();
        for channel in &self.channels {
            tracing::info!("Starting channel: {}", channel.name());
            if let Err(err) = channel.start(agent.clone()).await {
                tracing::error!("Failed to start channel {}: {}", channel.name(), err);
                errors.push(err);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ChannelError::Multiple(errors))
        }
    }

    /// Stop all registered channels.
    // Intentionally retained: lifecycle method for graceful channel shutdown.
    #[allow(dead_code)]
    pub async fn stop_all(&self) -> Result<(), ChannelError> {
        for channel in &self.channels {
            tracing::info!("Stopping channel: {}", channel.name());
            channel.stop().await?;
        }
        Ok(())
    }

    /// Return the names of all registered channels.
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
        let msg = ChannelMessage::new("user123", "Hello").with_username("Alice");
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
        assert_eq!(
            config.webhook_url,
            Some("https://example.com/webhook".to_string())
        );
    }
}
