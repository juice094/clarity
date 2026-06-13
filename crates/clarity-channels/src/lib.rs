#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! External communication channels for Clarity.
//!
//! This crate provides a pluggable channel abstraction for messaging
//! platforms (WeChat, Telegram, Discord, etc.). The `zeroclaw` module
//! hosts the ZeroClaw-compatible channel primitives and the migrated
//! WeChat iLink implementation.
//!
//! Design goals:
//! - Zero frontend dependency.
//! - Async trait-based channels.
//! - Easy to add new platforms without touching core agent logic.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Exponential backoff retry policy for channel operations.
pub mod retry;

/// Re-export the retry policy and error classifier.
pub use retry::{RetryPolicy, RetryableError};

/// ZeroClaw-compatible channel primitives and the migrated WeChat iLink
/// implementation. This module is the target surface for the migration.
pub mod zeroclaw;

/// Emit a structured log record through `tracing`.
///
/// This is a lightweight port of `zeroclaw_log::record!`. It expects
/// severity literals `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR` and a
/// `zeroclaw::log::Event` payload.
#[macro_export]
macro_rules! record {
    ($level:ident, $event:expr, $msg:expr $(,)?) => {{
        let __zc_event: $crate::zeroclaw::log::Event = $event;
        let __zc_outcome = __zc_event.outcome.unwrap_or($crate::zeroclaw::log::EventOutcome::Unknown);
        ::tracing::event!(
            target: "clarity_channels_event",
            ::tracing::Level::$level,
            zc_name = %__zc_event.name,
            zc_action = %__zc_event.action.as_str(),
            zc_outcome = %__zc_outcome.as_str(),
            zc_attrs = %__zc_event.attrs,
            zc_file = %file!(),
            zc_line = %line!(),
            message = %$msg,
        );
    }};
}

/// Errors that can occur when operating a channel.
#[derive(Error, Debug)]
pub enum ChannelError {
    /// Channel is missing required configuration.
    #[error("channel is not configured: {0}")]
    NotConfigured(String),
    /// Channel configuration is invalid.
    #[error("configuration error: {0}")]
    ConfigError(String),
    /// Authentication with the platform failed.
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    /// Sending a message through the channel failed.
    #[error("send failed: {0}")]
    SendFailed(String),
    /// Network error from the HTTP client.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A cryptographic operation failed.
    #[error("crypto error: {0}")]
    Crypto(String),
    /// Platform returned an error response.
    #[error("platform error (code={code}): {message}")]
    Platform {
        /// Platform error code.
        code: i64,
        /// Platform error message.
        message: String,
    },
    /// Other channel-specific error.
    #[error("{0}")]
    Other(String),
}

/// A message received from an external channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    /// Unique message ID from the channel.
    pub id: String,
    /// Channel alias that received the message.
    pub channel_alias: String,
    /// Sender identifier (platform-specific).
    pub sender_id: String,
    /// Sender display name, if available.
    pub sender_name: Option<String>,
    /// Conversation / chat identifier.
    pub chat_id: String,
    /// Plain text content.
    pub text: String,
    /// Optional media attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// UTC timestamp.
    pub timestamp: DateTime<Utc>,
}

/// A media attachment in a channel message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// What kind of media this attachment represents.
    pub kind: AttachmentKind,
    /// Public URL when the attachment is hosted externally.
    pub url: Option<String>,
    /// Raw bytes when the attachment is loaded in memory.
    pub bytes: Option<Vec<u8>>,
    /// MIME type of the attachment, if known.
    pub mime_type: Option<String>,
    /// Original file name of the attachment.
    pub file_name: Option<String>,
}

/// Kinds of attachments supported by channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    /// Image attachment.
    Image,
    /// Voice/audio attachment.
    Voice,
    /// Video attachment.
    Video,
    /// Generic file attachment.
    File,
}

impl fmt::Display for AttachmentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttachmentKind::Image => write!(f, "image"),
            AttachmentKind::Voice => write!(f, "voice"),
            AttachmentKind::Video => write!(f, "video"),
            AttachmentKind::File => write!(f, "file"),
        }
    }
}

/// A reply to be sent back through a channel.
#[derive(Debug, Clone, Default)]
pub struct OutboundMessage {
    /// Target chat/recipient identifier.
    pub chat_id: String,
    /// Text content of the reply.
    pub text: String,
    /// Media attachments to send.
    pub attachments: Vec<Attachment>,
}

/// A message sink that the channel can use to deliver inbound messages.
#[async_trait]
pub trait MessageSink: Send + Sync {
    /// Called by the channel when a new message arrives.
    async fn on_message(&self, message: ChannelMessage);

    /// Called by the channel when a recoverable error occurs.
    async fn on_error(&self, error: ChannelError);
}

/// A communication channel (e.g. WeChat, Telegram, Discord).
#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable channel name.
    fn name(&self) -> &str;

    /// Check whether the channel is healthy and authenticated.
    async fn health_check(&self) -> Result<(), ChannelError>;

    /// Start receiving messages and forward them to the sink.
    /// This method should run until the channel is shut down.
    async fn run(&self, sink: Box<dyn MessageSink>) -> Result<(), ChannelError>;

    /// Send a message back through the channel.
    async fn send(&self, message: OutboundMessage) -> Result<(), ChannelError>;
}

/// A registry of configured channels.
#[derive(Default)]
pub struct ChannelRegistry {
    channels: Vec<Box<dyn Channel>>,
}

impl ChannelRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a channel implementation.
    pub fn register(&mut self, channel: Box<dyn Channel>) {
        self.channels.push(channel);
    }

    /// Borrow the registered channels.
    pub fn channels(&self) -> &[Box<dyn Channel>] {
        &self.channels
    }

    /// Mutably borrow the registered channels.
    pub fn channels_mut(&mut self) -> &mut [Box<dyn Channel>] {
        &mut self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_kind_display() {
        assert_eq!(AttachmentKind::Image.to_string(), "image");
        assert_eq!(AttachmentKind::Voice.to_string(), "voice");
    }
}
