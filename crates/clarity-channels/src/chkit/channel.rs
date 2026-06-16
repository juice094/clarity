//! Reusable channel primitives for chat-platform adapters.
//!
//! Provides message/attachment types and traits used by the in-tree WeChat
//! iLink implementation and future channel adapters.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

/// A single media attachment on an inbound or outbound message.
#[derive(Debug, Clone)]
pub struct MediaAttachment {
    /// Original file name (e.g. `voice.ogg`, `photo.jpg`).
    pub file_name: String,
    /// Raw bytes of the attachment.
    pub data: Vec<u8>,
    /// MIME type if known (e.g. `audio/ogg`, `image/jpeg`).
    pub mime_type: Option<String>,
}

impl MediaAttachment {
    /// Load an attachment from a file path on disk.
    ///
    /// # Safety
    /// Callers must validate `path` when it originates from untrusted input.
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let p = std::path::Path::new(path);
        let data = std::fs::read(p)?;
        let file_name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment")
            .to_string();
        let mime_type = match p.extension().and_then(|e| e.to_str()) {
            Some("pdf") => Some("application/pdf".to_string()),
            Some("xlsx") => Some(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            ),
            Some("docx") => Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            ),
            Some("csv") => Some("text/csv".to_string()),
            Some("png") => Some("image/png".to_string()),
            Some("jpg") | Some("jpeg") => Some("image/jpeg".to_string()),
            Some("txt") => Some("text/plain".to_string()),
            _ => Some("application/octet-stream".to_string()),
        };
        Ok(Self {
            file_name,
            data,
            mime_type,
        })
    }
}

/// Compact description of a tool call presented to the user for approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelApprovalRequest {
    /// Name of the tool being called.
    pub tool_name: String,
    /// Human-readable summary of the tool arguments.
    pub arguments_summary: String,
    /// Raw tool arguments for channels that can render structured diffs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_arguments: Option<serde_json::Value>,
}

/// The operator's response to a channel-presented approval prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelApprovalResponse {
    /// Execute this one call.
    Approve,
    /// Deny this call.
    Deny,
    /// Execute and add tool to session-scoped allowlist.
    #[serde(rename = "always")]
    AlwaysApprove,
    /// Deny this call and supply an edited replacement for the arguments.
    #[serde(rename = "deny_with_edit")]
    DenyWithEdit {
        /// Edited argument string to use instead.
        replacement: String,
    },
}

/// A message received from or sent to a channel.
#[derive(Debug, Clone, Default)]
pub struct ChannelMessage {
    /// Unique message identifier.
    pub id: String,
    /// Sender identifier (platform-specific).
    pub sender: String,
    /// Platform identifier used when replying (user ID / chat ID / room ID).
    pub reply_target: String,
    /// Text content of the message.
    pub content: String,
    /// Channel type name (e.g. `wechat`).
    pub channel: String,
    /// Channel alias when the platform supports multiple bot instances.
    pub channel_alias: Option<String>,
    /// Unix epoch seconds.
    pub timestamp: u64,
    /// Platform thread identifier for threaded replies.
    pub thread_ts: Option<String>,
    /// Thread scope identifier for interruption/cancellation grouping.
    pub interruption_scope_id: Option<String>,
    /// Media attachments.
    pub attachments: Vec<MediaAttachment>,
    /// Email subject for reply threading.
    pub subject: Option<String>,
}

/// Message to send through a channel.
#[derive(Debug, Clone)]
pub struct SendMessage {
    /// Text content of the message.
    pub content: String,
    /// Recipient identifier.
    pub recipient: String,
    /// Subject line for email-style channels.
    pub subject: Option<String>,
    /// Thread timestamp/identifier.
    pub thread_ts: Option<String>,
    /// Cancellation token that can abort send retries.
    pub cancellation_token: Option<CancellationToken>,
    /// Media attachments.
    pub attachments: Vec<MediaAttachment>,
    /// Identifier of the message this one replies to.
    pub in_reply_to: Option<String>,
}

impl SendMessage {
    /// Create a new message with content and recipient.
    pub fn new(content: impl Into<String>, recipient: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            recipient: recipient.into(),
            subject: None,
            thread_ts: None,
            cancellation_token: None,
            attachments: vec![],
            in_reply_to: None,
        }
    }

    /// Create a new message with content, recipient, and subject.
    pub fn with_subject(
        content: impl Into<String>,
        recipient: impl Into<String>,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            recipient: recipient.into(),
            subject: Some(subject.into()),
            thread_ts: None,
            cancellation_token: None,
            attachments: vec![],
            in_reply_to: None,
        }
    }

    /// Set the message this one replies to.
    pub fn in_reply_to(mut self, msg_id: Option<String>) -> Self {
        self.in_reply_to = msg_id;
        self
    }

    /// Set the subject line (useful for email-style channels).
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Set the thread timestamp/identifier.
    pub fn in_thread(mut self, thread_ts: Option<String>) -> Self {
        self.thread_ts = thread_ts;
        self
    }

    /// Attach a cancellation token that can abort send retries.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Attach media files to the message.
    pub fn with_attachments(mut self, attachments: Vec<MediaAttachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// Build a reply `SendMessage` from an inbound `ChannelMessage`.
    pub fn reply_to(msg: &ChannelMessage, content: impl Into<String>) -> Self {
        let mut sm = Self::new(content, &msg.reply_target)
            .in_thread(msg.thread_ts.clone())
            .in_reply_to(Some(msg.id.clone()));
        if let Some(ref subj) = msg.subject {
            let reply_subject = if subj.to_ascii_lowercase().starts_with("re:") {
                subj.clone()
            } else {
                format!("Re: {}", subj)
            };
            sm = sm.subject(reply_subject);
        }
        sm
    }
}

impl ChannelMessage {
    /// Construct a `ChannelMessage` with required fields set and optional
    /// fields zeroed.
    pub fn new(
        id: impl Into<String>,
        sender: impl Into<String>,
        reply_target: impl Into<String>,
        content: impl Into<String>,
        channel: impl Into<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            id: id.into(),
            sender: sender.into(),
            reply_target: reply_target.into(),
            content: content.into(),
            channel: channel.into(),
            timestamp,
            ..Self::default()
        }
    }
}

/// Core channel trait — implement for any messaging platform.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable channel name.
    fn name(&self) -> &str;

    /// Send a message through this channel.
    async fn send(&self, message: &SendMessage) -> anyhow::Result<()>;

    /// Start listening for incoming messages (long-running).
    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()>;

    /// Check if channel is healthy.
    async fn health_check(&self) -> bool {
        true
    }

    /// Signal that the bot is processing a response (e.g. "typing" indicator).
    async fn start_typing(&self, _recipient: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Stop any active typing indicator.
    async fn stop_typing(&self, _recipient: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Whether this channel supports progressive message updates via draft edits.
    fn supports_draft_updates(&self) -> bool {
        false
    }

    /// Returns the bot's own handle/identity on this channel, if known.
    fn self_handle(&self) -> Option<String> {
        None
    }

    /// The exact form the bot expects when addressed by users on this channel.
    fn self_addressed_mention(&self) -> Option<String> {
        None
    }

    /// Whether the orchestrator should drop an inbound message as self-authored.
    fn drop_self_messages(&self, msg: &ChannelMessage) -> bool {
        let Some(handle) = self.self_handle() else {
            return false;
        };
        let handle_norm = handle.trim_start_matches('@').to_ascii_lowercase();
        let sender_norm = msg.sender.trim_start_matches('@').to_ascii_lowercase();
        !handle_norm.is_empty() && handle_norm == sender_norm
    }

    /// Whether this channel supports multi-message streaming delivery.
    fn supports_multi_message_streaming(&self) -> bool {
        false
    }

    /// Minimum delay (ms) between sending each paragraph in multi-message mode.
    fn multi_message_delay_ms(&self) -> u64 {
        800
    }

    /// Send an initial draft message. Returns a platform-specific message ID.
    async fn send_draft(&self, _message: &SendMessage) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    /// Update a previously sent draft message.
    async fn update_draft(
        &self,
        _recipient: &str,
        _message_id: &str,
        _text: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Finalize a draft with the complete response.
    async fn finalize_draft(
        &self,
        _recipient: &str,
        _message_id: &str,
        _text: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Cancel a previously sent draft message.
    async fn cancel_draft(&self, _recipient: &str, _message_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Request interactive tool-call approval from the channel operator.
    async fn request_approval(
        &self,
        _recipient: &str,
        _request: &ChannelApprovalRequest,
    ) -> anyhow::Result<Option<ChannelApprovalResponse>> {
        Ok(None)
    }

    /// Ask the user a multiple-choice question.
    async fn request_choice(
        &self,
        _question: &str,
        _choices: &[String],
        _timeout: std::time::Duration,
    ) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    /// Whether this channel can answer free-form questions.
    fn supports_free_form_ask(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_message_reply_to_sets_threading_fields() {
        let inbound = ChannelMessage {
            id: "msg-001".into(),
            reply_target: "user@example.com".into(),
            thread_ts: Some("thread-1".into()),
            subject: Some("Hello there".into()),
            ..ChannelMessage::new("msg-001", "alice", "user@example.com", "", "email", 0)
        };
        let reply = SendMessage::reply_to(&inbound, "Got it");
        assert_eq!(reply.recipient, "user@example.com");
        assert_eq!(reply.in_reply_to.as_deref(), Some("msg-001"));
        assert_eq!(reply.thread_ts.as_deref(), Some("thread-1"));
        assert_eq!(reply.subject.as_deref(), Some("Re: Hello there"));
        assert_eq!(reply.content, "Got it");
    }

    #[test]
    fn drop_self_messages_normalizes_at_prefix_and_case() {
        struct StubChannel;
        #[async_trait]
        impl Channel for StubChannel {
            fn name(&self) -> &str {
                "stub"
            }
            fn self_handle(&self) -> Option<String> {
                Some("My_Bot".into())
            }
            async fn send(&self, _message: &SendMessage) -> anyhow::Result<()> {
                Ok(())
            }
            async fn listen(
                &self,
                _tx: tokio::sync::mpsc::Sender<ChannelMessage>,
            ) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let ch = StubChannel;
        let msg = ChannelMessage::new("1", "@my_bot", "", "hi", "stub", 0);
        assert!(ch.drop_self_messages(&msg));

        let other = ChannelMessage::new("1", "@other", "", "hi", "stub", 0);
        assert!(!ch.drop_self_messages(&other));
    }
}
