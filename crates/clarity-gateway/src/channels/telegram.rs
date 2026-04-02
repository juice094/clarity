//! Telegram Bot 接口（预留实现）
//!
//! 提供 Telegram Bot 接入点，支持：
//! - 接收用户消息
//! - 发送回复
//! - Webhook 或 Long polling 模式

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{Channel, ChannelConfig, ChannelError, ChannelMessage, ChannelStatus};

/// Telegram Bot Channel
pub struct TelegramChannel {
    config: ChannelConfig,
    status: ChannelStatus,
    bot_token: Option<String>,
}

impl TelegramChannel {
    pub fn new(config: ChannelConfig) -> Self {
        let bot_token = config.token.clone();
        Self {
            config,
            status: ChannelStatus::Stopped,
            bot_token,
        }
    }

    /// 处理接收到的消息（供 webhook 调用）
    pub async fn handle_update(&self, update: TelegramUpdate) -> Result<(), ChannelError> {
        if let Some(message) = update.message {
            info!(
                "Received Telegram message from {}: {}",
                message.from.id, message.text.as_deref().unwrap_or("")
            );

            // TODO: 转发到 clarity-core 处理
            // 临时 echo 回复
            let response = format!("Echo: {}", message.text.as_deref().unwrap_or(""));
            self.send_message(&message.from.id.to_string(), &response)
                .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self) -> Result<(), ChannelError> {
        if !self.config.enabled {
            warn!("Telegram channel is disabled");
            return Ok(());
        }

        if self.bot_token.is_none() {
            return Err(ChannelError::AuthFailed(
                "Telegram bot token not configured".to_string(),
            ));
        }

        info!("Starting Telegram channel...");

        // TODO: 实现 webhook 设置或 long polling
        // let token = self.bot_token.as_ref().unwrap();
        // 1. 设置 webhook: https://api.telegram.org/bot{token}/setWebhook
        // 2. 或使用 long polling 模式

        info!("Telegram channel started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("Stopping Telegram channel...");
        // TODO: 清理 webhook 或停止 polling
        info!("Telegram channel stopped");
        Ok(())
    }

    async fn send_message(&self, user_id: &str, message: &str) -> Result<(), ChannelError> {
        let token = self
            .bot_token
            .as_ref()
            .ok_or_else(|| ChannelError::NotStarted)?;

        // TODO: 调用 Telegram API 发送消息
        // https://api.telegram.org/bot{token}/sendMessage
        // POST body: {"chat_id": user_id, "text": message}

        info!(
            "[Telegram] Would send message to {}: {}",
            user_id, message
        );

        Ok(())
    }

    fn status(&self) -> ChannelStatus {
        self.status.clone()
    }
}

// ==================== Telegram API 类型 ====================

/// Telegram Update（Webhook 推送的消息）
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

/// Telegram Message
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: TelegramUser,
    pub chat: TelegramChat,
    pub date: i64,
    pub text: Option<String>,
}

/// Telegram User
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramUser {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

/// Telegram Chat
#[derive(Debug, Deserialize, Serialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
}

/// 发送消息请求
#[derive(Debug, Serialize)]
pub struct SendMessageRequest {
    pub chat_id: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<String>,
}

// ==================== Webhook Handler ====================

/// 处理 Telegram webhook 请求
pub async fn webhook_handler(
    update: TelegramUpdate,
) -> Result<impl axum::response::IntoResponse, ChannelError> {
    // TODO: 从状态中获取 TelegramChannel 实例并处理
    info!("Received Telegram webhook: {:?}", update);

    // 临时返回成功
    Ok((axum::http::StatusCode::OK, "OK"))
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
}
