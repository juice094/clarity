//! ChannelSendTool — send messages to IM channels (Feishu, DingTalk, Slack, Webhook)
//!
//! Allows an agent to proactively send messages to configured IM channels.
//! Supports platform-specific formatting and HMAC-SHA256 signature (DingTalk).
//!
//! # Supported Platforms
//!
//! | Platform | Format | Auth |
//! |----------|--------|------|
//! | `feishu` | `{"msg_type": "text", "content": {"text": ...}}` | None |
//! | `dingtalk` | `{"msgtype": "text", "text": {"content": ...}}` + timestamp + sign | HMAC-SHA256 |
//! | `slack` | `{"text": ...}` | None (webhook URL contains token) |
//! | `webhook` | Generic JSON `{"message": ...}` | None |

use async_trait::async_trait;
use serde_json::{Value, json};
use tracing::info;

use crate::helpers;
use crate::{Tool, ToolContext, ToolResult};
use clarity_contract::ToolError;

/// Tool for sending messages to IM channels
pub struct ChannelSendTool;

impl ChannelSendTool {
    /// Create a new ChannelSendTool instance
    pub fn new() -> Self {
        Self
    }

    /// Build platform-specific payload
    fn build_payload(
        &self,
        platform: &str,
        message: &str,
        secret: Option<&str>,
    ) -> ToolResult<Value> {
        match platform {
            "feishu" => Ok(json!({
                "msg_type": "text",
                "content": {
                    "text": message
                }
            })),
            "dingtalk" => {
                let timestamp = chrono::Utc::now().timestamp_millis();
                let sign = match secret {
                    Some(s) => Some(Self::compute_dingtalk_sign(s, timestamp)?),
                    None => None,
                };

                let mut payload = json!({
                    "msgtype": "text",
                    "text": {
                        "content": message
                    }
                });

                if let Some(sign) = sign {
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert("timestamp".to_string(), json!(timestamp));
                        obj.insert("sign".to_string(), json!(sign));
                    }
                }

                Ok(payload)
            }
            "slack" => Ok(json!({
                "text": message
            })),
            _ => Ok(json!({
                "message": message
            })),
        }
    }

    /// Compute DingTalk HMAC-SHA256 signature
    fn compute_dingtalk_sign(secret: &str, timestamp: i64) -> ToolResult<String> {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let sign_string = format!("{}\n{}", timestamp, secret);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| {
            ToolError::execution_failed(format!("Failed to initialize HMAC signer: {}", e))
        })?;
        mac.update(sign_string.as_bytes());
        let result = mac.finalize();
        let bytes = result.into_bytes();
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }
}

impl Default for ChannelSendTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ChannelSendTool {
    fn name(&self) -> &str {
        "channel_send"
    }

    fn description(&self) -> &str {
        "Send a message to an IM channel or webhook. \
         Supports Feishu (飞书), DingTalk (钉钉), Slack, and generic webhooks. \
         Use this when the agent needs to proactively notify a user or channel."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "platform": {
                    "type": "string",
                    "enum": ["feishu", "dingtalk", "slack", "webhook"],
                    "description": "Target platform"
                },
                "webhook_url": {
                    "type": "string",
                    "description": "Webhook URL for the target platform"
                },
                "message": {
                    "type": "string",
                    "description": "Message content to send"
                },
                "secret": {
                    "type": "string",
                    "description": "Secret key for HMAC signature (required for DingTalk)"
                }
            },
            "required": ["platform", "webhook_url", "message"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let platform = helpers::required_str(&args, "platform")?;
        let webhook_url = helpers::required_str(&args, "webhook_url")?;
        let message = helpers::required_str(&args, "message")?;
        let secret = helpers::optional_str(&args, "secret");

        info!(
            "ChannelSendTool: platform={}, webhook={}",
            platform, webhook_url
        );

        // Validate platform
        match platform {
            "feishu" | "dingtalk" | "slack" | "webhook" => {}
            _ => {
                return Err(ToolError::invalid_params(format!(
                    "Unsupported platform: {}. Supported: feishu, dingtalk, slack, webhook",
                    platform
                )));
            }
        }

        // Build platform-specific payload
        let payload = self.build_payload(platform, message, secret)?;

        // Send HTTP POST
        let client = reqwest::Client::new();
        let resp = client.post(webhook_url).json(&payload).send().await;

        match resp {
            Ok(response) => {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "(failed to read body)".to_string());

                if status.is_success() {
                    Ok(json!({
                        "sent": true,
                        "platform": platform,
                        "status": status.as_u16(),
                        "response_body": body,
                    }))
                } else {
                    Err(ToolError::execution_failed(format!(
                        "Webhook returned HTTP {}: {}",
                        status.as_u16(),
                        body
                    )))
                }
            }
            Err(e) => Err(ToolError::execution_failed(format!(
                "Failed to send webhook request: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_payload_feishu() {
        let tool = ChannelSendTool::new();
        let payload = tool.build_payload("feishu", "Hello", None).unwrap();
        assert_eq!(payload["msg_type"].as_str().unwrap(), "text");
        assert_eq!(payload["content"]["text"].as_str().unwrap(), "Hello");
    }

    #[test]
    fn test_build_payload_dingtalk() {
        let tool = ChannelSendTool::new();
        let payload = tool
            .build_payload("dingtalk", "Hello", Some("secret123"))
            .unwrap();
        assert_eq!(payload["msgtype"].as_str().unwrap(), "text");
        assert_eq!(payload["text"]["content"].as_str().unwrap(), "Hello");
        assert!(payload["timestamp"].is_number());
        assert!(payload["sign"].is_string());
    }

    #[test]
    fn test_build_payload_slack() {
        let tool = ChannelSendTool::new();
        let payload = tool.build_payload("slack", "Hello", None).unwrap();
        assert_eq!(payload["text"].as_str().unwrap(), "Hello");
    }

    #[test]
    fn test_build_payload_webhook() {
        let tool = ChannelSendTool::new();
        let payload = tool.build_payload("webhook", "Hello", None).unwrap();
        assert_eq!(payload["message"].as_str().unwrap(), "Hello");
    }

    #[test]
    fn test_compute_dingtalk_sign() {
        use base64::Engine;
        let sign = ChannelSendTool::compute_dingtalk_sign("secret123", 1234567890).unwrap();
        // Verify it's valid base64
        let decoded = base64::engine::general_purpose::STANDARD.decode(&sign);
        assert!(decoded.is_ok());
        assert_eq!(decoded.unwrap().len(), 32); // SHA256 output length
    }

    #[test]
    fn test_tool_name() {
        let tool = ChannelSendTool::new();
        assert_eq!(tool.name(), "channel_send");
    }

    #[test]
    fn test_tool_parameters() {
        let tool = ChannelSendTool::new();
        let params = tool.parameters();
        assert_eq!(params["type"].as_str().unwrap(), "object");
        assert!(params["properties"]["platform"].is_object());
    }
}
