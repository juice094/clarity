//! WeChat iLink Bot channel adapter for Clarity Gateway.
//!
//! Wraps `clarity_channels::chkit::wechat::WeChatChannel` so it
//! implements the gateway's `Channel` trait. The WeChat iLink implementation
//! supports dynamic `/bind <code>` pairing, media attachments, typing
//! indicators, and structured logging.
//!
//! Configuration is read from the channel's `extra` JSON:
//! - `alias`: channel alias used in logs and state file names.
//! - `data_dir`: directory where bot token, sync cursor, and allowed
//!   users are persisted.
//! - `allowed_users`: optional initial list of allowed sender IDs.
//! - `api_base_url`: optional override for the iLink API base URL.
//! - `cdn_base_url`: optional override for the iLink CDN base URL.
//!
//! Environment variables used by `load_channel_configs`:
//! - `WECHAT_ENABLED=true`
//! - `WECHAT_ALIAS` (default: "default")
//! - `WECHAT_DATA_DIR` (default: platform data dir / clarity / channels)
//! - `WECHAT_ALLOWED_USERS` (comma-separated, default: none)

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, info, warn};

use clarity_channels::chkit::channel::{Channel as ChkitChannel, ChannelMessage, SendMessage};
use clarity_channels::chkit::wechat::WeChatChannel;
use clarity_core::Agent;

use super::{Channel, ChannelConfig, ChannelError};

/// WeChat gateway channel adapter.
pub struct WeChatGatewayChannel {
    config: ChannelConfig,
    inner: Option<Arc<WeChatChannel>>,
}

impl WeChatGatewayChannel {
    /// Create a new WeChat gateway channel from the given configuration.
    pub fn new(config: ChannelConfig) -> Self {
        let inner = match build_wechat_channel(&config) {
            Ok(channel) => Some(Arc::new(channel)),
            Err(e) => {
                warn!("[WeChat] Failed to create channel: {}", e);
                None
            }
        };

        Self { config, inner }
    }
}

#[async_trait]
impl Channel for WeChatGatewayChannel {
    fn name(&self) -> &str {
        "wechat"
    }

    async fn start(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        if !self.config.enabled {
            info!("[WeChat] Channel is disabled");
            return Ok(());
        }

        let inner = self.inner.as_ref().ok_or_else(|| {
            ChannelError::ConfigError("WeChat channel failed to initialize".to_string())
        })?;

        info!("[WeChat] Starting channel...");
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ChannelMessage>(128);
        let channel = inner.clone();

        // Long-running listener task.
        let listen_handle = tokio::spawn(async move {
            if let Err(e) = channel.listen(tx).await {
                error!("[WeChat] Listener exited: {}", e);
            }
        });

        // Agent reply task.
        let channel = inner.clone();
        let agent_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                info!(
                    "[WeChat] Received message from {}: {}",
                    message.sender, message.content
                );

                match agent.run(&message.content).await {
                    Ok(response) => {
                        let chunks = split_message(&response, 2000);
                        for chunk in chunks {
                            let outbound = SendMessage::reply_to(&message, chunk.to_string());
                            if let Err(e) = channel.send(&outbound).await {
                                error!("[WeChat] Failed to send reply: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("[WeChat] Agent error: {}", e);
                        let outbound = SendMessage::reply_to(
                            &message,
                            "Sorry, I encountered an error processing your message.",
                        );
                        let _ = channel.send(&outbound).await;
                    }
                }
            }

            // Keep the listener handle alive until the receiver drops.
            let _ = listen_handle.await;
        });

        agent_task
            .await
            .map_err(|e| ChannelError::Unknown(e.to_string()))
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("[WeChat] Stopping channel...");
        Ok(())
    }
}

/// Build a `WeChatChannel` from the gateway's `ChannelConfig::extra` JSON.
fn build_wechat_channel(config: &ChannelConfig) -> anyhow::Result<WeChatChannel> {
    let extra = config
        .extra
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing WeChat extra config"))?;

    let alias = extra
        .get("alias")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    let state_dir = extra
        .get("data_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);

    let api_base_url = extra
        .get("api_base_url")
        .and_then(|v| v.as_str())
        .map(String::from);

    let cdn_base_url = extra
        .get("cdn_base_url")
        .and_then(|v| v.as_str())
        .map(String::from);

    let static_allowed: Vec<String> = extra
        .get("allowed_users")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let state_dir_for_resolver = state_dir.clone();
    let peer_resolver: Arc<dyn Fn() -> Vec<String> + Send + Sync> = Arc::new(move || {
        let file_users = load_allowed_users(&state_dir_for_resolver);
        let mut combined = static_allowed.clone();
        for u in file_users {
            if !combined.contains(&u) {
                combined.push(u);
            }
        }
        combined
    });

    let channel = WeChatChannel::new(
        alias,
        peer_resolver,
        api_base_url,
        cdn_base_url,
        Some(state_dir),
    )?;

    Ok(channel)
}

/// Load allowed users persisted via the `/bind` pairing flow.
fn load_allowed_users(state_dir: &std::path::Path) -> Vec<String> {
    let path = state_dir.join("allowed_users.json");
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Default data directory for WeChat state.
fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.join("clarity").join("channels"))
        .unwrap_or_else(|| PathBuf::from(".clarity").join("channels"))
}

/// Split a long message into chunks at newline boundaries when possible.
fn split_message(text: &str, max_length: usize) -> Vec<&str> {
    if text.len() <= max_length {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + max_length).min(text.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_message_short() {
        let text = "Hello";
        let chunks = split_message(text, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello");
    }

    #[test]
    fn test_split_message_long() {
        let long_text = "a".repeat(5000);
        let chunks = split_message(&long_text, 2000);
        assert!(chunks.len() > 1);
    }
}
