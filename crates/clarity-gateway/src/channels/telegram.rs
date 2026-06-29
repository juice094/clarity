//! Telegram Bot 渠道实现
//!
//! 使用 teloxide crate 实现 Telegram Bot：
//! - 接收用户消息
//! - 发送响应（支持流式响应）
//! - 支持 Long polling 模式

// Intentionally retained: public types and helpers are kept for Telegram integration and tests.
#![allow(dead_code)]

use async_trait::async_trait;
use clarity_channels::retry::RetryPolicy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

#[cfg(feature = "telegram")]
use teloxide::prelude::*;
#[cfg(feature = "telegram")]
use teloxide::types::ParseMode;

use clarity_core::Agent;

use super::{Channel, ChannelConfig, ChannelError};

/// Telegram Bot 渠道
pub struct TelegramChannel {
    config: ChannelConfig,
    bot_token: Option<String>,
}

impl TelegramChannel {
    /// Create a new Telegram channel from the given configuration.
    pub fn new(config: ChannelConfig) -> Self {
        Self {
            bot_token: config.token.clone(),
            config,
        }
    }

    /// 使用 teloxide 运行 bot（feature enabled）
    #[cfg(feature = "telegram")]
    async fn run_with_teloxide(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        let token = self.bot_token.as_ref().ok_or_else(|| {
            ChannelError::AuthFailed("Telegram bot token not configured".to_string())
        })?;

        info!("Starting Telegram bot with teloxide...");

        let bot = Bot::new(token);

        teloxide::repl(bot, move |bot: Bot, msg: Message| {
            let agent = agent.clone();
            async move {
                if let Some(text) = msg.text() {
                    info!("[Telegram] Received message from {}: {}", msg.chat.id, text);

                    // 处理消息并获取响应
                    match process_message(agent, text).await {
                        Ok(response) => {
                            // 发送响应，支持长消息分批
                            let chunks = split_message(&response, 4096);
                            for chunk in chunks {
                                if let Err(e) = bot
                                    .send_message(msg.chat.id, chunk)
                                    .parse_mode(ParseMode::MarkdownV2)
                                    .await
                                {
                                    error!("[Telegram] Failed to send message: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            error!("[Telegram] Failed to process message: {}", e);
                            let _ = bot
                                .send_message(
                                    msg.chat.id,
                                    "Sorry, I encountered an error processing your message.",
                                )
                                .await;
                        }
                    }
                }
                respond(())
            }
        })
        .await;

        Ok(())
    }

    /// Mock 实现（feature disabled）
    #[cfg(not(feature = "telegram"))]
    async fn run_with_teloxide(&self, _agent: Arc<Agent>) -> Result<(), ChannelError> {
        warn!("[Telegram] teloxide feature is disabled, using mock mode");

        // Mock mode: 记录配置但不实际启动
        info!(
            "[Telegram] Mock mode - would start bot with token: {:?}",
            self.bot_token.as_ref().map(|_| "***REDACTED***")
        );

        // 保持运行直到收到停止信号
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        info!("[Telegram] Mock bot finished");

        Ok(())
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        if !self.config.enabled {
            warn!("[Telegram] Channel is disabled");
            return Ok(());
        }

        info!("[Telegram] Starting channel...");
        self.run_with_teloxide(agent).await
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("[Telegram] Stopping channel...");
        // teloxide 的 repl 会在程序退出时自动停止
        Ok(())
    }
}

/// 处理用户消息
async fn process_message(agent: Arc<Agent>, text: &str) -> Result<String, ChannelError> {
    match agent.run(text).await {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("[Telegram] Agent error: {}", e);
            Err(ChannelError::Unknown(e.to_string()))
        }
    }
}

/// 将长消息分割成多个块
fn split_message(text: &str, max_length: usize) -> Vec<&str> {
    if text.len() <= max_length {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + max_length).min(text.len());
        // 尝试在换行处分割
        let split_point = if end < text.len() {
            text[start..end]
                .rfind('\n')
                .map(|i| start + i + 1)
                .unwrap_or(end)
        } else {
            end
        };

        chunks.push(&text[start..split_point]);
        start = split_point;
    }

    chunks
}

// ==================== Telegram API 类型 ====================

/// Telegram Update（Webhook 推送的消息）
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramUpdate {
    /// Unique update identifier.
    pub update_id: i64,
    /// Incoming message, if any.
    pub message: Option<TelegramMessage>,
}

/// Telegram Message
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramMessage {
    /// Message identifier.
    pub message_id: i64,
    /// Sending user.
    pub from: TelegramUser,
    /// Chat where the message was posted.
    pub chat: TelegramChat,
    /// Unix timestamp of the message.
    pub date: i64,
    /// Message text.
    pub text: Option<String>,
}

/// Telegram User
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramUser {
    /// User identifier.
    pub id: i64,
    /// Whether the user is a bot.
    pub is_bot: bool,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: Option<String>,
    /// Username handle.
    pub username: Option<String>,
}

/// Telegram Chat
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramChat {
    /// Chat identifier.
    pub id: i64,
    /// Chat type (e.g. "private").
    #[serde(rename = "type")]
    pub chat_type: String,
}

/// 发送消息请求
#[derive(Debug, Serialize)]
pub struct SendMessageRequest {
    /// Target chat identifier.
    pub chat_id: i64,
    /// Message text.
    pub text: String,
    /// Optional parse mode identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<String>,
}

/// HTTP 实现的 Telegram API 客户端
pub struct TelegramApiClient {
    client: reqwest::Client,
    token: String,
    base_url: String,
    retry_policy: RetryPolicy,
}

impl TelegramApiClient {
    /// Create a new Telegram API client for the given bot token.
    pub fn new(token: impl Into<String>) -> Self {
        let token = token.into();
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: format!("https://api.telegram.org/bot{}", token),
            token,
            retry_policy: RetryPolicy::new(),
        }
    }

    /// Set a custom retry policy for Telegram Bot API calls.
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// 发送消息
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), ChannelError> {
        let text = text.to_string();
        self.retry_policy
            .execute(move || {
                let client = self.client.clone();
                let base_url = self.base_url.clone();
                let text = text.clone();
                async move {
                    let url = format!("{}/sendMessage", base_url);

                    let body = SendMessageRequest {
                        chat_id,
                        text: text.clone(),
                        parse_mode: Some("Markdown".to_string()),
                    };

                    let response = client.post(&url).json(&body).send().await?;

                    if !response.status().is_success() {
                        let error_text = response.text().await.unwrap_or_default();
                        return Err(ChannelError::SendFailed(format!(
                            "Telegram API error: {}",
                            error_text
                        )));
                    }

                    Ok(())
                }
            })
            .await
    }

    /// 设置 Webhook
    pub async fn set_webhook(&self, webhook_url: &str) -> Result<(), ChannelError> {
        let webhook_url = webhook_url.to_string();
        let log_url = webhook_url.clone();
        self.retry_policy
            .execute(move || {
                let client = self.client.clone();
                let base_url = self.base_url.clone();
                let webhook_url = webhook_url.clone();
                async move {
                    let url = format!("{}/setWebhook", base_url);

                    let body = serde_json::json!({
                        "url": webhook_url,
                    });

                    let response = client.post(&url).json(&body).send().await?;

                    if !response.status().is_success() {
                        let error_text = response.text().await.unwrap_or_default();
                        return Err(ChannelError::Unknown(format!(
                            "Failed to set webhook: {}",
                            error_text
                        )));
                    }

                    Ok(())
                }
            })
            .await?;

        info!("[Telegram] Webhook set to: {}", log_url);
        Ok(())
    }

    /// 删除 Webhook
    pub async fn delete_webhook(&self) -> Result<(), ChannelError> {
        self.retry_policy
            .execute(|| {
                let client = self.client.clone();
                let base_url = self.base_url.clone();
                async move {
                    let url = format!("{}/deleteWebhook", base_url);
                    client.get(&url).send().await?;
                    Ok::<(), ChannelError>(())
                }
            })
            .await?;

        info!("[Telegram] Webhook deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_update_deserialization() {
        let json = r#"{
            "update_id": 123456789,
            "message": {
                "message_id": 1,
                "from": {
                    "id": 12345,
                    "is_bot": false,
                    "first_name": "Test",
                    "username": "testuser"
                },
                "chat": {
                    "id": 12345,
                    "type": "private"
                },
                "date": 1609459200,
                "text": "Hello"
            }
        }"#;

        let update: TelegramUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 123456789);
        assert!(update.message.is_some());

        let message = update.message.unwrap();
        assert_eq!(message.text, Some("Hello".to_string()));
        assert_eq!(message.from.id, 12345);
    }

    #[test]
    fn test_split_message() {
        // 测试短消息
        let text = "Hello";
        let chunks = split_message(text, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello");

        // 测试长消息
        let long_text = "a".repeat(5000);
        let chunks = split_message(&long_text, 4096);
        assert!(chunks.len() > 1);

        // 测试在换行处分割
        let text_with_newlines = "Line1\nLine2\nLine3";
        let chunks = split_message(text_with_newlines, 10);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_telegram_api_client_creation() {
        let client = TelegramApiClient::new("test_token");
        assert_eq!(client.base_url, "https://api.telegram.org/bottest_token");
    }

    #[test]
    fn test_telegram_api_client_accepts_retry_policy() {
        use clarity_channels::retry::RetryPolicy;

        let client = TelegramApiClient::new("test_token")
            .with_retry_policy(RetryPolicy::new().with_max_attempts(5));

        assert_eq!(client.retry_policy.max_attempts, 5);
    }
}
