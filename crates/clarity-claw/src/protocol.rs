//! Protocol abstraction for Claw Gateway connections.
//!
//! OpenClaw JSON-RPC and the native Clarity Gateway WebSocket protocol are
//! unified behind a single command/event interface so consumers can connect
//! to either dialect without prior knowledge of which one the server speaks.

use serde_json::Value;

/// Commands that can be sent to any Claw protocol handler.
#[derive(Clone, Debug)]
pub enum ProtocolCommand {
    /// Send a chat message.
    ///
    /// The wire method is chosen by the detected dialect:
    /// - Gateway WebSocket uses `chat.send`.
    /// - OpenClaw JSON-RPC uses `sessions.send`.
    Chat {
        /// Session key (OpenClaw) or unused (Gateway).
        session_key: String,
        /// Message text.
        message: String,
    },
    /// Fetch message history.
    GetHistory {
        /// Session key to load history for.
        session_key: String,
    },
    /// Subscribe to session-level events.
    SubscribeSession {
        /// Session key to subscribe to.
        key: String,
    },
    /// Subscribe to message-level events.
    SubscribeMessages {
        /// Session key to subscribe to.
        key: String,
    },
    /// Request missing role-context events from the Gateway (Gateway dialect
    /// only; OpenClaw transports should implement sync through a separate
    /// mechanism or return an error).
    SyncRoleContext {
        /// Role context to synchronize.
        role_id: String,
        /// Last event id known locally; None means from the beginning.
        since_event_id: Option<String>,
        /// Local device id, used for presence tracking.
        device_id: String,
    },
    /// Provide or remove the passphrase used to encrypt role-context events at
    /// rest for the OpenClaw/syncthing-rust transport.
    ///
    /// An empty `passphrase` clears any existing key for the role.
    SetRolePassphrase {
        /// Role context to set the passphrase for.
        role_id: String,
        /// Passphrase; empty means "clear".
        passphrase: String,
    },
}

/// Events emitted by any Claw protocol handler.
#[derive(Clone, Debug)]
pub enum ProtocolEvent {
    /// Connection established and ready for commands.
    Connected {
        /// Gateway URL.
        gateway_url: String,
        /// Session id if returned by the server.
        session_id: Option<String>,
    },
    /// A chat response chunk.
    ChatChunk(String),
    /// The current response is complete.
    Done,
    /// Conversation history payload.
    History(Vec<ProtocolHistoryMessage>),
    /// Device pairing result (OpenClaw only).
    PairingResult {
        /// Paired device id.
        device_id: String,
        /// Whether pairing was approved.
        approved: bool,
        /// Auth token returned by the Gateway.
        token: Option<String>,
        /// Granted scopes.
        scopes: Vec<String>,
    },
    /// The connection is retrying after a transient failure (OpenClaw only).
    ReconnectPending {
        /// Human-readable reason for the reconnect.
        reason: String,
        /// Seconds until the next retry attempt.
        seconds: u64,
    },
    /// Server-side or client-side error.
    Error(String),
    /// Raw wire message for protocols that stream them directly.
    WireMessage(Value),
    /// Response to a role-context sync request (Gateway dialect only).
    RoleContextSynced {
        /// Role that was synchronized.
        role_id: String,
        /// Missing events returned by the Gateway.
        events: Vec<clarity_contract::ClawContextEvent>,
        /// Cursor for the next sync request.
        next_cursor: Option<String>,
        /// Devices currently online for this role.
        online_devices: Vec<String>,
    },
    /// The transport does not support the requested operation.
    Unsupported {
        /// Human-readable reason.
        reason: String,
    },
}

/// A single message in a history response.
#[derive(Clone, Debug)]
pub struct ProtocolHistoryMessage {
    /// Role of the message author.
    pub role: String,
    /// Message content.
    pub content: String,
}

/// Wire dialect detected from the server's first message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectedProtocol {
    /// Native Clarity Gateway WebSocket protocol (`WsResponse`).
    GatewayWebSocket,
    /// OpenClaw / KimiClaw JSON-RPC over WebSocket.
    OpenClawJsonRpc,
}

impl DetectedProtocol {
    /// Inspect a server's first text frame and decide which dialect it speaks.
    ///
    /// The Gateway protocol always starts with a `welcome` envelope, while
    /// OpenClaw starts with an `event` (`hello-ok` or `connect.challenge`) or
    /// a `res` reply. An empty or unparseable frame defaults to OpenClaw so
    /// the handler can surface the parse error to the caller.
    pub fn from_first_frame(text: &str) -> Self {
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if msg_type == "welcome" {
                return Self::GatewayWebSocket;
            }
        }
        Self::OpenClawJsonRpc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_gateway_welcome() {
        let json = r#"{"type":"welcome","session_id":"abc","message":"hello"}"#;
        assert_eq!(
            DetectedProtocol::from_first_frame(json),
            DetectedProtocol::GatewayWebSocket
        );
    }

    #[test]
    fn detect_openclaw_hello_ok() {
        let json = r#"{"type":"event","event":"hello-ok","payload":{}}"#;
        assert_eq!(
            DetectedProtocol::from_first_frame(json),
            DetectedProtocol::OpenClawJsonRpc
        );
    }

    #[test]
    fn detect_openclaw_challenge() {
        let json = r#"{"type":"event","event":"connect.challenge","payload":{"nonce":"n1"}}"#;
        assert_eq!(
            DetectedProtocol::from_first_frame(json),
            DetectedProtocol::OpenClawJsonRpc
        );
    }

    #[test]
    fn detect_unparseable_defaults_to_openclaw() {
        // Defaulting to OpenClaw lets the handler emit a concrete parse error
        // instead of silently choosing a protocol.
        assert_eq!(
            DetectedProtocol::from_first_frame("not json"),
            DetectedProtocol::OpenClawJsonRpc
        );
    }
}
