//! Channels 模块 - 多平台接入点
//!
//! 预留 Telegram、Discord 等平台的 Bot 接口

pub mod telegram;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Channel 接口 - 所有平台接入点需实现此 trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// Channel 名称
    fn name(&self) -> &str;

    /// 启动 Channel
    async fn start(&self) -> Result<(), ChannelError>;

    /// 停止 Channel
    async fn stop(&self) -> Result<(), ChannelError>;

    /// 发送消息到用户
    async fn send_message(&self, user_id: &str, message: &str) -> Result<(), ChannelError>;

    /// 获取 Channel 状态
    fn status(&self) -> ChannelStatus;
}

/// Channel 错误
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Channel not started")]
    NotStarted,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Channel 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// 消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub user_id: String,
    pub username: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub metadata: Option<serde_json::Value>,
}

/// Channel 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub enabled: bool,
    pub token: Option<String>,
    pub webhook_url: Option<String>,
    pub extra: Option<serde_json::Value>,
}

/// Channel 管理器
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
        self.channels.push(channel);
    }

    pub async fn start_all(&self) -> Result<(), ChannelError> {
        for channel in &self.channels {
            channel.start().await?;
        }
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<(), ChannelError> {
        for channel in &self.channels {
            channel.stop().await?;
        }
        Ok(())
    }

    pub fn get_status(&self) -> Vec<(&str, ChannelStatus)> {
        self.channels
            .iter()
            .map(|c| (c.name(), c.status()))
            .collect()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}
