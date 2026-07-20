//! Shared types for the OpenClaw Gateway client.
//!
//! These types model sessions, messages, and chat blocks as observed in the
//! Kimi Desktop OpenClaw Gateway protocol.

use serde::{Deserialize, Serialize};

/// A session returned by `sessions.list` or `sessions.preview`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawSession {
    /// Unique session key, e.g. `agent:main:main`.
    pub key: String,
    /// Human-readable title, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Agent id owning the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Session creation timestamp (ms since Unix epoch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_ms: Option<u64>,
    /// Last update timestamp (ms since Unix epoch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<u64>,
    /// Number of messages in the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_count: Option<usize>,
    /// Current model alias, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Response payload from `sessions.list`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SessionList {
    /// Matching sessions.
    pub sessions: Vec<OpenClawSession>,
    /// Total count (may exceed returned page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
}

/// A single message in a chat history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenClawMessage {
    /// Author role: `user`, `assistant`, `system`, or `tool`.
    pub role: String,
    /// Message content as a string when the server provides plain text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Structured content blocks when the server provides them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<ChatBlock>>,
    /// Optional timestamp in milliseconds since Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
    /// Optional message id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// A structured chat content block.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatBlock {
    /// Plain text block.
    Text {
        /// Text content.
        text: String,
    },
    /// Image block (inline data or URI).
    Image {
        /// Base64-encoded image data, if inline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        /// Image URI, if referenced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uri: Option<String>,
        /// MIME type.
        #[serde(default, skip_serializing_if = "Option::is_none", alias = "mime_type")]
        mime_type: Option<String>,
        /// File name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_name: Option<String>,
    },
    /// File block (inline data or URI).
    File {
        /// Base64-encoded file data, if inline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        /// Text extraction, if available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// File URI, if referenced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uri: Option<String>,
        /// MIME type.
        #[serde(default, skip_serializing_if = "Option::is_none", alias = "mime_type")]
        mime_type: Option<String>,
        /// File name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_name: Option<String>,
    },
    /// Resource link block (e.g. `kimi-file://{uuid}`).
    ResourceLink {
        /// Resource URI.
        uri: String,
        /// Optional title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Optional name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Optional MIME type.
        #[serde(default, skip_serializing_if = "Option::is_none", alias = "mime_type")]
        mime_type: Option<String>,
    },
    /// Generic resource block.
    Resource {
        /// Resource description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resource: Option<serde_json::Value>,
        /// URI.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uri: Option<String>,
        /// MIME type.
        #[serde(default, skip_serializing_if = "Option::is_none", alias = "mime_type")]
        mime_type: Option<String>,
        /// Text content.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// Inline data.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        /// File name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_name: Option<String>,
    },
}

/// Parameters for `chat.send`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendParams {
    /// Target session key.
    pub session_key: String,
    /// Message content blocks.
    pub message: Vec<ChatBlock>,
    /// Whether to stream the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Parameters for `chat.history`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatHistoryParams {
    /// Target session key.
    pub session_key: String,
    /// Maximum messages to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Whether to include tool messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_tools: Option<bool>,
}

/// Response payload from `chat.history`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ChatHistory {
    /// Messages in chronological order.
    pub messages: Vec<OpenClawMessage>,
    /// Cursor for pagination, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Server-side chat event payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatEvent {
    /// Session key.
    pub session_key: String,
    /// Message being streamed or completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<OpenClawMessage>,
    /// Chunk of assistant text when streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk: Option<String>,
    /// Whether this is the final event for the turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
}

/// Server-side agent event payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Session key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    /// Event kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Raw payload for forward compatibility.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Parameters for `sessions.list`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SessionListParams {
    /// Filter by kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<String>>,
    /// Maximum sessions to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Only sessions active within the last N minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_minutes: Option<u32>,
    /// Maximum messages to consider for activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_block_text_serializes() {
        let block = ChatBlock::Text {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn resource_link_parses() {
        let json = r#"{"type":"resource_link","uri":"kimi-file://123e4567-e89b-12d3-a456-426614174000","title":"doc.pdf"}"#;
        let block: ChatBlock = serde_json::from_str(json).unwrap();
        match block {
            ChatBlock::ResourceLink { uri, title, .. } => {
                assert_eq!(uri, "kimi-file://123e4567-e89b-12d3-a456-426614174000");
                assert_eq!(title, Some("doc.pdf".to_string()));
            }
            _ => panic!("expected resource_link"),
        }
    }

    #[test]
    fn session_list_parses() {
        let json = r#"{"sessions":[{"key":"agent:main:main","title":"Main"}],"total":1}"#;
        let list: SessionList = serde_json::from_str(json).unwrap();
        assert_eq!(list.sessions.len(), 1);
        assert_eq!(list.sessions[0].key, "agent:main:main");
        assert_eq!(list.total, Some(1));
    }
}
