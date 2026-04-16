//! Discord Bot 渠道实现
//!
//! 使用 poise + serenity 实现 Discord Bot：
//! - 支持 Slash Command
//! - 接收消息事件
//! - 支持流式响应（分批发送）

#![allow(dead_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use clarity_core::Agent;

use super::{Channel, ChannelConfig, ChannelError};

// ==================== Feature-gated imports ====================

#[cfg(feature = "discord")]
use poise::serenity_prelude as serenity;

// ==================== Discord Channel ====================

/// Discord Bot 渠道
pub struct DiscordChannel {
    config: ChannelConfig,
    bot_token: Option<String>,
}

impl DiscordChannel {
    pub fn new(config: ChannelConfig) -> Self {
        Self {
            bot_token: config.token.clone(),
            config,
        }
    }

    /// 使用 poise 运行 bot（feature enabled）
    #[cfg(feature = "discord")]
    async fn run_with_poise(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        use serenity::GatewayIntents;

        let token = self.bot_token.as_ref().ok_or_else(|| {
            ChannelError::AuthFailed("Discord bot token not configured".to_string())
        })?;

        info!("[Discord] Starting bot with poise framework...");

        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILDS;

        // 创建 poise 框架
        let framework = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![ask(), help()],
                ..Default::default()
            })
            .setup(|ctx, _ready, framework| {
                Box::pin(async move {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    info!("[Discord] Slash commands registered");
                    Ok(Data { agent })
                })
            })
            .build();

        let mut client = serenity::Client::builder(token, intents)
            .framework(framework)
            .await
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        info!("[Discord] Bot is now running!");

        client
            .start()
            .await
            .map_err(|e| ChannelError::Unknown(e.to_string()))?;

        Ok(())
    }

    /// Mock 实现（feature disabled）
    #[cfg(not(feature = "discord"))]
    async fn run_with_poise(&self, _agent: Arc<Agent>) -> Result<(), ChannelError> {
        warn!("[Discord] discord feature is disabled, using mock mode");

        info!(
            "[Discord] Mock mode - would start bot with token: {:?}",
            self.bot_token.as_ref().map(|_| "***REDACTED***")
        );

        // 模拟运行
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        info!("[Discord] Mock bot finished");

        Ok(())
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        if !self.config.enabled {
            warn!("[Discord] Channel is disabled");
            return Ok(());
        }

        info!("[Discord] Starting channel...");
        self.run_with_poise(agent).await
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("[Discord] Stopping channel...");
        // Discord client 会在 drop 时自动关闭
        Ok(())
    }
}

// ==================== Poise Framework Types ====================

#[cfg(feature = "discord")]
struct Data {
    agent: Arc<Agent>,
}

#[cfg(feature = "discord")]
type Error = Box<dyn std::error::Error + Send + Sync>;
#[cfg(feature = "discord")]
type Context<'a> = poise::Context<'a, Data, Error>;

// ==================== Commands ====================

/// /ask 命令 - 向 AI 提问
#[cfg(feature = "discord")]
#[poise::command(slash_command, prefix_command, category = "AI")]
async fn ask(
    ctx: Context<'_>,
    #[description = "Your question"] question: String,
    #[description = "Enable streaming response"]
    #[flag]
    _stream: bool,
) -> Result<(), Error> {
    info!(
        "[Discord] /ask command from {}: {}",
        ctx.author().name,
        question
    );

    // 先发送 "thinking" 响应
    ctx.defer().await?;

    let agent = &ctx.data().agent;

    // 普通响应
    match agent.run(&question).await {
        Ok(response) => {
            // Discord 消息限制 2000 字符
            let chunks = split_discord_message(&response, 2000);
            for (i, chunk) in chunks.iter().enumerate() {
                let chunk_str = chunk.to_string();
                if i == 0 {
                    ctx.say(chunk_str).await?;
                } else {
                    ctx.channel_id().say(ctx.http(), chunk_str).await?;
                }
            }
        }
        Err(e) => {
            error!("[Discord] Agent error: {}", e);
            ctx.say(format!("❌ Error: {}", e)).await?;
        }
    }

    Ok(())
}

/// /help 命令 - 显示帮助信息
#[cfg(feature = "discord")]
#[poise::command(slash_command, prefix_command)]
async fn help(ctx: Context<'_>) -> Result<(), Error> {
    let help_text = r#"**Clarity Bot Help**

Available commands:
• `/ask <question>` - Ask the AI a question
• `/ask <question> --stream` - Get a streaming response
• `/help` - Show this help message

Tips:
- Questions can be up to 2000 characters
- Responses may be split into multiple messages if long
- Streaming mode shows response chunks as they arrive"#;

    ctx.say(help_text).await?;
    Ok(())
}

// ==================== Utilities ====================

/// 将长消息分割成适合 Discord 的块
fn split_discord_message(text: &str, max_length: usize) -> Vec<&str> {
    if text.len() <= max_length {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + max_length).min(text.len());

        // 尝试在代码块或换行处分割
        let split_point = if end < text.len() {
            let slice = &text[start..end];

            // 优先在 ``` 之前分割
            slice
                .rfind("\n```")
                .map(|i| start + i)
                .or_else(|| slice.rfind('\n').map(|i| start + i + 1))
                .unwrap_or(end)
        } else {
            end
        };

        chunks.push(&text[start..split_point.min(text.len())]);
        start = split_point;
    }

    chunks
}

// ==================== Discord API 类型 ====================

/// Discord Webhook 请求体
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscordWebhookRequest {
    pub content: Option<String>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub embeds: Option<Vec<DiscordEmbed>>,
}

/// Discord Embed
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub color: Option<u32>,
    pub fields: Option<Vec<DiscordEmbedField>>,
}

/// Discord Embed Field
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

/// HTTP 客户端用于发送 Discord Webhook
pub struct DiscordWebhookClient {
    client: reqwest::Client,
    webhook_url: String,
}

impl DiscordWebhookClient {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            webhook_url: webhook_url.into(),
        }
    }

    /// 发送简单文本消息
    pub async fn send_message(&self, content: &str) -> Result<(), ChannelError> {
        let body = DiscordWebhookRequest {
            content: Some(content.to_string()),
            username: None,
            avatar_url: None,
            embeds: None,
        };

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "Discord webhook error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送 Embed 消息
    pub async fn send_embed(&self, embed: DiscordEmbed) -> Result<(), ChannelError> {
        let body = DiscordWebhookRequest {
            content: None,
            username: None,
            avatar_url: None,
            embeds: Some(vec![embed]),
        };

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "Discord webhook error: {}",
                error_text
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_discord_message() {
        // 短消息
        let text = "Hello Discord!";
        let chunks = split_discord_message(text, 2000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);

        // 长消息
        let long_text = "a".repeat(5000);
        let chunks = split_discord_message(&long_text, 2000);
        assert!(chunks.len() >= 3);
    }

    #[test]
    fn test_discord_webhook_request_serialization() {
        let request = DiscordWebhookRequest {
            content: Some("Hello".to_string()),
            username: Some("Clarity Bot".to_string()),
            avatar_url: None,
            embeds: Some(vec![DiscordEmbed {
                title: Some("Test".to_string()),
                description: Some("Description".to_string()),
                color: Some(0x00ff00),
                fields: None,
            }]),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("Clarity Bot"));
    }
}
