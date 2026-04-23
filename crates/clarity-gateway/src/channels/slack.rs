//! Slack 渠道实现
//!
//! 基于 Slack Events API + Web API：
//! - 发送：通过 `chat.postMessage` 推送响应
//! - 接收：通过 Gateway 主服务器的 `/webhook/slack` 端点（由 WebhookChannel 统一处理）
//!
//! 配置环境变量：
//! - `SLACK_ENABLED=true`
//! - `SLACK_BOT_TOKEN=xoxb-...`

#![allow(dead_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use clarity_core::Agent;

use super::{Channel, ChannelConfig, ChannelError};

/// Slack Events API 推送的消息体（简化版）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlackEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub event: Option<SlackMessageEvent>,
    pub challenge: Option<String>,
}

/// Slack message event
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlackMessageEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub user: Option<String>,
    pub text: Option<String>,
    pub channel: String,
    pub ts: String,
}

/// Slack Web API 响应
#[derive(Debug, Clone, Deserialize)]
struct SlackApiResponse {
    ok: bool,
    error: Option<String>,
}

/// Slack Bot 渠道
pub struct SlackChannel {
    config: ChannelConfig,
    bot_token: Option<String>,
}

impl SlackChannel {
    pub fn new(config: ChannelConfig) -> Self {
        Self {
            bot_token: config.token.clone(),
            config,
        }
    }

    /// 发送消息到指定 Slack 频道
    pub async fn send_message(&self, channel: &str, text: &str) -> Result<(), ChannelError> {
        let token = self.bot_token.as_ref().ok_or_else(|| {
            ChannelError::AuthFailed("Slack bot token not configured".to_string())
        })?;

        let client = reqwest::Client::new();
        let resp = client
            .post("https://slack.com/api/chat.postMessage")
            .bearer_auth(token)
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&serde_json::json!({
                "channel": channel,
                "text": text,
            }))
            .send()
            .await
            .map_err(ChannelError::HttpError)?;

        let status = resp.status();
        let body: SlackApiResponse = resp.json().await.map_err(ChannelError::HttpError)?;

        if !body.ok {
            let err = body.error.unwrap_or_else(|| "unknown".to_string());
            return Err(ChannelError::SendFailed(format!(
                "Slack API error (HTTP {}): {}",
                status, err
            )));
        }

        Ok(())
    }

    /// 验证 Slack Events API 请求签名（可选）
    #[allow(dead_code)]
    pub fn verify_signature(
        &self,
        body: &str,
        timestamp: &str,
        signature: &str,
        signing_secret: &str,
    ) -> bool {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let base_string = format!("v0:{}:{}", timestamp, body);
        let mut mac = match HmacSha256::new_from_slice(signing_secret.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(base_string.as_bytes());
        let result = mac.finalize();
        let bytes: Vec<u8> = result.into_bytes().to_vec();
        let computed = format!("v0={}", bytes_to_hex(&bytes));

        // Constant-time comparison
        if computed.len() != signature.len() {
            return false;
        }
        computed.bytes().zip(signature.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        result.push(HEX[(b >> 4) as usize] as char);
        result.push(HEX[(b & 0x0f) as usize] as char);
    }
    result
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    async fn start(&self, _agent: Arc<Agent>) -> Result<(), ChannelError> {
        if self.bot_token.is_none() {
            warn!("[Slack] Bot token not set; channel will be inactive");
        } else {
            info!("[Slack] Channel ready (send via Web API, receive via /webhook/slack)");
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("[Slack] Channel stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_channel_new() {
        let config = ChannelConfig::new()
            .enabled()
            .with_token("xoxb-test-token");
        let channel = SlackChannel::new(config);
        assert!(channel.bot_token.is_some());
    }

    #[test]
    fn test_slack_channel_name() {
        let config = ChannelConfig::new();
        let channel = SlackChannel::new(config);
        assert_eq!(channel.name(), "slack");
    }

    #[tokio::test]
    async fn test_slack_send_no_token() {
        let config = ChannelConfig::new();
        let channel = SlackChannel::new(config);
        let result = channel.send_message("#general", "hello").await;
        assert!(matches!(result, Err(ChannelError::AuthFailed(_))));
    }

    #[test]
    fn test_slack_event_deserialize() {
        let json = r#"{
            "type": "event_callback",
            "event": {
                "type": "message",
                "user": "U123",
                "text": "hello bot",
                "channel": "C456",
                "ts": "1234567890.123456"
            }
        }"#;
        let event: SlackEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "event_callback");
        let msg = event.event.unwrap();
        assert_eq!(msg.text.unwrap(), "hello bot");
        assert_eq!(msg.channel, "C456");
    }

    #[test]
    fn test_slack_challenge_deserialize() {
        let json = r#"{"type":"url_verification","challenge":"abc123"}"#;
        let event: SlackEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.challenge.unwrap(), "abc123");
    }
}
