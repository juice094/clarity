//! Server-side OpenClaw request/response types.
//!
//! These types mirror the client-side `clarity-claw::openclaw_gateway::types`
//! but are kept in the gateway so the server does not depend on the client
//! crate.

use serde::{Deserialize, Serialize};

/// A single structured chat content block.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatBlock {
    /// Plain text block.
    Text {
        /// Text content.
        text: String,
    },
    /// Image block.
    Image {
        /// Base64-encoded image data, if inline.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        /// Image URI, if referenced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uri: Option<String>,
    },
    /// File block.
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
    },
    /// Resource link block (e.g. `kimi-file://{uuid}`).
    ResourceLink {
        /// Resource URI.
        uri: String,
    },
    /// Generic resource block.
    Resource {
        /// Resource description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resource: Option<serde_json::Value>,
        /// URI.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uri: Option<String>,
        /// Text content.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
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

/// Response payload from `chat.send`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatSendResponse {
    /// The assistant message produced by the agent.
    pub message: OpenClawMessage,
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

/// Result of a `device.pair.request` call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairRequestResult {
    /// Device id that was registered.
    #[serde(alias = "deviceId", alias = "device_id")]
    pub device_id: String,
    /// Whether the pairing was immediately approved.
    pub approved: bool,
    /// Device token returned on approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Granted scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl ChatBlock {
    /// Extract plain text from a list of blocks.
    pub fn blocks_to_text(blocks: &[ChatBlock]) -> String {
        blocks
            .iter()
            .filter_map(|b| match b {
                ChatBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

impl OpenClawMessage {
    /// Build an assistant text message.
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(text.into()),
            blocks: None,
            timestamp_ms: None,
            id: None,
        }
    }

    /// Build a user text message.
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(text.into()),
            blocks: None,
            timestamp_ms: None,
            id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_block_blocks_to_text_joins_text_blocks() {
        let blocks = vec![
            ChatBlock::Text {
                text: "hello ".to_string(),
            },
            ChatBlock::Text {
                text: "world".to_string(),
            },
            ChatBlock::Image {
                data: None,
                uri: Some("kimi-file://img".to_string()),
            },
        ];
        assert_eq!(ChatBlock::blocks_to_text(&blocks), "hello world");
    }

    #[test]
    fn chat_block_blocks_to_text_empty() {
        assert_eq!(ChatBlock::blocks_to_text(&[]), "");
    }

    #[test]
    fn openclaw_message_builders() {
        let assistant = OpenClawMessage::assistant_text("reply");
        assert_eq!(assistant.role, "assistant");
        assert_eq!(assistant.content, Some("reply".to_string()));

        let user = OpenClawMessage::user_text("question");
        assert_eq!(user.role, "user");
        assert_eq!(user.content, Some("question".to_string()));
    }

    #[test]
    fn session_list_serializes_with_total() {
        let list = SessionList {
            sessions: vec![OpenClawSession {
                key: "agent:main:main".to_string(),
                title: Some("Default".to_string()),
                agent_id: Some("clarity".to_string()),
                created_at_ms: Some(1),
                updated_at_ms: Some(2),
                message_count: Some(0),
                model: None,
            }],
            total: Some(1),
        };
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(json["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(json["total"], 1);
    }
}
